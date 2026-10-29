#![cfg(target_os = "macos")]
//! Proposal 0005, §2 and §4: what the host must get right before any native object is read.
//!
//! These exercise the real transport against a scripted helper bound to an owner-only endpoint in
//! this process, so the peer the kernel reports is this test process. They cover framing and
//! allocation limits, descriptor permissions and ownership, capability redaction, and the session
//! state machine around an unacknowledged close.

use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use tempfile::TempDir;

use recon::inspection::{
    AccessProfile, AcquisitionMode, Capability, CloseOutcome, Incarnation, InspectionErrorCode,
    MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Op, OpenRequest, Reconciliation, SessionId,
    SessionRecord, SessionState, SessionStore, Stage, close, current_incarnation,
    decode_frame_length, encode_frame, native_request, open, reconcile,
};

const PROTOCOL: u32 = 1;
const HELPER_BUILD: &str = "recon-native-1";
const SESSION_A: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
const SESSION_B: &str = "1122334455667788990011223344aabb";
const CAPABILITY_A: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const CAPABILITY_B: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

// --- scripted helper ---

/// What the scripted helper does with one request.
enum Reply {
    /// Answer with a well-formed frame carrying this object.
    Json(Value),
    /// Answer with exactly these bytes, framing mistakes included.
    Raw(Vec<u8>),
    /// Wait, then answer. Used to let a read deadline expire with work still dispatched.
    Delayed(Duration, Value),
    /// Close the connection without answering, like a helper that died mid-close.
    Hangup,
}

type Handler = Arc<dyn Fn(&Map<String, Value>) -> Reply + Send + Sync>;

/// A helper endpoint bound in this process: owner-only, inside the store's 0700 endpoint
/// directory, so the kernel reports this test process as the peer.
struct Helper {
    requests: Arc<Mutex<Vec<Map<String, Value>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Helper {
    fn start(endpoint: &Path, handler: Handler) -> Self {
        let _ = fs::remove_file(endpoint);
        let listener = UnixListener::bind(endpoint).expect("bind the scripted helper endpoint");
        fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))
            .expect("make the endpoint owner-only");
        listener.set_nonblocking(true).expect("poll for connections");

        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => serve(stream, &handler, &requests),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })
        };
        Self { requests, stop, thread: Some(thread) }
    }

    fn requests(&self) -> Vec<Map<String, Value>> {
        self.requests.lock().expect("read the recorded requests").clone()
    }

    fn request_count(&self) -> usize {
        self.requests.lock().expect("read the recorded requests").len()
    }

    /// Stop answering. The endpoint file stays, which is what an exited helper leaves behind.
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(
    mut stream: UnixStream,
    handler: &Handler,
    requests: &Arc<Mutex<Vec<Map<String, Value>>>>,
) {
    stream.set_nonblocking(false).expect("block while serving one request");
    stream.set_read_timeout(Some(Duration::from_secs(5))).expect("bound the scripted read");

    let mut header = [0_u8; 4];
    if stream.read_exact(&mut header).is_err() {
        return;
    }
    let length = u32::from_be_bytes(header) as usize;
    let mut payload = vec![0_u8; length];
    if stream.read_exact(&mut payload).is_err() {
        return;
    }
    let request: Map<String, Value> =
        serde_json::from_slice(&payload).expect("the host sent one JSON object");
    let reply = handler(&request);
    requests.lock().expect("record the request").push(request);

    match reply {
        Reply::Json(value) => {
            let _ = stream.write_all(&frame(&value));
        }
        Reply::Raw(bytes) => {
            let _ = stream.write_all(&bytes);
        }
        Reply::Delayed(delay, value) => {
            sleep(delay);
            let _ = stream.write_all(&frame(&value));
        }
        Reply::Hangup => {}
    }
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialize the scripted reply");
    let mut framed = Vec::with_capacity(4 + body.len());
    framed.extend_from_slice(&(body.len() as u32).to_be_bytes());
    framed.extend_from_slice(&body);
    framed
}

// --- fixtures ---

struct Fixture {
    _root: TempDir,
    store: SessionStore,
    incarnation: Incarnation,
    endpoint: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("a private descriptor root");
        let store = SessionStore::open(root.path().join("inspection")).expect("open the store");
        let incarnation = current_incarnation().expect("this process's own incarnation");
        let endpoint = store.endpoint_path(incarnation).expect("an endpoint path");
        Self { _root: root, store, incarnation, endpoint }
    }

    fn open_request(&self, mode: AcquisitionMode) -> OpenRequest<'_> {
        OpenRequest {
            socket_path: &self.endpoint,
            bundle_path: Path::new("/System/Applications/TextEdit.app"),
            bundle_id: "com.apple.TextEdit",
            incarnation: self.incarnation,
            mode,
            profile: AccessProfile::Standard,
            helper_build: HELPER_BUILD,
            protocol: PROTOCOL,
        }
    }
}

