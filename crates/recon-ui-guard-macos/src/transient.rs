//! What happens between two looks at the Mac.
//!
//! The watcher looks at the desktop every ten milliseconds, so an application that is activated
//! and put back, or a Space that is switched and switched again, inside one of those gaps leaves
//! every look identical. macOS announces both as they happen: the workspace posts
//! `NSWorkspaceDidActivateApplicationNotification` when the active application changes and
//! `NSWorkspaceActiveSpaceDidChangeNotification` when the user's Space does. Listening to them
//! alongside the looks is what makes an activation that was undone a reported disturbance rather
//! than a quiet desktop.
//!
//! A notification is only delivered while some thread is running a run loop, and the thread
//! running the operation is busy running it. So the guard listens on a thread of its own, which
//! is also the first in the process to ask for the shared workspace: whatever the workspace
//! attaches to the current run loop, it attaches to one that runs.
//!
//! Nothing here acts on what it hears. It records the announcement and the moment it arrived,
//! and leaves the meaning to the guard.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{ClassType, msg_send};
use objc2_app_kit::{
    NSRunningApplication, NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
};
use objc2_core_foundation::{CFRunLoop, CFRunLoopRunResult, kCFRunLoopDefaultMode};
use objc2_foundation::{NSNotification, NSNotificationCenter};

use recon_core::process::Pid;

/// How long the listening thread lets the run loop wait before it checks whether the watch is
/// over. It is a teardown delay, not a sampling interval: a notification that arrives during a
/// slice is delivered the moment it is posted.
const SLICE: Duration = Duration::from_millis(25);

/// What the notification center hands back for a registration: the token that removes it again.
type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

/// Something macOS announced about the desktop while the guard was watching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    /// An application became the active one. The process is the one the notification named, when
    /// it named one Recon could resolve.
    Activated(Option<Pid>),
    /// The Space the user is looking at changed.
    SpaceChanged,
}

/// A running subscription to the workspace's account of the desktop.
///
/// Dropping it stops the observers and the thread, so a guard that fails, panics, or returns
/// early cannot leave a listener behind.
pub(crate) struct Notices {
    announced: Arc<Mutex<Vec<(Notice, Instant)>>>,
    stop: Arc<AtomicBool>,
    started: AtomicBool,
    listener: Option<JoinHandle<()>>,
}

impl Notices {
    /// Starts listening, and returns only once macOS has accepted both observers.
    ///
    /// # Errors
    ///
    /// Why the observers could not be registered. An operation Recon cannot hear about cannot
    /// report that it left the user's desktop alone, so the caller fails before it runs rather
    /// than watching with one eye.
    pub(crate) fn listening() -> Result<Self, String> {
        let announced = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let recording = Arc::clone(&announced);
        let listening = Arc::clone(&stop);
        let (registered_tx, registered_rx) = mpsc::channel();
        let listener = thread::Builder::new()
            .name("recon-no-focus-notices".to_owned())
            .spawn(move || listen(&recording, &listening, &registered_tx))
            .map_err(|error| format!("its thread could not start: {error}"))?;
        // Dropping `notices` on either failure stops the thread it started.
        let notices =
            Self { announced, stop, started: AtomicBool::new(false), listener: Some(listener) };
        match registered_rx.recv() {
            Ok(Ok(())) => Ok(notices),
            Ok(Err(reason)) => Err(reason),
            Err(_) => Err("the thread listening for them stopped before it registered".to_owned()),
        }
    }

    /// Moves everything announced since the last call into `into`, timed from `clock`.
    pub(crate) fn take(&self, clock: Instant, into: &mut Vec<(Notice, Duration)>) {
        // A poisoned log is one a panicking observer left behind; the announcements already in it
        // are still what macOS said, and losing them would turn a disturbance into a quiet watch.
        let mut announced = self.announced.lock().unwrap_or_else(PoisonError::into_inner);
        let first = !self.started.swap(true, Ordering::AcqRel);
        into.extend(announced.drain(..).filter_map(|(notice, at)| {
            if first && at < clock {
                None
            } else {
                Some((notice, at.saturating_duration_since(clock)))
            }
        }));
    }
}

