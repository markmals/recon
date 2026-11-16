use std::collections::HashSet;
use std::time::{Duration, Instant};

use objc2_application_services::AXError;
use objc2_core_foundation::CFString;
use serde_json::json;

use recon_core::error::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, NotFound,
    ReleaseState,
};
use recon_core::process::Pid;
use recon_input_macos::request::ElementSelector;
use recon_window_server_macos::window::{Bounds, Window, WindowId};

use crate::element::{Accessibility, Element, MESSAGING_TIMEOUT_SECONDS, application_element};
use crate::tree::{ElementAttributes, ElementChildren, ElementSource, MAX_DEPTH, ReadFailure};

#[derive(Clone, Debug)]
pub struct ResolvedElement {
    pub element: Element,
    pub role: Option<String>,
    pub title: Option<String>,
    pub identifier: Option<String>,
    pub enabled: bool,
    pub actions: Vec<String>,
    pub frame: Bounds,
}

impl ResolvedElement {
    pub fn advertises_press(&self) -> bool {
        self.actions.iter().any(|action| action == "AXPress")
    }
}

#[derive(Debug)]
struct Match<E> {
    element: E,
    role: Option<String>,
    title: Option<String>,
    identifier: Option<String>,
    enabled: Option<bool>,
    actions: Vec<String>,
    frame: Option<Bounds>,
}

impl From<Match<Element>> for ResolvedElement {
    fn from(matched: Match<Element>) -> Self {
        Self {
            element: matched.element,
            role: matched.role,
            title: matched.title,
            identifier: matched.identifier,
            enabled: matched.enabled == Some(true),
            actions: matched.actions,
            frame: matched.frame.unwrap_or(Bounds { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }),
        }
    }
}

/// What the caller resolved an element for. Acting on the element requires it to report that it
/// is enabled; taking only a point on it does not. A unique, completely read match inside a
/// finite, non-empty frame within the window is required either way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvePurpose {
    /// `ui click` acts on the element, so a disabled or unknown enabled state is unactionable.
    Click,
    /// `ui scroll` takes only the event location from the element.
    Scroll,
}

pub fn resolve(
    pid: Pid,
    window: &Window,
    selector: &ElementSelector,
    purpose: ResolvePurpose,
) -> Result<ResolvedElement, Failure> {
    let application = application_element(pid)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let root = Element::new(crate::window_element::window_element_within(
        &application,
        pid,
        window.id,
        Some(deadline),
    )?);
    let mut source = Accessibility::new();
    let mut seen = HashSet::new();
    let mut matches = Vec::new();
    let search = Search { selector, pid, window: window.id, deadline };
    walk(&mut source, root, 0, &search, &mut seen, &mut matches)?;
    match matches.len() {
        0 => Err(Failure::new(
            ErrorCode::NotFound,
            "no accessibility element matched the action selector",
            NotFound {
                kind: "element",
                pid: Some(pid.0),
                window_id: Some(window.id.0),
                ..Default::default()
            }
            .details(),
        )),
        1 => validate(matches.remove(0), purpose, pid, window),
        _ => Err(action_failure(
            ErrorCode::AmbiguousElement,
            "more than one accessibility element matched the action selector",
            pid,
            window.id,
        )),
    }
}

pub fn press(resolved: &ResolvedElement, pid: Pid, window: WindowId) -> Result<(), Failure> {
    // SAFETY: the live element was revalidated by a fresh strict lookup immediately before this
    // single semantic action. AXPress is never substituted or retried.
    let error = resolved.element.perform_action(&CFString::from_str("AXPress"));
    match error {
        AXError::Success => Ok(()),
        AXError::CannotComplete => Err(Failure::new(
            ErrorCode::ActionTimeout,
            format!(
                "AXPress did not complete within {MESSAGING_TIMEOUT_SECONDS} seconds; execution is unknown"
            ),
            ActionFailure {
                stage: ActionStage::Dispatch,
                dispatch: DispatchState::Unknown,
                mechanism: ActionMechanism::AxPress,
                pid: pid.0,
                window_id: window.0,
                release: ReleaseState::NotNeeded,
                key_window: None,
            }
            .details(),
        )),
        _ => Err(Failure::new(
            ErrorCode::ActionFailed,
            format!("AXPress failed with accessibility error {}", error.0),
            ActionFailure {
                stage: ActionStage::Dispatch,
                dispatch: DispatchState::Unknown,
                mechanism: ActionMechanism::AxPress,
                pid: pid.0,
                window_id: window.0,
                release: ReleaseState::NotNeeded,
                key_window: None,
            }
            .details(),
        )),
    }
}