/// The helper's successful `open` answer.
fn open_reply(incarnation: Incarnation, session: &str, capability: &str, reused: bool) -> Value {
    json!({
        "ok": true,
        "protocol": PROTOCOL,
        "helperBuild": HELPER_BUILD,
        "sessionId": session,
        "capability": capability,
        "pid": incarnation.pid,
        "pidversion": incarnation.pidversion,
        "profile": "standard",
        "mode": "attach",
        "establishedMode": "attach",
        "reused": reused,
        "capabilities": ["windows", "tree", "node", "find", "fonts"],
        "effects": ["target-code-loaded"],
    })
}

fn always(reply: impl Fn(&Map<String, Value>) -> Reply + Send + Sync + 'static) -> Handler {
    Arc::new(reply)
}

fn record_for(
    incarnation: Incarnation,
    session: &str,
    capability: &str,
    state: SessionState,
) -> SessionRecord {
    SessionRecord {
        session_id: SessionId::parse(session).expect("a well-formed session id"),
        capability: Capability::parse(capability).expect("a well-formed capability"),
        bundle_path: PathBuf::from("/System/Applications/TextEdit.app"),
        bundle_id: "com.apple.TextEdit".to_owned(),
        incarnation,
        established_mode: AcquisitionMode::Launch,
        requested_mode: AcquisitionMode::Launch,
        profile: AccessProfile::Standard,
        helper_build: HELPER_BUILD.to_owned(),
        protocol: PROTOCOL,
        socket_path: PathBuf::from("/dev/null"),
        state,
        opened_at: now(),
        updated_at: now(),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs()
}

// --- framing and allocation limits ---

#[test]
fn an_announced_frame_over_the_cap_is_refused_from_its_header_alone() {
    let failure = decode_frame_length(u32::MAX.to_be_bytes(), MAX_RESPONSE_BYTES)
        .expect_err("a 4 GiB frame is refused");

    assert_eq!(failure.code(), InspectionErrorCode::Failed);
    assert_eq!(failure.stage(), Stage::Receive);
    assert!(failure.message().contains(&MAX_RESPONSE_BYTES.to_string()), "{failure}");

    // The cap itself is allowed, and an empty frame is not a frame.
    let at_cap = u32::try_from(MAX_RESPONSE_BYTES).expect("the cap fits a frame header");
    assert_eq!(
        decode_frame_length(at_cap.to_be_bytes(), MAX_RESPONSE_BYTES).unwrap(),
        MAX_RESPONSE_BYTES
    );
    assert!(decode_frame_length(0_u32.to_be_bytes(), MAX_RESPONSE_BYTES).is_err());
}

#[test]
fn an_oversized_response_never_reaches_a_buffer() {
    let fixture = Fixture::new();
    let announced = u32::MAX.to_be_bytes().to_vec();
    let _helper = Helper::start(&fixture.endpoint, always(move |_| Reply::Raw(announced.clone())));

    let started = Instant::now();
    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach))
        .expect_err("a 4 GiB response is refused");

    assert_eq!(failure.code(), InspectionErrorCode::Failed);
    assert_eq!(failure.stage(), Stage::Receive);
    // Refusal comes from the header, not from waiting for 4 GiB of body to arrive.
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    assert!(fixture.store.records().unwrap().is_empty(), "no session was recorded");
}

#[test]
fn an_oversized_request_is_refused_before_it_is_written() {
    let oversized = json!({ "padding": "x".repeat(MAX_REQUEST_BYTES) });
    let failure =
        encode_frame(&oversized, MAX_REQUEST_BYTES).expect_err("a 64 KiB+ request is refused");

    assert_eq!(failure.code(), InspectionErrorCode::InvalidArgument);
    assert_eq!(failure.stage(), Stage::Send);
    assert!(failure.message().contains(&MAX_REQUEST_BYTES.to_string()), "{failure}");

    // And through the live path: the helper is never given the chance to read it.
    let fixture = Fixture::new();
    let helper =
        Helper::start(&fixture.endpoint, always(move |_| Reply::Json(json!({ "ok": true }))));
    let record = record_for(fixture.incarnation, SESSION_A, CAPABILITY_A, SessionState::Active);
    let record = SessionRecord { socket_path: fixture.endpoint.clone(), ..record };
    fixture.store.insert(&record).expect("record the session");

    let failure = native_request(
        &fixture.store,
        &record.session_id,
        Op::Tree,
        json!({ "padding": "x".repeat(MAX_REQUEST_BYTES) }),
    )
    .expect_err("the request exceeds the cap");

    assert_eq!(failure.code(), InspectionErrorCode::InvalidArgument);
    assert_eq!(helper.request_count(), 0, "nothing was sent");
    // A request that never left the host leaves no read outstanding.
    assert!(
        fixture.store.unresolved_execution(fixture.incarnation).unwrap().is_none(),
        "the read guard was released"
    );
}