impl Drop for Notices {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

/// Registers both observers, reports whether macOS accepted them, and then keeps this thread's
/// run loop alive until the watch is over.
fn listen(
    announced: &Arc<Mutex<Vec<(Notice, Instant)>>>,
    stop: &AtomicBool,
    registered: &mpsc::Sender<Result<(), String>>,
) {
    // SAFETY: an immutable Core Foundation constant.
    let Some(mode) = (unsafe { kCFRunLoopDefaultMode }) else {
        let _ = registered.send(Err("this Mac has no default run loop mode".to_owned()));
        return;
    };
    // SAFETY: `+[NSWorkspace sharedWorkspace]` takes no arguments and returns the shared
    // workspace, or nil in a process that has none. Asking for it neither activates an
    // application nor opens anything.
    let workspace: Option<Retained<NSWorkspace>> =
        unsafe { msg_send![NSWorkspace::class(), sharedWorkspace] };
    let Some(workspace) = workspace else {
        let _ = registered.send(Err("this process has no shared workspace".to_owned()));
        return;
    };
    // SAFETY: `-[NSWorkspace notificationCenter]` takes no arguments and returns the center the
    // workspace posts its announcements to, or nil when it has none.
    let center: Option<Retained<NSNotificationCenter>> =
        unsafe { msg_send![&*workspace, notificationCenter] };
    let Some(center) = center else {
        let _ = registered.send(Err("the shared workspace has no notification center".to_owned()));
        return;
    };

    let activations = Arc::clone(announced);
    let activated = RcBlock::new(move |notification: NonNull<NSNotification>| {
        // SAFETY: the notification center passes a live notification for the length of the call.
        let notification = unsafe { notification.as_ref() };
        record(&activations, Notice::Activated(activated_pid(notification)));
    });
    let spaces = Arc::clone(announced);
    let switched = RcBlock::new(move |_: NonNull<NSNotification>| {
        // The Space notification carries no detail: that the user's Space changed is the whole
        // of what macOS says, and the whole of what the contract forbids.
        record(&spaces, Notice::SpaceChanged);
    });

    // SAFETY: both names are AppKit's own notification constants and both blocks take the
    // notification the center hands them. A nil queue asks for delivery as each notification is
    // posted, rather than through an operation queue Recon would also have to run.
    let observers = unsafe {
        (
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidActivateApplicationNotification),
                None,
                None,
                &activated,
            ),
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceActiveSpaceDidChangeNotification),
                None,
                None,
                &switched,
            ),
        )
    };
    let _ = registered.send(Ok(()));

    while !stop.load(Ordering::Acquire) {
        // A run loop with nothing attached to it finishes at once; sleeping out the rest of the
        // slice keeps this thread from spinning while the operation runs.
        if CFRunLoop::run_in_mode(Some(mode), SLICE.as_secs_f64(), false)
            == CFRunLoopRunResult::Finished
        {
            thread::sleep(SLICE);
        }
    }

    forget(&center, observers);
}

/// Takes both observers off the workspace's notification center. The guard watches one operation;
/// an observer that outlived it would record a desktop nobody is being held to.
fn forget(center: &NSNotificationCenter, observers: (Observer, Observer)) {
    let (activation, space) = observers;
    let activation: &AnyObject = AsRef::as_ref(&*activation);
    let space: &AnyObject = AsRef::as_ref(&*space);
    // SAFETY: both observers were registered on this center above, each is removed once, and
    // neither is used again afterwards.
    unsafe {
        center.removeObserver(activation);
        center.removeObserver(space);
    }
}

fn record(announced: &Mutex<Vec<(Notice, Instant)>>, notice: Notice) {
    let at = Instant::now();
    announced.lock().unwrap_or_else(PoisonError::into_inner).push((notice, at));
}

/// The process an activation notification named.
///
/// An announcement Recon cannot resolve to a process is still an activation; it just cannot be
/// held against the selected target.
fn activated_pid(notification: &NSNotification) -> Option<Pid> {
    let about = notification.userInfo()?;
    // SAFETY: an immutable string constant AppKit exports as the key for the application an
    // activation notification is about.
    let key = unsafe { NSWorkspaceApplicationKey };
    let application = about.objectForKey(key)?.downcast::<NSRunningApplication>().ok()?;
    // A running application without a process ID reports -1.
    u32::try_from(application.processIdentifier()).ok().map(Pid)
}

