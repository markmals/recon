//! The window server's view of an application: every window it owns, front to back.

use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect,
};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
    kCGNullWindowID, kCGWindowAlpha, kCGWindowBounds, kCGWindowIsOnscreen, kCGWindowLayer,
    kCGWindowName, kCGWindowNumber, kCGWindowOwnerPID,
};

use crate::ui::window::{Bounds, Window, title_from, windows_of};

/// The windows the given process owns, in the window server's front-to-back order.
pub fn windows_owned_by(pid: u32) -> Result<Vec<Window>, String> {
    Ok(windows_of(windows()?, pid))
}

fn windows() -> Result<Vec<Window>, String> {
    let listed = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
        .ok_or_else(|| "macOS did not report its window list".to_owned())?;
    // SAFETY: the window server describes each window with a dictionary keyed by CFString.
    let listed: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(listed) };
    let keys = Keys::of_the_window_server();
    // An entry without the facts that identify a window is not a window Recon can report.
    Ok(listed.iter().filter_map(|entry| window_from(&entry, &keys)).collect())
}

fn window_from(entry: &CFDictionary<CFString, CFType>, keys: &Keys) -> Option<Window> {
    let number = |key: &CFString| entry.get(key)?.downcast::<CFNumber>().ok();
    let title =
        entry.get(keys.name).and_then(|name| Some(name.downcast::<CFString>().ok()?.to_string()));

    Some(Window {
        id: u32::try_from(number(keys.number)?.as_i64()?).ok()?,
        owner_pid: u32::try_from(number(keys.owner_pid)?.as_i64()?).ok()?,
        title: title_from(title),
        layer: number(keys.layer)?.as_i32()?,
        bounds: bounds_from(entry.get(keys.bounds)?)?,
        on_screen: entry
            .get(keys.on_screen)
            .and_then(|flag| flag.downcast::<CFBoolean>().ok())
            .is_some_and(|flag| flag.as_bool()),
        alpha: number(keys.alpha)?.as_f64()?,
    })
}

fn bounds_from(bounds: CFRetained<CFType>) -> Option<Bounds> {
    let bounds = bounds.downcast::<CFDictionary>().ok()?;
    let mut rect = CGRect::ZERO;
    // SAFETY: the window server stores bounds in the dictionary representation of a CGRect.
    let made = unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &raw mut rect) };
    made.then_some(Bounds {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    })
}

struct Keys {
    number: &'static CFString,
    owner_pid: &'static CFString,
    name: &'static CFString,
    layer: &'static CFString,
    bounds: &'static CFString,
    on_screen: &'static CFString,
    alpha: &'static CFString,
}

impl Keys {
    fn of_the_window_server() -> Self {
        // SAFETY: the documented dictionary keys of `CGWindowListCopyWindowInfo`.
        unsafe {
            Self {
                number: kCGWindowNumber,
                owner_pid: kCGWindowOwnerPID,
                name: kCGWindowName,
                layer: kCGWindowLayer,
                bounds: kCGWindowBounds,
                on_screen: kCGWindowIsOnscreen,
                alpha: kCGWindowAlpha,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use objc2_core_foundation::{CGPoint, CGSize};
    use objc2_core_graphics::CGRectCreateDictionaryRepresentation;

    use super::*;

    fn entry(pairs: &[(&'static CFString, &CFType)]) -> CFRetained<CFDictionary<CFString, CFType>> {
        let keys: Vec<&CFString> = pairs.iter().map(|(key, _)| *key).collect();
        let values: Vec<&CFType> = pairs.iter().map(|(_, value)| *value).collect();
        CFDictionary::from_slices(&keys, &values)
    }

    #[test]
    fn a_window_server_entry_becomes_a_window() {
        let keys = Keys::of_the_window_server();
        let bounds = CGRectCreateDictionaryRepresentation(CGRect::new(
            CGPoint::new(12.0, 34.0),
            CGSize::new(800.0, 600.0),
        ));
        let number = CFNumber::new_i64(53510);
        let owner_pid = CFNumber::new_i64(412);
        let layer = CFNumber::new_i64(0);
        let alpha = CFNumber::new_f64(0.5);
        let title = CFString::from_str("Recon Fixture — Main");
        let entry = entry(&[
            (keys.number, &number),
            (keys.owner_pid, &owner_pid),
            (keys.layer, &layer),
            (keys.alpha, &alpha),
            (keys.name, &title),
            (keys.bounds, &bounds),
            (keys.on_screen, CFBoolean::new(true)),
        ]);

        assert_eq!(
            window_from(&entry, &keys),
            Some(Window {
                id: 53510,
                owner_pid: 412,
                title: Some("Recon Fixture — Main".to_owned()),
                layer: 0,
                bounds: Bounds { x: 12.0, y: 34.0, width: 800.0, height: 600.0 },
                on_screen: true,
                alpha: 0.5,
            })
        );
    }

    #[test]
    fn an_entry_without_a_title_or_an_onscreen_flag_still_describes_a_window() {
        let keys = Keys::of_the_window_server();
        let bounds = CGRectCreateDictionaryRepresentation(CGRect::ZERO);
        let number = CFNumber::new_i64(7);
        let owner_pid = CFNumber::new_i64(412);
        let layer = CFNumber::new_i64(3);
        let alpha = CFNumber::new_f64(0.0);
        let entry = entry(&[
            (keys.number, &number),
            (keys.owner_pid, &owner_pid),
            (keys.layer, &layer),
            (keys.alpha, &alpha),
            (keys.bounds, &bounds),
        ]);

        let window = window_from(&entry, &keys).expect("a window");
        assert_eq!(window.title, None);
        assert!(!window.on_screen);
    }

    #[test]
    fn an_entry_without_a_window_number_is_not_a_window() {
        let keys = Keys::of_the_window_server();
        let owner_pid = CFNumber::new_i64(412);

        assert_eq!(window_from(&entry(&[(keys.owner_pid, &owner_pid)]), &keys), None);
    }
}
