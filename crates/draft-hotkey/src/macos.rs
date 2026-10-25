//! macOS `CGEventTap`.
//!
//! macOS has no `WH_KEYBOARD_LL`; the only way to see a bare modifier such as
//! Right Control is an event tap at the HID point. The tap runs on its own
//! thread and never blocks: it pushes one small event into a bounded channel
//! and returns.
//!
//! The tap is *active* rather than listen-only, so the dictation key and
//! Escape-while-listening are swallowed before the focused application sees
//! them, the same way the Windows hook suppresses them. Creating an active tap
//! is also the honest permission check: macOS returns null when Draft has not
//! been granted Accessibility and Input Monitoring, so a missing grant becomes
//! a real error instead of a key that silently never fires.
//!
//! Two macOS-specific facts shape this module:
//!
//! * A modifier key produces `FlagsChanged`, not `KeyDown`/`KeyUp`. The right
//!   and left Control keys share one flag, so the keycode of the changed key
//!   is what identifies the dictation key, and the press/release state is
//!   derived by toggling our own record rather than reading the flag.
//! * macOS disables a tap whose callback was slow (`TapDisabledByTimeout`) or
//!   whose input was blocked (`TapDisabledByUserInput`). Both arrive as events;
//!   the callback re-enables the tap and logs it. A tap left disabled looks
//!   exactly like a key that stopped working, which is the failure this app
//!   must never present silently.

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, bail};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext, CFRunLoopTimerCreate,
    CFRunLoopTimerInvalidate, CFRunLoopTimerRef, kCFRunLoopCommonModes,
};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, EventField, KeyCode,
};
use core_graphics::event_source::CGEventSourceStateID;
use tracing::{info, warn};

use crate::{HotkeyConfig, HotkeyEvent};

/// Escape, which cancels an active capture.
const KEY_ESCAPE: u16 = KeyCode::ESCAPE;

/// `kCGEventSourceStateHIDSystemState`. A real key press arrives at the tap
/// with this source state; anything Draft or another program synthesizes
/// carries a private or combined-session source instead.
const HID_SYSTEM_STATE: i64 = CGEventSourceStateID::HIDSystemState as i64;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    /// Returns a CoreFoundation `Boolean`, which is an unsigned byte.
    fn AXIsProcessTrusted() -> u8;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDCheckAccess(request_type: u32) -> u32;
    fn IOHIDRequestAccess(request_type: u32) -> u8;
}

/// `kIOHIDRequestTypeListenEvent`: observing keys, which is what an event tap
/// needs. Posting synthetic keys is a separate grant Draft does not use.
const LISTEN_EVENT: u32 = 1;
/// `kIOHIDAccessTypeGranted`.
const ACCESS_GRANTED: u32 = 0;

/// The two grants an active event tap needs. They are different panes in
/// System Settings and neither one implies the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub accessibility: bool,
    pub input_monitoring: bool,
}

impl Permissions {
    pub fn granted(self) -> bool {
        self.accessibility && self.input_monitoring
    }
}

/// The System Settings pane that grants a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyPane {
    Accessibility,
    InputMonitoring,
}

impl PrivacyPane {
    fn url(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Self::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        }
    }
}

/// Read the grants straight from the system on every call. A cached value would
/// keep claiming "granted" after someone revoked access in System Settings.
pub fn permissions() -> Permissions {
    Permissions {
        accessibility: unsafe { AXIsProcessTrusted() != 0 },
        input_monitoring: unsafe { IOHIDCheckAccess(LISTEN_EVENT) == ACCESS_GRANTED },
    }
}

/// Show the Input Monitoring prompt. The system only shows it once; opening the
/// pane is the reliable path after the person has already answered.
pub fn request_input_monitoring() {
    unsafe { IOHIDRequestAccess(LISTEN_EVENT) };
}

/// Open the exact Privacy & Security pane for a permission.
///
/// `open` is used instead of `NSWorkspace` because the URL scheme is the only
/// documented way to deep-link a privacy pane, and it needs no extra framework.
pub fn open_privacy_settings(pane: PrivacyPane) -> std::io::Result<()> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(pane.url())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("open exited with {status}")))
    }
}

/// The macOS keycode for a configured key.
///
/// `Pause` and `Scroll Lock` have no Apple keyboard equivalent, so they have no
/// keycode and `HotkeyConfig::available()` never offers them here.
fn keycode(config: HotkeyConfig) -> Option<u16> {
    match config {
        HotkeyConfig::RightCtrl => Some(KeyCode::RIGHT_CONTROL),
        HotkeyConfig::RightAlt => Some(KeyCode::RIGHT_OPTION),
        HotkeyConfig::F8 => Some(KeyCode::F8),
        HotkeyConfig::F9 => Some(KeyCode::F9),
        HotkeyConfig::Pause | HotkeyConfig::ScrollLock => None,
    }
}

