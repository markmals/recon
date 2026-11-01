import Darwin
import Foundation

/// One client conversation: the peer the kernel vouches for, one framed request, one answer.
struct ReconConnection: Sendable {
    /// A client that stops talking mid-frame must not hold a server thread.
    private static let transportTimeout = timeval(tv_sec: 10, tv_usec: 0)

    let descriptor: Int32

    func serve(_ router: ReconRouter) {
        defer { close(descriptor) }
        guard let peer = ReconProcessIncarnation.peer(of: descriptor), peer.uid == geteuid()
        else { return }
        applyTimeouts()
        let startedAt = ContinuousClock.now
        guard let header = receive(4) else { return }
        let size = ReconFrame.length([UInt8](header))
        guard ReconFrame.isValidRequest(size) else {
            return refuse("INVALID_ARGUMENT", "framing", "Request exceeds 64 KiB or is empty")
        }
        guard let body = receive(Int(size)) else { return }
        guard let request = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
        else {
            return refuse("INVALID_ARGUMENT", "json", "Request must be one JSON object")
        }
        guard claims(request, match: peer) else {
            return refuse("STALE_SESSION", "peer", "Peer process incarnation changed")
        }
        send(router.answer(request, startedAt: startedAt))
    }

    /// The peer the kernel named must be the process the request says it is, incarnation
    /// included: a pid alone is reused.
    private func claims(_ request: [String: Any], match peer: ReconProcessIncarnation) -> Bool {
        guard let pid = request["clientPid"] as? NSNumber, pid.int32Value == peer.pid,
            let pidversion = request["clientPidversion"] as? NSNumber,
            pidversion.uint32Value == peer.pidversion,
            let uid = request["clientUid"] as? NSNumber, uid.uint32Value == peer.uid
        else { return false }
        return true
    }

    private func applyTimeouts() {
        var timeout = Self.transportTimeout
        let size = socklen_t(MemoryLayout<timeval>.size)
        setsockopt(descriptor, SOL_SOCKET, SO_RCVTIMEO, &timeout, size)
        setsockopt(descriptor, SOL_SOCKET, SO_SNDTIMEO, &timeout, size)
    }

    private func refuse(_ code: String, _ stage: String, _ message: String) {
        send(ReconEnvelope.serialized(ReconEnvelope.error(code, stage, message)))
    }

    private func send(_ response: Data?) {
        var body = framed(response)
        var header = ReconFrame.header(UInt32(body.count))
        guard header.withUnsafeMutableBytes({ transfer($0, sending: true) }) else { return }
        _ = body.withUnsafeMutableBytes { transfer($0, sending: true) }
    }

    /// An answer that could not be built, or that does not fit the frame, is replaced rather
    /// than truncated: a client must never decode half an observation.
    private func framed(_ response: Data?) -> Data {
        guard let response, response.count <= ReconFrame.maximumResponseBytes else {
            let refusal = ReconEnvelope.error(
                "INSPECTION_UNAVAILABLE", "response-size", "Native response exceeds 8 MiB")
            return ReconEnvelope.serialized(refusal) ?? Data()
        }
        return response
    }

    private func receive(_ count: Int) -> Data? {
        var body = Data(count: count)
        let complete = body.withUnsafeMutableBytes { transfer($0, sending: false) }
        return complete ? body : nil
    }

    private func transfer(_ buffer: UnsafeMutableRawBufferPointer, sending: Bool) -> Bool {
        guard var cursor = buffer.baseAddress else { return true }
        var remaining = buffer.count
        while remaining > 0 {
            let moved =
                sending
                ? Darwin.send(descriptor, cursor, remaining, 0)
                : Darwin.recv(descriptor, cursor, remaining, 0)
            guard moved > 0 else {
                if moved < 0 && errno == EINTR { continue }
                return false
            }
            cursor += moved
            remaining -= moved
        }
        return true
    }
}