#[test]
fn a_malformed_response_is_a_typed_failure() {
    for (label, body) in [
        ("a JSON array", frame(&json!([1, 2, 3]))),
        ("a bare number", frame(&json!(7))),
        ("an object without ok", frame(&json!({ "sessionId": SESSION_A }))),
        ("invalid JSON", {
            let mut raw = 5_u32.to_be_bytes().to_vec();
            raw.extend_from_slice(b"{not}");
            raw
        }),
        ("a truncated body", {
            let mut raw = 64_u32.to_be_bytes().to_vec();
            raw.extend_from_slice(b"{\"ok\":true}");
            raw
        }),
    ] {
        let fixture = Fixture::new();
        let _helper = Helper::start(&fixture.endpoint, always(move |_| Reply::Raw(body.clone())));

        let failure = match open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)) {
            Err(failure) => failure,
            Ok(outcome) => panic!("{label} must not establish a session: {outcome:?}"),
        };
        assert_eq!(failure.code(), InspectionErrorCode::Failed, "{label}: {failure}");
        assert!(fixture.store.records().unwrap().is_empty(), "{label} recorded a session");
    }
}

#[test]
fn a_helper_failure_envelope_keeps_its_published_classification() {
    let fixture = Fixture::new();
    let _helper = Helper::start(
        &fixture.endpoint,
        always(|_| {
            Reply::Json(json!({
                "ok": false,
                "error": { "code": "INSPECTION_BUSY", "message": "another session holds this process" },
            }))
        }),
    );

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("the helper refused");
    assert_eq!(failure.code(), InspectionErrorCode::Busy);
    assert!(failure.message().contains("another session"), "{failure}");
}

#[test]
fn an_unpublished_helper_code_is_not_passed_through() {
    let fixture = Fixture::new();
    let _helper = Helper::start(
        &fixture.endpoint,
        always(|_| {
            Reply::Json(json!({
                "ok": false,
                "error": { "code": "TOTALLY_FINE", "message": "invented classification" },
            }))
        }),
    );

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("the helper refused");
    assert_eq!(failure.code(), InspectionErrorCode::Failed);
}

// --- descriptor permissions and identity ---

#[test]
fn the_descriptor_root_and_its_records_are_owner_only() {
    let fixture = Fixture::new();
    let mode = |path: &Path| fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777;

    assert_eq!(mode(fixture.store.root()), 0o700);
    assert_eq!(mode(&fixture.store.root().join("sessions")), 0o700);
    assert_eq!(mode(fixture.endpoint.parent().unwrap()), 0o700);

    let record = record_for(fixture.incarnation, SESSION_A, CAPABILITY_A, SessionState::Active);
    fixture.store.insert(&record).expect("record the session");
    let descriptor = fixture.store.root().join("sessions").join(format!("{SESSION_A}.json"));
    assert_eq!(mode(&descriptor), 0o600);

    // A descriptor anyone else can read is refused rather than trusted.
    fs::set_permissions(&descriptor, fs::Permissions::from_mode(0o644)).unwrap();
    let failure =
        fixture.store.records().expect_err("a group/other-readable descriptor is refused");
    assert_eq!(failure.code(), InspectionErrorCode::Unavailable);
    assert!(failure.message().contains("0644"), "{failure}");
}

#[test]
fn a_descriptor_root_others_can_reach_is_refused() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("inspection");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

    let failure = SessionStore::open(&path).expect_err("a world-readable root is refused");
    assert_eq!(failure.code(), InspectionErrorCode::Unavailable);
    assert_eq!(failure.stage(), Stage::Store);
}