struct Search<'a> {
    selector: &'a ElementSelector,
    pid: Pid,
    window: WindowId,
    deadline: Instant,
}

fn walk<S: ElementSource>(
    source: &mut S,
    element: S::Element,
    depth: usize,
    search: &Search<'_>,
    seen: &mut HashSet<S::Element>,
    matches: &mut Vec<Match<S::Element>>,
) -> Result<(), Failure> {
    if Instant::now() >= search.deadline {
        return Err(lookup_timeout(search.pid));
    }
    // An application may publish one element under more than one parent, or list an ancestor
    // again among a node's children. It is the same element either way, so reading it a second
    // time would count one match twice and a cycle would run on to the depth bound.
    if !seen.insert(element.clone()) {
        return Ok(());
    }
    let attributes = source.read(&element, depth < MAX_DEPTH).map_err(|failure| match failure {
        ReadFailure::Timeout => lookup_timeout(search.pid),
        ReadFailure::Unavailable | ReadFailure::Failed(_) => action_failure(
            ErrorCode::ActionUnavailable,
            "an accessibility subtree could not be read, so the selector is not unique",
            search.pid,
            search.window,
        ),
    })?;
    if matches_selector(&attributes, search.selector) {
        matches.push(Match {
            element,
            role: attributes.role.clone(),
            title: attributes.title.clone(),
            identifier: attributes.identifier.clone(),
            enabled: attributes.enabled,
            actions: attributes.actions.clone(),
            frame: attributes.frame,
        });
    }
    match attributes.children {
        ElementChildren::Read { elements, unreadable: false } => {
            for child in elements {
                walk(source, child, depth + 1, search, seen, matches)?;
            }
            Ok(())
        }
        ElementChildren::Read { unreadable: true, .. }
        | ElementChildren::Unread { present: true } => Err(action_failure(
            ErrorCode::ActionUnavailable,
            "an accessibility subtree was omitted, so the selector is not unique",
            search.pid,
            search.window,
        )),
        ElementChildren::Unread { present: false } => Ok(()),
    }
}

fn matches_selector<E>(attributes: &ElementAttributes<E>, selector: &ElementSelector) -> bool {
    selector
        .identifier
        .as_ref()
        .is_none_or(|expected| attributes.identifier.as_ref() == Some(expected))
        && selector.role.as_ref().is_none_or(|expected| attributes.role.as_ref() == Some(expected))
        && selector
            .title
            .as_ref()
            .is_none_or(|expected| attributes.title.as_ref() == Some(expected))
}

fn validate(
    matched: Match<Element>,
    purpose: ResolvePurpose,
    pid: Pid,
    window: &Window,
) -> Result<ResolvedElement, Failure> {
    validate_fields(purpose, matched.enabled, matched.frame, pid, window)?;
    Ok(matched.into())
}

