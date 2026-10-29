//! Relating one native observation to one external accessibility observation of the same window.
//!
//! The two sources are read separately, through different mechanisms, over different intervals.
//! This module compares only the evidence each one reported and names the relationships that
//! evidence supports. It asserts no object identity: a candidate says two observations match on a
//! stated basis, which is why even a unique candidate carries its basis and its ambiguity.
//!
//! It works on the JSON both sources already produce — the helper's native tree response and
//! `recon ui ax tree`'s response — so the matching rules can be exercised without a target, a
//! session, or an accessibility grant.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// How far apart two normalized screen edges may be and still be read as the same edge, in
/// logical points. Proposal 0005 fixes this at one point on every edge.
pub(super) const EDGE_TOLERANCE_POINTS: f64 = 1.0;

/// A native object as this correlation refers to it. The handle is the helper's spelling,
/// preserved exactly; an accessibility path never substitutes for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeReference {
    pub(super) handle: String,
    pub(super) kind: String,
    /// The runtime class, which is absent when the record carried no `basic` facet.
    pub(super) class: Option<String>,
}

/// An accessibility element as this correlation refers to it. The path describes where the
/// element sat in this observation; it identifies a position, not an object to resolve later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ElementReference {
    pub(super) path: String,
    pub(super) role: Option<String>,
    pub(super) identifier: Option<String>,
}

/// What made two observations candidates for the same interface element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MatchBasis {
    /// The same non-empty accessibility identifier, observed on both interfaces.
    AccessibilityIdentifier,
    /// The same self-reported accessibility role and normalized screen bounds within
    /// [`EDGE_TOLERANCE_POINTS`] on every edge.
    RoleAndBounds,
}

/// One candidate relationship, which is evidence of a correspondence rather than a claim that the
/// two observations describe one object.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Candidate {
    pub(super) native: NativeReference,
    pub(super) accessibility: ElementReference,
    pub(super) basis: MatchBasis,
    /// Set when either side of this candidate also matched something else on the same basis.
    pub(super) ambiguous: bool,
}

/// How a native rectangle reached the top-left global screen space accessibility reports in.
/// Recon records what the helper used; `scale` is metadata about the display, never a factor
/// applied to logical points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScreenTransform {
    pub(super) display_id: Option<u64>,
    pub(super) scale: Option<f64>,
    /// The primary display's height in logical points, which is the whole difference between
    /// AppKit's bottom-left global origin and accessibility's top-left one.
    pub(super) primary_screen_height: Option<f64>,
}

/// The native side's own provenance, kept as that source reported it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeEvidence {
    pub(super) observation: Option<Value>,
    pub(super) coverage: Option<Value>,
    /// Every distinct transform the compared rectangles were normalized with: one per display the
    /// observed objects were on.
    pub(super) transforms: Vec<ScreenTransform>,
    /// Native objects this correlation could refer to, counting each object once.
    pub(super) observed: usize,
}

/// The accessibility side's own provenance. The external read states its completeness in the
/// response itself; its observation id and interval belong to the caller that performed it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AccessibilityEvidence {
    pub(super) truncated: bool,
    pub(super) narrowed: bool,
    /// Elements actually read, excluding repeated and unreadable placeholders.
    pub(super) observed: usize,
}

/// What one correlation found. Unmatched records on either side are a difference between two
/// evidence sources, not a defect and not a licence to synthesize the missing counterpart.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Correlation {
    pub(super) native: NativeEvidence,
    pub(super) accessibility: AccessibilityEvidence,
    pub(super) candidates: Vec<Candidate>,
    pub(super) unmatched_native: Vec<NativeReference>,
    pub(super) unmatched_accessibility: Vec<ElementReference>,
}