#[cfg(test)]
mod tests {
    use objc2_foundation::{NSDictionary, NSString};

    use super::*;

    /// A subscription with nothing listening behind it, for the bookkeeping the guard relies on.
    fn unattended() -> Notices {
        Notices {
            announced: Arc::new(Mutex::new(Vec::new())),
            stop: Arc::new(AtomicBool::new(false)),
            started: AtomicBool::new(false),
            listener: None,
        }
    }

    #[test]
    fn what_was_announced_is_timed_from_the_start_and_taken_once() {
        let notices = unattended();
        let clock = Instant::now();
        record(&notices.announced, Notice::SpaceChanged);
        record(&notices.announced, Notice::Activated(Some(Pid(4242))));
        let mut announced = Vec::new();

        notices.take(clock, &mut announced);

        assert_eq!(announced.len(), 2);
        assert_eq!(announced[0].0, Notice::SpaceChanged);
        assert_eq!(announced[1].0, Notice::Activated(Some(Pid(4242))));
        assert!(announced[0].1 <= announced[1].1);
        // Taking what has been announced leaves nothing to report twice.
        notices.take(clock, &mut announced);
        assert_eq!(announced.len(), 2);
    }

    /// The observers are registered before the first look, so an announcement can arrive before
    /// the baseline. It belongs to setup, not to the operation being judged.
    #[test]
    fn an_announcement_from_before_the_watch_is_excluded() {
        let notices = unattended();
        record(&notices.announced, Notice::SpaceChanged);
        thread::sleep(Duration::from_millis(2));
        let clock = Instant::now();
        let mut announced = Vec::new();

        notices.take(clock, &mut announced);

        assert!(announced.is_empty());
    }

    /// The observers, the blocks behind them, and the process an activation names, end to end.
    ///
    /// Recon posts the two notifications itself rather than activating an application or
    /// switching a Space: really disturbing the desktop is what the guard exists to forbid, and
    /// the center delivers either one the same way.
    #[test]
    fn what_the_workspace_posts_is_recorded_with_the_process_it_names() {
        let Ok(notices) = Notices::listening() else {
            // A Mac that cannot register them is the case the guard fails closed on.
            return;
        };
        let clock = Instant::now();
        // SAFETY: `+[NSWorkspace sharedWorkspace]` takes no arguments and returns the workspace
        // the observers above were registered with.
        let workspace: Retained<NSWorkspace> =
            unsafe { msg_send![NSWorkspace::class(), sharedWorkspace] };
        // SAFETY: `-[NSWorkspace notificationCenter]` takes no arguments and returns the center
        // they were registered on.
        let center: Retained<NSNotificationCenter> =
            unsafe { msg_send![&*workspace, notificationCenter] };
        let Some(application) = workspace.frontmostApplication() else {
            return;
        };
        // SAFETY: the key constant an activation notification carries its application under.
        let key: &NSString = unsafe { NSWorkspaceApplicationKey };
        let about = NSDictionary::from_slices::<NSString>(&[key], &[&*application]);
        // SAFETY: a dictionary of objects stays a dictionary of objects; only the types of its
        // keys and values are being forgotten, which is how the notification carries it.
        let about: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(about) };
        // SAFETY: both names are AppKit's own, and the activation carries the user info its
        // observer reads.
        unsafe {
            center.postNotificationName_object(NSWorkspaceActiveSpaceDidChangeNotification, None);
            center.postNotificationName_object_userInfo(
                NSWorkspaceDidActivateApplicationNotification,
                None,
                Some(&about),
            );
        }

        let mut announced = Vec::new();
        notices.take(clock, &mut announced);

        let heard: Vec<Notice> = announced.iter().map(|(notice, _)| *notice).collect();
        let named = u32::try_from(application.processIdentifier()).ok().map(Pid);
        // The desktop is the user's, so it may have announced something of its own as well.
        assert!(heard.contains(&Notice::SpaceChanged), "{heard:?}");
        assert!(heard.contains(&Notice::Activated(named)), "{heard:?}");
    }
}