#[test]
fn an_endpoint_that_is_not_owner_only_is_never_connected_to() {
    let fixture = Fixture::new();
    let helper =
        Helper::start(&fixture.endpoint, always(move |_| Reply::Json(json!({ "ok": true }))));
    fs::set_permissions(&fixture.endpoint, fs::Permissions::from_mode(0o666)).unwrap();

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("a world-writable endpoint is refused");
    assert_eq!(failure.code(), InspectionErrorCode::Unavailable);
    assert_eq!(failure.stage(), Stage::Connect);
    assert_eq!(helper.request_count(), 0, "nothing was sent to it");
}

#[test]
fn a_peer_that_is_not_the_bound_incarnation_is_refused_before_anything_is_sent() {
    let fixture = Fixture::new();
    let helper =
        Helper::start(&fixture.endpoint, always(move |_| Reply::Json(json!({ "ok": true }))));

    let impostor =
        Incarnation::new(fixture.incarnation.pid, fixture.incarnation.pidversion.wrapping_add(1));
    let request =
        OpenRequest { incarnation: impostor, ..fixture.open_request(AcquisitionMode::Attach) };

    let failure = open(&fixture.store, &request).expect_err("the peer is a different incarnation");
    assert_eq!(failure.code(), InspectionErrorCode::StaleSession);
    assert_eq!(failure.stage(), Stage::Peer);
    assert_eq!(helper.request_count(), 0, "no request crossed to the wrong process");
}

#[test]
fn generated_identities_are_unguessable_and_well_formed() {
    let ids: Vec<SessionId> =
        (0..64).map(|_| SessionId::generate().expect("kernel randomness")).collect();
    let capabilities: Vec<Capability> =
        (0..64).map(|_| Capability::generate().expect("kernel randomness")).collect();

    for (id, capability) in ids.iter().zip(&capabilities) {
        assert_eq!(id.as_str().len(), 32);
        assert_eq!(capability.expose().len(), 64);
        assert_eq!(SessionId::parse(id.as_str()).unwrap(), *id);
        assert_eq!(Capability::parse(capability.expose()).unwrap(), *capability);
    }

    let distinct_ids: std::collections::BTreeSet<&str> =
        ids.iter().map(SessionId::as_str).collect();
    let distinct_capabilities: std::collections::BTreeSet<&str> =
        capabilities.iter().map(Capability::expose).collect();
    assert_eq!(distinct_ids.len(), ids.len(), "identifiers are never repeated");
    assert_eq!(distinct_capabilities.len(), capabilities.len());
}

#[test]
fn a_session_id_from_the_wire_cannot_name_a_path() {
    for hostile in [
        "../../etc/passwd",
        "..",
        "",
        "ABCDEF0123456789abcdef0123456789",
        "zz1e2d3c4b5a69788796a5b4c3d2e1f0",
    ] {
        assert!(SessionId::parse(hostile).is_err(), "{hostile} must be refused");
    }
    assert!(SessionId::parse(SESSION_A).is_ok());

    let fixture = Fixture::new();
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |request| {
            let mut reply = open_reply(
                Incarnation::new(
                    request["expectedPid"].as_i64().unwrap() as i32,
                    request["expectedPidversion"].as_u64().unwrap() as u32,
                ),
                SESSION_A,
                CAPABILITY_A,
                false,
            );
            reply["sessionId"] = json!("../../../escape");
            Reply::Json(reply)
        }),
    );

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach))
        .expect_err("a path-shaped session id is refused");
    assert_eq!(failure.code(), InspectionErrorCode::InvalidArgument);
    assert!(fixture.store.records().unwrap().is_empty(), "nothing was written");
}

#[test]
fn a_capability_is_never_printed() {
    let capability = Capability::parse(CAPABILITY_A).unwrap();
    assert!(!format!("{capability:?}").contains(CAPABILITY_A));
    assert!(!format!("{capability}").contains(CAPABILITY_A));
    assert_eq!(capability.expose(), CAPABILITY_A);

    let record = record_for(Incarnation::new(42, 7), SESSION_A, CAPABILITY_A, SessionState::Active);
    let rendered = format!("{record:?}");
    assert!(!rendered.contains(CAPABILITY_A), "{rendered}");
    assert!(rendered.contains(SESSION_A), "the session id is not a secret");

    // A failure that mentions the session must not carry its capability.
    let fixture = Fixture::new();
    let stored = SessionRecord { socket_path: fixture.endpoint.clone(), ..record };
    fixture.store.insert(&stored).unwrap();
    let mut helper = Helper::start(&fixture.endpoint, always(|_| Reply::Hangup));
    helper.stop();

    let failure = native_request(&fixture.store, &stored.session_id, Op::Tree, json!({}))
        .expect_err("the helper is gone");
    assert!(!failure.to_string().contains(CAPABILITY_A), "{failure}");
}

