use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::LazyLock;

use objc2_core_foundation::CGPoint;
use recon_core::error::Failure;
use recon_core::error::{ActionMechanism, DispatchState, ReleaseState};

use crate::failure::with_delivery;
use crate::request::{MouseButton, Point, ScrollUnit};
use crate::target::Target;

pub(crate) struct Source(*mut c_void);

impl Source {
    pub(crate) fn private() -> Option<Self> {
        // SAFETY: creates an event source with no shared HID state.
        NonNull::new(unsafe { CGEventSourceCreate(PRIVATE_SOURCE) })
            .map(|source| Self(source.as_ptr()))
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the source is a retained Core Foundation object owned by this wrapper.
        unsafe { CFRelease(self.0) }
    }
}

pub(crate) struct Event(*mut c_void);

impl Event {
    fn mouse(source: &Source, button: MouseButton, down: bool, global: CGPoint) -> Option<Self> {
        let (event_type, mouse_button) = match (button, down) {
            (MouseButton::Left, true) => (1, 0),
            (MouseButton::Left, false) => (2, 0),
            (MouseButton::Right, true) => (3, 1),
            (MouseButton::Right, false) => (4, 1),
            (MouseButton::Middle, true) => (25, 2),
            (MouseButton::Middle, false) => (26, 2),
        };
        // SAFETY: arguments follow CGEventCreateMouseEvent's C contract.
        NonNull::new(unsafe { CGEventCreateMouseEvent(source.0, event_type, global, mouse_button) })
            .map(|event| Self(event.as_ptr()))
    }

    fn wheel(source: &Source, unit: ScrollUnit, dx: i32, dy: i32, global: CGPoint) -> Option<Self> {
        let unit = match unit {
            ScrollUnit::Pixel => 0,
            ScrollUnit::Line => 1,
        };
        // SAFETY: the two-axis wheel event has three values, with the third unused.
        let event = unsafe { CGEventCreateScrollWheelEvent2(source.0, unit, 2, -dy, -dx, 0) };
        let event = NonNull::new(event).map(|event| Self(event.as_ptr()))?;
        // SAFETY: event is live and global is a finite point validated by the caller.
        unsafe { CGEventSetLocation(event.0, global) };
        Some(event)
    }

    fn key(source: &Source, keycode: u16, down: bool) -> Option<Self> {
        // SAFETY: creates a keyboard or modifier transition event for one virtual key code.
        NonNull::new(unsafe { CGEventCreateKeyboardEvent(source.0, keycode, down) })
            .map(|event| Self(event.as_ptr()))
    }

    fn flags(&self, flags: u64) {
        // SAFETY: event remains alive for this call.
        unsafe { CGEventSetFlags(self.0, flags) }
    }

    fn stamp(
        &self,
        target: &Target,
        local: Option<Point>,
        click: Option<(i64, i64, i64)>,
    ) -> Result<(), Failure> {
        // SAFETY: SLEventSetIntegerValueField has the private SkyLight event-field signature.
        let set: unsafe extern "C" fn(*mut c_void, u32, i64) = private_set_field()?;
        // SAFETY: the event is live, and fields 40 and 51 carry the bound process and window.
        unsafe {
            set(self.0, 40, target.pid().0 as i64);
            set(self.0, 51, target.window().id.0 as i64);
        }
        if let Some((state, button, group)) = click {
            // SAFETY: the event is live, and these are the mouse fields the WindowServer route reads.
            unsafe {
                set(self.0, 1, state);
                set(self.0, 3, button);
                set(self.0, 7, 3);
                set(self.0, 58, group);
            }
        }
        if let Some(local) = local {
            // SAFETY: CGEventSetWindowLocation takes window-local logical coordinates.
            let set_location: unsafe extern "C" fn(*mut c_void, f64, f64) =
                private_window_location()?;
            // SAFETY: the event is live and the point was validated against the bound window.
            unsafe { set_location(self.0, local.x, local.y) };
        }
        Ok(())
    }