/// Whether the key changes a modifier flag instead of producing key events.
fn is_modifier(config: HotkeyConfig) -> bool {
    matches!(config, HotkeyConfig::RightCtrl | HotkeyConfig::RightAlt)
}

/// A shared record the tap callback can read without locking.
///
/// The mach port is stored after the tap exists so a disabled tap can be
/// re-enabled from inside its own callback, which cannot borrow the tap.
struct Shared {
    sender: SyncSender<HotkeyEvent>,
    keycode: u16,
    modifier: bool,
    /// Whether synthetic input may drive the hotkey. Read once at startup so the
    /// callback, which sees every key press in the system, does not touch the
    /// environment for each one.
    allow_injected: bool,
    holding: AtomicBool,
    escape_down: AtomicBool,
    port: AtomicPtr<c_void>,
}

/// The owner of the tap thread. Dropping it stops the run loop.
pub struct HotkeyListener {
    run_loop: CFRunLoop,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        // `start` returns only after the loop is running, so this stop can
        // never land before `CFRunLoopRun` and be discarded.
        self.run_loop.stop();
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                warn!("hotkey thread panicked");
            }
        }
    }
}

pub fn start(config: HotkeyConfig) -> Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    let Some(keycode) = keycode(config) else {
        bail!("{} is not a key on an Apple keyboard", config.name());
    };

    let (sender, receiver) = sync_channel(4096);
    let allow_injected = std::env::var_os("DRAFT_ALLOW_INJECTED_KEYS").is_some();
    if allow_injected {
        warn!("synthetic keyboard input is enabled (test mode)");
    }
    let shared = Arc::new(Shared {
        sender,
        keycode,
        modifier: is_modifier(config),
        allow_injected,
        holding: AtomicBool::new(false),
        escape_down: AtomicBool::new(false),
        port: AtomicPtr::new(std::ptr::null_mut()),
    });

    let (ready_sender, ready_receiver) = channel::<std::result::Result<CFRunLoop, String>>();
    let thread = thread::Builder::new()
        .name("draft-hotkey".to_owned())
        .spawn(move || run_tap(&shared, ready_sender))
        .context("spawn hotkey thread")?;

    match ready_receiver.recv() {
        Ok(Ok(run_loop)) => {
            info!(key = config.id(), keycode, "global hotkey ready");
            Ok((
                HotkeyListener {
                    run_loop,
                    thread: Some(thread),
                },
                receiver,
            ))
        }
        Ok(Err(error)) => {
            let _ = thread.join();
            bail!("install event tap: {error}");
        }
        Err(_) => {
            let _ = thread.join();
            bail!("hotkey thread exited before it was ready");
        }
    }
}

fn run_tap(shared: &Arc<Shared>, ready: Sender<std::result::Result<CFRunLoop, String>>) {
    let tap = match CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        {
            let shared = Arc::clone(shared);
            move |_proxy: CGEventTapProxy, kind: CGEventType, event: &CGEvent| {
                handle_event(&shared, kind, event)
            }
        },
    ) {
        Ok(tap) => tap,
        Err(()) => {
            let _ = ready.send(Err(
                "macOS refused the tap. Grant Draft Accessibility and Input Monitoring in \
                 System Settings, Privacy & Security, then relaunch."
                    .to_owned(),
            ));
            return;
        }
    };

    shared.port.store(
        tap.mach_port.as_concrete_TypeRef() as *mut c_void,
        Ordering::SeqCst,
    );

    let run_loop = CFRunLoop::get_current();
    let source = match tap.mach_port.create_runloop_source(0) {
        Ok(source) => source,
        Err(()) => {
            let _ = ready.send(Err("create a run loop source for the event tap".to_owned()));
            return;
        }
    };
    let common_modes = unsafe { kCFRunLoopCommonModes };
    run_loop.add_source(&source, common_modes);
    tap.enable();

    // A zero-length timer reports readiness from *inside* the running loop. If
    // the thread sent readiness before `run_current`, a listener dropped
    // immediately afterwards could stop the loop too early and the join below
    // would never return.
    let context = Box::into_raw(Box::new(ready));
    let mut timer_context = CFRunLoopTimerContext {
        version: 0,
        info: context as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
    };
    let timer = unsafe {
        CFRunLoopTimerCreate(
            core_foundation::base::kCFAllocatorDefault,
            0.0,
            0.0,
            0,
            0,
            announce_ready,
            &mut timer_context,
        )
    };
    if timer.is_null() {
        unsafe { drop(Box::from_raw(context)) };
        warn!("the event tap is running but readiness could not be reported");
    } else {
        let timer = unsafe { CFRunLoopTimer::wrap_under_create_rule(timer) };
        run_loop.add_timer(&timer, common_modes);

        CFRunLoop::run_current();

        // The loop retains the timer while it runs; this releases our
        // reference once the loop has stopped.
        drop(timer);
    }

    shared.port.store(std::ptr::null_mut(), Ordering::SeqCst);
}