#[test]
fn every_request_carries_this_process_and_the_negotiated_build() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let helper = Helper::start(
        &fixture.endpoint,
        always(move |_| Reply::Json(open_reply(incarnation, SESSION_A, CAPABILITY_A, false))),
    );

    open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)).expect("open a session");

    let requests = helper.requests();
    let request = requests.first().expect("the open request");
    assert_eq!(request["op"], json!("open"));
    assert_eq!(request["protocol"], json!(PROTOCOL));
    assert_eq!(request["helperBuild"], json!(HELPER_BUILD));
    assert_eq!(request["clientPid"], json!(incarnation.pid));
    assert_eq!(request["clientPidversion"], json!(incarnation.pidversion));
    assert!(request["clientUid"].is_u64(), "the client uid is sent");
    assert_eq!(request["expectedPid"], json!(incarnation.pid));
    assert_eq!(request["expectedPidversion"], json!(incarnation.pidversion));
    // A first open presents no capability: there is none to present.
    assert!(request.get("capability").is_none());
}

#[test]
fn a_mismatched_helper_build_fails_explicitly() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |_| {
            let mut reply = open_reply(incarnation, SESSION_A, CAPABILITY_A, false);
            reply["helperBuild"] = json!("recon-native-0");
            Reply::Json(reply)
        }),
    );

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("a different build cannot serve this session");
    assert_eq!(failure.code(), InspectionErrorCode::ProtocolMismatch, "{failure}");
    assert!(fixture.store.records().unwrap().is_empty(), "no session was recorded");
}

#[test]
fn a_session_the_host_cannot_record_is_revoked_rather_than_orphaned() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let helper = Helper::start(
        &fixture.endpoint,
        always(move |request| {
            if request.get("op").and_then(Value::as_str) == Some("close") {
                Reply::Json(json!({ "ok": true, "revoked": true, "wasActive": true }))
            } else {
                Reply::Json(open_reply(incarnation, SESSION_A, CAPABILITY_A, false))
            }
        }),
    );

    // The helper will acknowledge the open, but the host cannot write the descriptor: the sessions
    // directory is read-only. Without a recorded session there is no capability to close with, so
    // the open must revoke the just-established session on the helper instead of leaking it.
    let sessions = fixture.store.root().join("sessions");
    fs::set_permissions(&sessions, fs::Permissions::from_mode(0o500))
        .expect("seal the sessions dir");

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach))
        .expect_err("a session that cannot be recorded is not established");
    assert_eq!(failure.stage(), Stage::Store);

    fs::set_permissions(&sessions, fs::Permissions::from_mode(0o700)).expect("restore the dir");
    assert!(fixture.store.records().unwrap().is_empty(), "no session was recorded");
    assert!(
        helper
            .requests()
            .iter()
            .any(|request| request.get("op").and_then(Value::as_str) == Some("close")),
        "the orphaned session must be revoked on the helper"
    );
}

#[test]
fn a_handshake_answering_for_another_process_cannot_establish_a_session() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |_| {
            let mut reply = open_reply(incarnation, SESSION_A, CAPABILITY_A, false);
            reply["pidversion"] = json!(incarnation.pidversion.wrapping_add(1));
            Reply::Json(reply)
        }),
    );

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach))
        .expect_err("the reported incarnation differs");
    assert_eq!(failure.code(), InspectionErrorCode::Failed);
    assert_eq!(failure.stage(), Stage::Handshake);
}

// --- session state ---

#[test]
fn opening_records_one_active_session_for_the_process() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |_| Reply::Json(open_reply(incarnation, SESSION_A, CAPABILITY_A, false))),
    );

    let outcome =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)).expect("open");
    assert!(!outcome.reused);
    assert_eq!(outcome.record.state, SessionState::Active);
    assert_eq!(outcome.record.session_id.as_str(), SESSION_A);
    assert_eq!(outcome.record.incarnation, incarnation);
    assert_eq!(outcome.record.bundle_id, "com.apple.TextEdit");
    assert_eq!(outcome.record.protocol, PROTOCOL);
    assert_eq!(outcome.record.socket_path, fixture.endpoint);
    assert!(outcome.response.get("capability").is_none());

    // It round-trips through the descriptor, capability included.
    let reloaded = fixture.store.record(&outcome.record.session_id).unwrap().expect("a descriptor");
    assert_eq!(reloaded, outcome.record);
    assert_eq!(reloaded.capability.expose(), CAPABILITY_A);
}