/// Relates the native tree response `native` to the accessibility tree response `accessibility`.
///
/// Both arguments are the responses the two sources produced for the same selected window; this
/// function neither performs nor validates either read. A source that reported an error, a
/// different window, or a different process is the caller's to reject before correlating.
pub(super) fn correlate(native: &Value, accessibility: &Value) -> Correlation {
    let natives = native_subjects(native);
    let elements = element_subjects(accessibility);
    let pairs = pair(&natives, &elements);

    let mut native_matches: HashMap<usize, usize> = HashMap::new();
    let mut element_matches: HashMap<usize, usize> = HashMap::new();
    for pair in &pairs {
        *native_matches.entry(pair.native).or_default() += 1;
        *element_matches.entry(pair.element).or_default() += 1;
    }

    let candidates = pairs
        .iter()
        .map(|pair| Candidate {
            native: natives[pair.native].reference.clone(),
            accessibility: elements[pair.element].reference.clone(),
            basis: pair.basis,
            ambiguous: native_matches[&pair.native] > 1 || element_matches[&pair.element] > 1,
        })
        .collect();

    Correlation {
        native: NativeEvidence {
            observation: native.get("observation").cloned(),
            coverage: native.get("coverage").cloned(),
            transforms: transforms(native),
            observed: natives.len(),
        },
        accessibility: AccessibilityEvidence {
            truncated: accessibility["truncated"] == Value::Bool(true),
            narrowed: accessibility["narrowed"] == Value::Bool(true),
            observed: elements.len(),
        },
        unmatched_native: natives
            .iter()
            .enumerate()
            .filter(|(index, _)| !native_matches.contains_key(index))
            .map(|(_, subject)| subject.reference.clone())
            .collect(),
        unmatched_accessibility: elements
            .iter()
            .enumerate()
            .filter(|(index, _)| !element_matches.contains_key(index))
            .map(|(_, subject)| subject.reference.clone())
            .collect(),
        candidates,
    }
}

// ------------------------------------------------------------------ matching

/// A rectangle as its four edges, which is what the one-point tolerance applies to: two
/// rectangles whose origins and sizes are each within a point can still have edges two points
/// apart.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edges {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Edges {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { left: x, top: y, right: x + width, bottom: y + height }
    }

    fn within(self, other: Self, tolerance: f64) -> bool {
        (self.left - other.left).abs() <= tolerance
            && (self.top - other.top).abs() <= tolerance
            && (self.right - other.right).abs() <= tolerance
            && (self.bottom - other.bottom).abs() <= tolerance
    }
}

/// What one source reported about one object, reduced to what the two sources can be compared on.
struct Subject<R> {
    reference: R,
    identifier: Option<String>,
    role: Option<String>,
    bounds: Option<Edges>,
}

struct Pairing {
    native: usize,
    element: usize,
    basis: MatchBasis,
}

fn pair(
    natives: &[Subject<NativeReference>],
    elements: &[Subject<ElementReference>],
) -> Vec<Pairing> {
    let mut by_identifier: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_role: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, subject) in elements.iter().enumerate() {
        if let Some(identifier) = subject.identifier.as_deref() {
            by_identifier.entry(identifier).or_default().push(index);
        }
        if let Some(role) = subject.role.as_deref() {
            by_role.entry(role).or_default().push(index);
        }
    }

    let mut pairs: Vec<Pairing> = Vec::new();
    for (native, subject) in natives.iter().enumerate() {
        let Some(identifier) = subject.identifier.as_deref() else { continue };
        for &element in by_identifier.get(identifier).into_iter().flatten() {
            pairs.push(Pairing { native, element, basis: MatchBasis::AccessibilityIdentifier });
        }
    }

    // Role and bounds are the weaker basis, so they relate only what identifiers did not.
    let identified_natives: HashSet<usize> = pairs.iter().map(|pair| pair.native).collect();
    let identified_elements: HashSet<usize> = pairs.iter().map(|pair| pair.element).collect();
    for (native, subject) in natives.iter().enumerate() {
        if identified_natives.contains(&native) {
            continue;
        }
        let (Some(role), Some(bounds)) = (subject.role.as_deref(), subject.bounds) else {
            continue;
        };
        for &element in by_role.get(role).into_iter().flatten() {
            let candidate = &elements[element];
            // Two different non-empty accessibility identifiers are evidence against a
            // relationship, not the absence of evidence for one.
            if identified_elements.contains(&element)
                || (subject.identifier.is_some() && candidate.identifier.is_some())
            {
                continue;
            }
            if candidate.bounds.is_some_and(|other| bounds.within(other, EDGE_TOLERANCE_POINTS)) {
                pairs.push(Pairing { native, element, basis: MatchBasis::RoleAndBounds });
            }
        }
    }
    pairs.sort_by_key(|pair| (pair.native, pair.element));
    pairs
}

// --------------------------------------------------------- native observation

/// Every record one native tree response returned: the window the walk was scoped to, then the
/// objects of the walk itself. Both are observed native objects, and a window-scoped read puts
/// the window in its own array.
fn records(native: &Value) -> impl Iterator<Item = &Value> {
    let roots = native["windowRoots"].as_array().into_iter().flatten();
    roots.chain(native["nodes"].as_array().into_iter().flatten())
}