/// Fires inside the run loop on the tap thread and hands the loop back to
/// `start`. It is a one-shot: the timer is invalidated before anything else.
extern "C" fn announce_ready(timer: CFRunLoopTimerRef, info: *mut c_void) {
    unsafe {
        CFRunLoopTimerInvalidate(timer);
        let ready = Box::from_raw(info as *mut Sender<std::result::Result<CFRunLoop, String>>);
        let _ = ready.send(Ok(CFRunLoop::get_current()));
    }
}

fn handle_event(shared: &Shared, kind: CGEventType, event: &CGEvent) -> Option<CGEvent> {
    match kind {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            warn!(
                kind = kind as u32,
                "macOS disabled the event tap; re-enabling"
            );
            let port = shared.port.load(Ordering::Acquire);
            if !port.is_null() {
                unsafe { CGEventTapEnable(port as CFMachPortRef, true) };
            }
            return None;
        }
        CGEventType::KeyDown | CGEventType::KeyUp | CGEventType::FlagsChanged => {}
        _ => return None,
    }

    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

    // Real keys arrive from the HID system. Draft's own paste chord and any
    // other synthesized input do not, so they can never start a capture
    // unless an automated run asked for it before the tap was created.
    if !shared.allow_injected
        && event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID) != HID_SYSTEM_STATE
    {
        if keycode == shared.keycode {
            tracing::debug!(keycode, "ignoring a synthetic event for the dictation key");
        }
        return None;
    }

    if matches!(kind, CGEventType::FlagsChanged) && keycode == shared.keycode && shared.modifier {
        // Each FlagsChanged for this keycode is one physical transition, in
        // either direction. Toggling is exact even when the other side's
        // modifier is already down.
        let was_holding = shared.holding.fetch_xor(true, Ordering::SeqCst);
        let sent = if was_holding {
            shared.sender.try_send(HotkeyEvent::Released)
        } else {
            shared.sender.try_send(HotkeyEvent::Pressed)
        };
        let _ = sent;
        return swallow(event);
    }

    if keycode == shared.keycode {
        let out = match kind {
            CGEventType::KeyDown if !shared.holding.swap(true, Ordering::SeqCst) => {
                Some(HotkeyEvent::Pressed)
            }
            CGEventType::KeyUp if shared.holding.swap(false, Ordering::SeqCst) => {
                Some(HotkeyEvent::Released)
            }
            _ => None,
        };
        if let Some(out) = out {
            let _ = shared.sender.try_send(out);
        }
        // The dictation key belongs to Draft while the tap is installed.
        return swallow(event);
    }

    if keycode == KEY_ESCAPE {
        if matches!(kind, CGEventType::KeyDown) && shared.holding.load(Ordering::SeqCst) {
            if !shared.escape_down.swap(true, Ordering::SeqCst) {
                let _ = shared.sender.try_send(HotkeyEvent::Cancel);
            }
            return swallow(event);
        }
        if matches!(kind, CGEventType::KeyUp) && shared.escape_down.swap(false, Ordering::SeqCst) {
            return swallow(event);
        }
    }

    None
}

/// Remove a key event from the stream without affecting any later event.
///
/// `core-graphics` 0.24 turns a `None` callback result into the original event,
/// so dropping it is not possible through the safe wrapper. A null-type event
/// is discarded by the same machinery that posts the event the tap returns,
/// which is how an active tap deletes one.
fn swallow(event: &CGEvent) -> Option<CGEvent> {
    event.set_type(CGEventType::Null);
    Some(event.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_key_has_a_keycode() {
        for config in HotkeyConfig::available() {
            assert!(
                keycode(*config).is_some(),
                "{} is offered but has no keycode",
                config.id()
            );
        }
    }

    #[test]
    fn the_default_key_is_the_right_control_key() {
        assert_eq!(keycode(HotkeyConfig::RightCtrl), Some(0x3E));
        assert!(is_modifier(HotkeyConfig::RightCtrl));
    }

    #[test]
    fn the_windows_only_keys_are_not_mapped() {
        assert_eq!(keycode(HotkeyConfig::Pause), None);
        assert_eq!(keycode(HotkeyConfig::ScrollLock), None);
        assert!(!is_modifier(HotkeyConfig::Pause));
    }

    #[test]
    fn only_the_two_side_modifiers_are_flag_changes() {
        assert!(is_modifier(HotkeyConfig::RightAlt));
        assert!(!is_modifier(HotkeyConfig::F8));
        assert!(!is_modifier(HotkeyConfig::F9));
    }
}