#[test]
fn a_second_session_never_displaces_a_live_one() {
    let fixture = Fixture::new();
    let first = record_for(fixture.incarnation, SESSION_A, CAPABILITY_A, SessionState::Active);
    fixture.store.insert(&first).expect("record the first session");

    let second = record_for(fixture.incarnation, SESSION_B, CAPABILITY_B, SessionState::Active);
    let failure = fixture.store.insert(&second).expect_err("the process already has a session");
    assert_eq!(failure.code(), InspectionErrorCode::Busy);
    assert!(failure.message().contains(SESSION_A), "{failure}");

    // An uncertain close is just as live.
    let unconfirmed = SessionRecord { state: SessionState::CloseUnconfirmed, ..first.clone() };
    fixture.store.save(&unconfirmed).unwrap();
    assert_eq!(
        fixture.store.insert(&second).expect_err("still live").code(),
        InspectionErrorCode::Busy
    );

    // A closed one does not block the next session.
    fixture.store.save(&SessionRecord { state: SessionState::Closed, ..first }).unwrap();
    fixture.store.insert(&second).expect("the process is free again");
    assert_eq!(fixture.store.records().unwrap().len(), 2);
}

#[test]
fn reuse_keeps_the_mode_that_established_the_session() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let seen = Arc::new(Mutex::new(Vec::<Map<String, Value>>::new()));
    let recorder = Arc::clone(&seen);
    let _helper = Helper::start(
        &fixture.endpoint,
        Arc::new(move |request: &Map<String, Value>| {
            recorder.lock().unwrap().push(request.clone());
            let reused = request.contains_key("sessionId");
            let mut reply = open_reply(incarnation, SESSION_A, CAPABILITY_A, reused);
            reply["establishedMode"] = json!("launch");
            Reply::Json(reply)
        }),
    );

    let established =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Launch)).expect("open");
    assert_eq!(established.record.established_mode, AcquisitionMode::Launch);

    let reused =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing)).expect("reuse");
    assert!(reused.reused);
    assert_eq!(reused.record.session_id, established.record.session_id);
    assert_eq!(reused.record.established_mode, AcquisitionMode::Launch, "history is not rewritten");
    assert_eq!(reused.record.requested_mode, AcquisitionMode::Existing);
    assert_eq!(fixture.store.records().unwrap().len(), 1, "no second descriptor");

    // Reuse is authenticated with the capability the host already holds.
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests[1]["sessionId"], json!(SESSION_A));
    assert_eq!(requests[1]["capability"], json!(CAPABILITY_A));
}

#[test]
fn an_unexpected_new_session_cannot_replace_a_recorded_one() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |request| {
            let reused = request.contains_key("sessionId");
            // The helper answers with a different session than the host holds.
            Reply::Json(open_reply(incarnation, SESSION_B, CAPABILITY_B, reused))
        }),
    );
    let existing = SessionRecord {
        socket_path: fixture.endpoint.clone(),
        ..record_for(incarnation, SESSION_A, CAPABILITY_A, SessionState::Active)
    };
    fixture.store.insert(&existing).unwrap();

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("the helper named another session");
    assert_eq!(failure.code(), InspectionErrorCode::Failed);
    let records = fixture.store.records().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id.as_str(), SESSION_A, "the recorded session is untouched");
}

#[test]
fn a_lost_close_acknowledgment_keeps_the_session_recoverable() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(&fixture.endpoint, always(|_| Reply::Hangup));
    let record = SessionRecord {
        socket_path: fixture.endpoint.clone(),
        ..record_for(incarnation, SESSION_A, CAPABILITY_A, SessionState::Active)
    };
    fixture.store.insert(&record).unwrap();

    let outcome = close(&fixture.store, &record.session_id).expect("close reports an outcome");
    let CloseOutcome::Unconfirmed(cause) = outcome else {
        panic!("a dropped connection is not a confirmed close: {outcome:?}");
    };
    assert!(cause.leaves_outcome_unknown(), "{cause}");

    // The descriptor and its exact identity are retained, not discarded.
    let retained = fixture.store.record(&record.session_id).unwrap().expect("the descriptor stays");
    assert_eq!(retained.state, SessionState::CloseUnconfirmed);
    assert_eq!(retained.incarnation, incarnation);
    assert_eq!(retained.capability.expose(), CAPABILITY_A);

    // Native reads are refused while revocation is uncertain.
    let refused = native_request(&fixture.store, &record.session_id, Op::Tree, json!({}))
        .expect_err("reads wait for certainty");
    assert_eq!(refused.code(), InspectionErrorCode::Busy);
}