    fn post(self, target: &Target) {
        // SAFETY: the event stays alive through the process-addressed post, then Drop releases it.
        unsafe { CGEventPostToPid(target.pid().0 as i32, self.0) }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: the event is a retained Core Foundation object owned by this wrapper.
        unsafe { CFRelease(self.0) }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Delivery {
    pub(crate) dispatch: DispatchState,
    pub(crate) release: ReleaseState,
}

#[derive(Clone, Copy)]
pub(crate) struct MouseEvent {
    pub(crate) point: Point,
    pub(crate) button: MouseButton,
    pub(crate) down: bool,
    pub(crate) click_state: i64,
    pub(crate) flags: u64,
    pub(crate) group: i64,
}

#[derive(Clone, Copy)]
pub(crate) struct KeyEvent {
    pub(crate) keycode: u16,
    pub(crate) down: bool,
    pub(crate) flags: u64,
}

pub(crate) fn post_mouse(
    target: &Target,
    source: &Source,
    event: MouseEvent,
    delivery: Delivery,
) -> Result<(), Failure> {
    let window =
        target.revalidate(ActionMechanism::MouseEvents, delivery.dispatch, delivery.release)?;
    if event.down {
        Target::validate_point(&window, event.point, ActionMechanism::MouseEvents)
            .map_err(|failure| with_delivery(failure, delivery))?;
    }
    let global = global_point(&window, event.point);
    let posted = Event::mouse(source, event.button, event.down, global).ok_or_else(|| {
        event_failure(ActionMechanism::MouseEvents, target, delivery.dispatch, delivery.release)
    })?;
    posted.flags(event.flags);
    posted.stamp(
        target,
        Some(event.point),
        Some((event.click_state, event.button.number(), event.group)),
    )?;
    posted.post(target);
    Ok(())
}

pub(crate) fn post_wheel(
    target: &Target,
    source: &Source,
    point: Point,
    dx: i32,
    dy: i32,
    unit: ScrollUnit,
) -> Result<(), Failure> {
    let window = target.revalidate(
        ActionMechanism::ScrollWheel,
        DispatchState::None,
        ReleaseState::NotNeeded,
    )?;
    Target::validate_point(&window, point, ActionMechanism::ScrollWheel)?;
    let global = global_point(&window, point);
    let event = Event::wheel(source, unit, dx, dy, global).ok_or_else(|| {
        event_failure(
            ActionMechanism::ScrollWheel,
            target,
            DispatchState::None,
            ReleaseState::NotNeeded,
        )
    })?;
    event.stamp(target, Some(point), None)?;
    event.post(target);
    Ok(())
}

pub(crate) fn post_key(
    target: &Target,
    source: &Source,
    event: KeyEvent,
    delivery: Delivery,
) -> Result<(), Failure> {
    target.revalidate(ActionMechanism::KeyEvents, delivery.dispatch, delivery.release)?;
    let posted = Event::key(source, event.keycode, event.down).ok_or_else(|| {
        event_failure(ActionMechanism::KeyEvents, target, delivery.dispatch, delivery.release)
    })?;
    posted.flags(event.flags);
    posted.stamp(target, None, None)?;
    posted.post(target);
    Ok(())
}

fn global_point(window: &recon_window_server_macos::window::Window, point: Point) -> CGPoint {
    CGPoint { x: window.bounds.x + point.x, y: window.bounds.y + point.y }
}

pub(crate) fn event_failure(
    mechanism: ActionMechanism,
    target: &Target,
    dispatch: DispatchState,
    release: ReleaseState,
) -> Failure {
    crate::failure::action(
        recon_core::error::ErrorCode::ActionFailed,
        "macOS could not create a private input event source or event",
        crate::failure::details(
            target,
            mechanism,
            recon_core::error::ActionStage::Dispatch,
            dispatch,
            release,
        ),
    )
}

fn private_set_field() -> Result<unsafe extern "C" fn(*mut c_void, u32, i64), Failure> {
    type Setter = unsafe extern "C" fn(*mut c_void, u32, i64);
    static SYMBOL: LazyLock<Option<Setter>> = LazyLock::new(|| {
        // SAFETY: binds the private SkyLight setter to its C signature.
        unsafe {
            crate::private::function(c"SLEventSetIntegerValueField", "window-event-fields").ok()
        }
    });
    if let Some(function) = *SYMBOL {
        return Ok(function);
    }
    // SAFETY: preserves the typed interface error when the symbol is absent.
    unsafe { crate::private::function(c"SLEventSetIntegerValueField", "window-event-fields") }
}

fn private_window_location() -> Result<unsafe extern "C" fn(*mut c_void, f64, f64), Failure> {
    type Setter = unsafe extern "C" fn(*mut c_void, f64, f64);
    static SYMBOL: LazyLock<Option<Setter>> = LazyLock::new(|| {
        // SAFETY: binds the private Core Graphics window-local location setter to its C signature.
        unsafe {
            crate::private::function(c"CGEventSetWindowLocation", "window-event-location").ok()
        }
    });
    if let Some(function) = *SYMBOL {
        return Ok(function);
    }
    // SAFETY: preserves the typed interface error when the symbol is absent.
    unsafe { crate::private::function(c"CGEventSetWindowLocation", "window-event-location") }
}

const PRIVATE_SOURCE: i32 = -1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> *mut c_void;
    fn CGEventCreateMouseEvent(
        source: *mut c_void,
        kind: u32,
        point: CGPoint,
        button: u32,
    ) -> *mut c_void;
    fn CGEventCreateScrollWheelEvent2(
        source: *mut c_void,
        unit: u32,
        wheels: u32,
        vertical: i32,
        horizontal: i32,
        third: i32,
    ) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetLocation(event: *mut c_void, point: CGPoint);
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPostToPid(pid: i32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *mut c_void);
}