fn validate_fields(
    purpose: ResolvePurpose,
    enabled: Option<bool>,
    frame: Option<Bounds>,
    pid: Pid,
    window: &Window,
) -> Result<(), Failure> {
    if purpose == ResolvePurpose::Click && enabled != Some(true) {
        return Err(action_failure(
            ErrorCode::ActionUnavailable,
            "the matched element is disabled or does not report an enabled state",
            pid,
            window.id,
        ));
    }
    let frame = frame.unwrap_or(Bounds { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
    let bounds = window.bounds;
    let finite = [frame.x, frame.y, frame.width, frame.height].into_iter().all(f64::is_finite);
    if !finite
        || frame.width <= 0.0
        || frame.height <= 0.0
        || frame.x < bounds.x
        || frame.y < bounds.y
        || frame.x + frame.width > bounds.x + bounds.width
        || frame.y + frame.height > bounds.y + bounds.height
    {
        return Err(action_failure(
            ErrorCode::ActionUnavailable,
            "the matched element does not have a finite, non-empty frame inside the window",
            pid,
            window.id,
        ));
    }
    Ok(())
}

fn action_failure(code: ErrorCode, message: &str, pid: Pid, window: WindowId) -> Failure {
    Failure::new(
        code,
        message,
        ActionFailure {
            stage: ActionStage::Resolve,
            dispatch: DispatchState::None,
            mechanism: ActionMechanism::None,
            pid: pid.0,
            window_id: window.0,
            release: ReleaseState::NotNeeded,
            key_window: None,
        }
        .details(),
    )
}

fn lookup_timeout(pid: Pid) -> Failure {
    Failure::new(
        ErrorCode::Timeout,
        format!("process {pid} did not finish the action element lookup within its deadline"),
        json!({"pid":pid.0,"timeoutSeconds":5}),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    struct FakeElement(usize);

    struct FakeSource {
        responses: HashMap<FakeElement, Result<ElementAttributes<FakeElement>, ReadFailure>>,
        reads: Vec<FakeElement>,
    }

    impl FakeSource {
        fn new(
            responses: Vec<(FakeElement, Result<ElementAttributes<FakeElement>, ReadFailure>)>,
        ) -> Self {
            Self { responses: responses.into_iter().collect(), reads: Vec::new() }
        }

        /// How often the walk asked this source for one element. A source answers the same
        /// element the same way every time, so a second read means the walk visited it twice.
        fn reads_of(&self, element: &FakeElement) -> usize {
            self.reads.iter().filter(|read| *read == element).count()
        }
    }

    impl ElementSource for FakeSource {
        type Element = FakeElement;

        fn read(
            &mut self,
            element: &FakeElement,
            _expand_children: bool,
        ) -> Result<ElementAttributes<FakeElement>, ReadFailure> {
            self.reads.push(element.clone());
            self.responses.get(element).expect("a response for every element read").clone()
        }
    }

    fn attributes(title: &str) -> ElementAttributes<FakeElement> {
        ElementAttributes { title: Some(title.to_owned()), ..Default::default() }
    }

    fn selector() -> ElementSelector {
        ElementSelector { identifier: None, role: None, title: Some("Share".to_owned()) }
    }

    fn window() -> Window {
        Window {
            id: WindowId(123),
            owner_pid: Pid(42),
            title: None,
            layer: 0,
            bounds: Bounds { x: 100.0, y: 100.0, width: 500.0, height: 500.0 },
            on_screen: true,
            alpha: 1.0,
        }
    }

    fn walk_with(source: &mut FakeSource) -> Result<Vec<Match<FakeElement>>, Failure> {
        let mut seen = HashSet::new();
        let mut matches = Vec::new();
        let selector = selector();
        let search = Search {
            selector: &selector,
            pid: Pid(42),
            window: WindowId(123),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        walk(source, FakeElement(0), 0, &search, &mut seen, &mut matches)?;
        Ok(matches)
    }

    #[test]
    fn proposal0007_duplicate_exact_matches_are_both_found() {
        let mut root = attributes("root");
        root.children = ElementChildren::Read {
            elements: vec![FakeElement(1), FakeElement(2)],
            unreadable: false,
        };
        let mut source = FakeSource::new(vec![
            (FakeElement(0), Ok(root)),
            (FakeElement(1), Ok(attributes("Share"))),
            (FakeElement(2), Ok(attributes("Share"))),
        ]);
        assert_eq!(walk_with(&mut source).unwrap().len(), 2);
    }

    #[test]
    fn proposal0007_an_unreadable_child_fails_instead_of_proving_uniqueness() {
        let mut root = attributes("root");
        root.children = ElementChildren::Read { elements: Vec::new(), unreadable: true };
        let mut source = FakeSource::new(vec![(FakeElement(0), Ok(root))]);
        assert_eq!(walk_with(&mut source).unwrap_err().code, ErrorCode::ActionUnavailable);
    }

    #[test]
    fn proposal0007_an_omitted_subtree_at_the_depth_bound_fails() {
        let mut root = attributes("root");
        root.children = ElementChildren::Unread { present: true };
        let mut source = FakeSource::new(vec![(FakeElement(0), Ok(root))]);
        let mut seen = HashSet::new();
        let mut matches = Vec::new();
        let selector = selector();
        let search = Search {
            selector: &selector,
            pid: Pid(42),
            window: WindowId(123),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        let failure =
            walk(&mut source, FakeElement(0), MAX_DEPTH, &search, &mut seen, &mut matches)
                .unwrap_err();
        assert_eq!(failure.code, ErrorCode::ActionUnavailable);
    }

    #[test]
    fn proposal0007_one_element_published_under_two_parents_matches_once() {
        let mut root = attributes("root");
        root.children = ElementChildren::Read {
            elements: vec![FakeElement(1), FakeElement(2)],
            unreadable: false,
        };
        let mut left = attributes("left");
        left.children = ElementChildren::Read { elements: vec![FakeElement(3)], unreadable: false };
        let mut right = attributes("right");
        right.children =
            ElementChildren::Read { elements: vec![FakeElement(3)], unreadable: false };
        let mut source = FakeSource::new(vec![
            (FakeElement(0), Ok(root)),
            (FakeElement(1), Ok(left)),
            (FakeElement(2), Ok(right)),
            (FakeElement(3), Ok(attributes("Share"))),
        ]);
        let matches = walk_with(&mut source).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].element, FakeElement(3));
        assert_eq!(source.reads_of(&FakeElement(3)), 1);
    }

    #[test]
    fn proposal0007_a_cycle_ends_the_walk_rather_than_failing_on_depth() {
        let mut root = attributes("root");
        root.children = ElementChildren::Read { elements: vec![FakeElement(1)], unreadable: false };
        let mut child = attributes("Share");
        // The application lists its own ancestor again among this element's children.
        child.children =
            ElementChildren::Read { elements: vec![FakeElement(0)], unreadable: false };
        let mut source =
            FakeSource::new(vec![(FakeElement(0), Ok(root)), (FakeElement(1), Ok(child))]);
        let matches = walk_with(&mut source).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].element, FakeElement(1));
        assert_eq!(source.reads_of(&FakeElement(0)), 1);
    }

    #[test]
    fn proposal0007_element_state_and_frame_must_be_actionable() {
        let window = window();
        let frame = Some(Bounds { x: 120.0, y: 120.0, width: 40.0, height: 20.0 });
        assert!(
            validate_fields(ResolvePurpose::Click, Some(true), frame, Pid(42), &window).is_ok()
        );
        for enabled in [None, Some(false)] {
            assert_eq!(
                validate_fields(ResolvePurpose::Click, enabled, frame, Pid(42), &window)
                    .unwrap_err()
                    .code,
                ErrorCode::ActionUnavailable
            );
        }
        for purpose in [ResolvePurpose::Click, ResolvePurpose::Scroll] {
            for invalid in [
                None,
                Some(Bounds { x: 99.0, y: 120.0, width: 40.0, height: 20.0 }),
                Some(Bounds { x: 120.0, y: 120.0, width: 0.0, height: 20.0 }),
                Some(Bounds { x: f64::NAN, y: 120.0, width: 40.0, height: 20.0 }),
            ] {
                assert_eq!(
                    validate_fields(purpose, Some(true), invalid, Pid(42), &window)
                        .unwrap_err()
                        .code,
                    ErrorCode::ActionUnavailable
                );
            }
        }
    }

    #[test]
    fn proposal0007_a_known_enabled_state_gates_an_element_click_only() {
        let window = window();
        let frame = Some(Bounds { x: 120.0, y: 120.0, width: 40.0, height: 20.0 });
        for enabled in [None, Some(false), Some(true)] {
            assert!(
                validate_fields(ResolvePurpose::Scroll, enabled, frame, Pid(42), &window).is_ok()
            );
        }
    }
}