#[test]
fn an_uncertain_close_is_resolved_only_by_authenticated_reconciliation() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let still_active = Arc::new(AtomicBool::new(true));
    let reported = Arc::clone(&still_active);
    let _helper = Helper::start(
        &fixture.endpoint,
        Arc::new(move |request: &Map<String, Value>| match request["op"].as_str() {
            Some("reconcile") => Reply::Json(json!({
                "ok": true,
                "protocol": PROTOCOL,
                "helperBuild": HELPER_BUILD,
                "active": reported.load(Ordering::Relaxed),
                "sessionId": SESSION_A,
                "outstandingRead": false,
            })),
            Some("close") => Reply::Hangup,
            _ => Reply::Json(open_reply(incarnation, SESSION_B, CAPABILITY_B, false)),
        }),
    );
    let record = SessionRecord {
        socket_path: fixture.endpoint.clone(),
        ..record_for(incarnation, SESSION_A, CAPABILITY_A, SessionState::Active)
    };
    fixture.store.insert(&record).unwrap();

    // The close acknowledgment is lost.
    assert!(matches!(
        close(&fixture.store, &record.session_id).unwrap(),
        CloseOutcome::Unconfirmed(_)
    ));

    // While the helper still reports the session active, no replacement session is opened.
    let refused = open(&fixture.store, &fixture.open_request(AcquisitionMode::Existing))
        .expect_err("the old session is still active");
    assert_eq!(refused.code(), InspectionErrorCode::Busy);
    assert!(refused.message().contains(SESSION_A), "{refused}");
    assert!(matches!(
        reconcile(&fixture.store, &record.session_id).unwrap(),
        Reconciliation::Active
    ));

    // Once bootstrap confirms revocation, the uncertainty resolves and a repeat close is idempotent.
    still_active.store(false, Ordering::Relaxed);
    let outcome = close(&fixture.store, &record.session_id).expect("reconciled close");
    assert!(
        matches!(outcome, CloseOutcome::ReconciledClosed { process_exited: false }),
        "{outcome:?}"
    );
    assert_eq!(
        fixture.store.record(&record.session_id).unwrap().unwrap().state,
        SessionState::Closed
    );
    assert!(matches!(
        close(&fixture.store, &record.session_id).unwrap(),
        CloseOutcome::AlreadyClosed
    ));

    // And the process is free to hold a new session.
    let opened =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)).expect("reopen");
    assert_eq!(opened.record.session_id.as_str(), SESSION_B);
}

#[test]
fn unreachable_bootstrap_preserves_the_unknown_outcome() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let mut helper = Helper::start(&fixture.endpoint, always(|_| Reply::Hangup));
    let record = SessionRecord {
        socket_path: fixture.endpoint.clone(),
        ..record_for(incarnation, SESSION_A, CAPABILITY_A, SessionState::CloseUnconfirmed)
    };
    fixture.store.insert(&record).unwrap();
    // The endpoint file remains, but nothing answers on it.
    helper.stop();

    let outcome =
        reconcile(&fixture.store, &record.session_id).expect("reconcile reports an outcome");
    assert!(matches!(outcome, Reconciliation::Unreachable(_)), "{outcome:?}");

    let failure = open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach))
        .expect_err("a replacement session is refused while revocation is unknown");
    assert_eq!(failure.code(), InspectionErrorCode::Unavailable);
    assert_eq!(
        fixture.store.record(&record.session_id).unwrap().unwrap().state,
        SessionState::CloseUnconfirmed,
        "the uncertainty is preserved"
    );

    let outcome =
        close(&fixture.store, &record.session_id).expect("a repeat close reports an outcome");
    assert!(matches!(outcome, CloseOutcome::Unconfirmed(_)), "{outcome:?}");
}

#[test]
fn an_acknowledged_close_and_an_already_closed_session_are_different_successes() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let _helper = Helper::start(
        &fixture.endpoint,
        always(move |request| match request["op"].as_str() {
            Some("close") => Reply::Json(json!({
                "ok": true,
                "protocol": PROTOCOL,
                "helperBuild": HELPER_BUILD,
                "revoked": true,
                "wasActive": true,
                "helperResident": true,
                "bootstrapActive": true,
            })),
            _ => Reply::Json(open_reply(incarnation, SESSION_A, CAPABILITY_A, false)),
        }),
    );

    let opened =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)).expect("open");
    let id = opened.record.session_id.clone();

    let outcome = close(&fixture.store, &id).expect("close");
    assert!(matches!(outcome, CloseOutcome::Acknowledged { was_active: true }), "{outcome:?}");
    assert!(matches!(close(&fixture.store, &id).unwrap(), CloseOutcome::AlreadyClosed));

    // A session this user never had is stale, not closed.
    let unknown = SessionId::parse(SESSION_B).unwrap();
    assert_eq!(
        close(&fixture.store, &unknown).expect_err("unknown session").code(),
        InspectionErrorCode::StaleSession
    );
}