fn native_subjects(native: &Value) -> Vec<Subject<NativeReference>> {
    let mut subjects: Vec<Subject<NativeReference>> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for node in records(native) {
        // A record Recon cannot refer back to is not a reference it can return.
        let (Some(handle), Some(kind)) = (text(&node["handle"]), text(&node["kind"])) else {
            continue;
        };
        let basic = &node["basic"];
        let subject = Subject {
            reference: NativeReference {
                handle: handle.clone(),
                kind,
                class: text(&basic["class"]),
            },
            identifier: observed(&basic["accessibilityIdentifier"]),
            role: observed(&basic["accessibilityRole"]),
            bounds: screen_bounds(&basic["normalizedFrame"]["screenTopLeft"]),
        };
        // The same object can be emitted again as a repeated reference within one walk. It is one
        // object, so it is one subject, holding whatever evidence the records carried.
        match seen.get(&handle) {
            Some(&index) => absorb(&mut subjects[index], subject),
            None => {
                seen.insert(handle, subjects.len());
                subjects.push(subject);
            }
        }
    }
    subjects
}

fn absorb(subject: &mut Subject<NativeReference>, other: Subject<NativeReference>) {
    subject.reference.class = subject.reference.class.take().or(other.reference.class);
    subject.identifier = subject.identifier.take().or(other.identifier);
    subject.role = subject.role.take().or(other.role);
    subject.bounds = subject.bounds.or(other.bounds);
}

/// The rectangle a native record normalized into the space accessibility reports bounds in.
///
/// A rectangle that says it is in another space, orientation, or unit is left alone: reading
/// AppKit's bottom-left origin or a backing-pixel rectangle as if it were this one would invent a
/// position the target never reported.
fn screen_bounds(rect: &Value) -> Option<Edges> {
    if rect["status"] != "value"
        || rect["space"] != "global-screen"
        || rect["orientation"] != "top-left"
        || rect["units"] != "logical-points"
    {
        return None;
    }
    Some(Edges::new(
        number(&rect["x"])?,
        number(&rect["y"])?,
        number(&rect["width"])?,
        number(&rect["height"])?,
    ))
}

fn transforms(native: &Value) -> Vec<ScreenTransform> {
    let mut transforms: Vec<ScreenTransform> = Vec::new();
    for node in records(native) {
        let reported = &node["basic"]["normalizedFrame"]["transform"];
        if reported["status"] != "value" {
            continue;
        }
        let transform = ScreenTransform {
            display_id: reported["displayId"].as_u64(),
            scale: number(&reported["scale"]),
            primary_screen_height: number(&reported["primaryScreenHeight"]),
        };
        if !transforms.contains(&transform) {
            transforms.push(transform);
        }
    }
    transforms
}

// -------------------------------------------------- accessibility observation

fn element_subjects(accessibility: &Value) -> Vec<Subject<ElementReference>> {
    let root = &accessibility["root"];
    let mut subjects = Vec::new();
    visit(root, text(&root["role"]).unwrap_or_else(|| "element".to_owned()), &mut subjects);
    subjects
}

fn visit(node: &Value, path: String, subjects: &mut Vec<Subject<ElementReference>>) {
    // A repeated element is one already read elsewhere in this tree, and a failed one was never
    // read: neither is a separate observation to relate to anything.
    if !node.is_object() || node.get("repeated").is_some() || node.get("error").is_some() {
        return;
    }
    let role = text(&node["role"]);
    subjects.push(Subject {
        reference: ElementReference {
            path: path.clone(),
            role: role.clone(),
            identifier: text(&node["identifier"]),
        },
        identifier: text(&node["identifier"]),
        role,
        bounds: element_bounds(&node["frame"]),
    });
    for (index, child) in node["children"].as_array().into_iter().flatten().enumerate() {
        let name = text(&child["role"]).unwrap_or_else(|| "element".to_owned());
        visit(child, format!("{path}/{name}[{index}]"), subjects);
    }
}

/// An element's frame, which accessibility reports in global screen points with the origin at the
/// primary display's top left.
fn element_bounds(frame: &Value) -> Option<Edges> {
    Some(Edges::new(
        number(&frame["x"])?,
        number(&frame["y"])?,
        number(&frame["width"])?,
        number(&frame["height"])?,
    ))
}

// ------------------------------------------------------------------- reading

/// A non-empty string. An empty accessibility identifier is AppKit's way of reporting no
/// identifier at all, and matching two of them would relate every unidentified object.
fn text(value: &Value) -> Option<String> {
    value.as_str().filter(|text| !text.is_empty()).map(str::to_owned)
}

/// One observation the helper wrote as a value-or-status field.
fn observed(field: &Value) -> Option<String> {
    if field["status"] != "value" {
        return None;
    }
    text(&field["value"])
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}