#[test]
fn an_unresolved_read_blocks_later_reads_across_close_and_reopen() {
    let fixture = Fixture::new();
    let incarnation = fixture.incarnation;
    let sessions = Arc::new(Mutex::new(VecDeque::from([SESSION_B.to_owned()])));
    let outstanding = Arc::new(AtomicBool::new(true));
    let reported = Arc::clone(&outstanding);
    let _helper = Helper::start(
        &fixture.endpoint,
        Arc::new(move |request: &Map<String, Value>| match request["op"].as_str() {
            // A read the target's main thread never finishes inside the deadline.
            Some("tree") => {
                Reply::Delayed(Duration::from_millis(2_500), json!({ "ok": true, "nodes": [] }))
            }
            Some("close") => Reply::Json(json!({ "ok": true, "wasActive": true })),
            // The session stays open while its read is outstanding, which is how a caller can
            // learn the read finished.
            Some("reconcile") => Reply::Json(json!({
                "ok": true,
                "active": true,
                "outstandingRead": reported.load(Ordering::Relaxed),
            })),
            _ => {
                let next =
                    sessions.lock().unwrap().pop_front().unwrap_or_else(|| SESSION_B.to_owned());
                Reply::Json(open_reply(incarnation, &next, CAPABILITY_B, false))
            }
        }),
    );

    let record = SessionRecord {
        socket_path: fixture.endpoint.clone(),
        ..record_for(incarnation, SESSION_A, CAPABILITY_A, SessionState::Active)
    };
    fixture.store.insert(&record).unwrap();

    let started = Instant::now();
    let failure =
        native_request(&fixture.store, &record.session_id, Op::Tree, json!({ "depth": 3 }))
            .expect_err("the read deadline expires");
    assert_eq!(failure.code(), InspectionErrorCode::Timeout);
    assert!(failure.may_continue_in_target(), "the main thread may still be running it");
    assert!(started.elapsed() >= Duration::from_secs(2), "the deadline is two seconds");
    assert!(started.elapsed() < Duration::from_secs(4), "{:?}", started.elapsed());

    let guard =
        fixture.store.unresolved_execution(incarnation).unwrap().expect("a read is unresolved");
    assert_eq!(guard.op, "tree");
    assert_eq!(guard.session_id.as_ref(), Some(&record.session_id));

    // Closing and reopening the session does not clear the process-wide guard.
    assert!(matches!(
        close(&fixture.store, &record.session_id).unwrap(),
        CloseOutcome::Acknowledged { .. }
    ));
    let reopened =
        open(&fixture.store, &fixture.open_request(AcquisitionMode::Attach)).expect("reopen");
    let refused = native_request(&fixture.store, &reopened.record.session_id, Op::Node, json!({}))
        .expect_err("the earlier read has not resolved");
    assert_eq!(refused.code(), InspectionErrorCode::Busy);
    assert!(refused.may_continue_in_target());

    // Only the helper reporting the read finished releases it.
    reconcile(&fixture.store, &reopened.record.session_id).expect("reconcile");
    assert!(
        fixture.store.unresolved_execution(incarnation).unwrap().is_some(),
        "still outstanding"
    );
    outstanding.store(false, Ordering::Relaxed);
    reconcile(&fixture.store, &reopened.record.session_id).expect("reconcile");
    assert!(fixture.store.unresolved_execution(incarnation).unwrap().is_none(), "released");
}

#[test]
fn lifecycle_operations_are_not_native_reads() {
    let fixture = Fixture::new();
    let record = record_for(fixture.incarnation, SESSION_A, CAPABILITY_A, SessionState::Active);
    fixture.store.insert(&record).unwrap();

    for op in [Op::Open, Op::Close, Op::Reconcile] {
        let failure = native_request(&fixture.store, &record.session_id, op, json!({}))
            .expect_err("lifecycle work has its own path");
        assert_eq!(failure.code(), InspectionErrorCode::InvalidArgument);
    }
    assert_eq!(Op::Tree.deadline(), Duration::from_secs(2));
    assert_eq!(Op::Open.deadline(), Duration::from_secs(10));
}
