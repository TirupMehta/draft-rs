//! Windows low-level keyboard hook.
//!
//! `WH_KEYBOARD_LL` is the only way to see a bare modifier such as Right Ctrl.
//! The hook runs on its own message-loop thread and never blocks: it pushes a
//! small event into a bounded channel and returns.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
    PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::{HotkeyConfig, HotkeyEvent};

/// Right Ctrl, the default dictation key.
const VK_RCONTROL: u32 = 0xA3;
/// Right Alt / AltGr.
const VK_RMENU: u32 = 0xA5;
/// Escape, which cancels an active capture.
const VK_ESCAPE: u32 = 0x1B;
/// F8, useful while another dictation app owns Right Ctrl.
const VK_F8: u32 = 0x77;
/// F9.
const VK_F9: u32 = 0x78;
/// Pause/Break.
const VK_PAUSE: u32 = 0x13;
/// Scroll Lock.
const VK_SCROLL: u32 = 0x91;

/// The Windows virtual-key code for a configured key.
///
/// The mapping lives here rather than in `HotkeyConfig` so the shared type
/// carries no Windows numbers. A macOS event tap will need its own table.
fn virtual_key(config: HotkeyConfig) -> u32 {
    match config {
        HotkeyConfig::RightCtrl => VK_RCONTROL,
        HotkeyConfig::RightAlt => VK_RMENU,
        HotkeyConfig::F8 => VK_F8,
        HotkeyConfig::F9 => VK_F9,
        HotkeyConfig::Pause => VK_PAUSE,
        HotkeyConfig::ScrollLock => VK_SCROLL,
    }
}

/// A leaked shared record. Leaking one per listener keeps the hook callback
/// lock-free and safe even if a callback is in flight during shutdown.
struct Shared {
    sender: SyncSender<HotkeyEvent>,
    virtual_key: u32,
    holding: AtomicBool,
    escape_down: AtomicBool,
}

static SHARED: AtomicPtr<Shared> = AtomicPtr::new(std::ptr::null_mut());
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Test-only escape hatch for automated end-to-end runs. Synthetic input is
/// ignored by default so Draft's own clipboard paste can never start a capture.
static ALLOW_INJECTED: OnceLock<bool> = OnceLock::new();

fn allow_injected() -> bool {
    *ALLOW_INJECTED.get_or_init(|| std::env::var_os("DRAFT_ALLOW_INJECTED_KEYS").is_some())
}

/// The owner of the hook thread. Dropping it stops the hook.
pub struct HotkeyListener {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // Stop publishing new events before asking the thread to exit.
            SHARED.store(std::ptr::null_mut(), Ordering::SeqCst);
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if thread.join().is_err() {
                warn!("hotkey thread panicked");
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    }
}

pub fn start(config: HotkeyConfig) -> Result<(HotkeyListener, Receiver<HotkeyEvent>)> {
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        bail!("the global hotkey listener is already running");
    }

    if allow_injected() {
        warn!("synthetic keyboard input is enabled (test mode)");
    }

    let (sender, receiver) = sync_channel(4096);
    let shared = Box::new(Shared {
        sender,
        virtual_key: virtual_key(config),
        holding: AtomicBool::new(false),
        escape_down: AtomicBool::new(false),
    });
    SHARED.store(Box::into_raw(shared), Ordering::SeqCst);

    let (ready_sender, ready_receiver) = channel::<std::result::Result<u32, String>>();
    let thread = thread::Builder::new()
        .name("draft-hotkey".to_owned())
        .spawn(move || run_hook(ready_sender))
        .context("spawn hotkey thread");

    let thread = match thread {
        Ok(thread) => thread,
        Err(error) => {
            SHARED.store(std::ptr::null_mut(), Ordering::SeqCst);
            RUNNING.store(false, Ordering::SeqCst);
            return Err(error);
        }
    };

    match ready_receiver.recv() {
        Ok(Ok(thread_id)) => {
            info!(
                virtual_key = virtual_key(config),
                key = config.id(),
                "global hotkey ready"
            );
            Ok((
                HotkeyListener {
                    thread_id,
                    thread: Some(thread),
                },
                receiver,
            ))
        }
        Ok(Err(error)) => {
            SHARED.store(std::ptr::null_mut(), Ordering::SeqCst);
            RUNNING.store(false, Ordering::SeqCst);
            let _ = thread.join();
            bail!("install keyboard hook: {error}");
        }
        Err(_) => {
            SHARED.store(std::ptr::null_mut(), Ordering::SeqCst);
            RUNNING.store(false, Ordering::SeqCst);
            let _ = thread.join();
            bail!("hotkey thread exited before it was ready");
        }
    }
}

fn run_hook(ready: Sender<std::result::Result<u32, String>>) {
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0);
        if hook.is_null() {
            let error = std::io::Error::last_os_error();
            let _ = ready.send(Err(error.to_string()));
            return;
        }

        // Create the message queue before reporting readiness so a stop that
        // arrives immediately can post WM_QUIT.
        let mut message = MSG::default();
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 0);

        let thread_id = GetCurrentThreadId();
        if ready.send(Ok(thread_id)).is_err() {
            UnhookWindowsHookEx(hook);
            return;
        }

        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        UnhookWindowsHookEx(hook);
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }

    let shared = SHARED.load(Ordering::Acquire);
    if shared.is_null() {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let shared = unsafe { &*shared };
    let keyboard = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };

    // Never react to synthetic input, including Draft's own paste, unless an
    // automated test explicitly opted in before the listener started.
    if keyboard.flags & LLKHF_INJECTED != 0 && !allow_injected() {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }

    let message = wparam as u32;
    let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    let up = message == WM_KEYUP || message == WM_SYSKEYUP;

    if keyboard.vkCode == shared.virtual_key {
        if down && !shared.holding.swap(true, Ordering::SeqCst) {
            let _ = shared.sender.try_send(HotkeyEvent::Pressed);
        } else if up && shared.holding.swap(false, Ordering::SeqCst) {
            let _ = shared.sender.try_send(HotkeyEvent::Released);
        }
        // The dictation key belongs to Draft while the listener is running.
        return 1;
    }

    if keyboard.vkCode == VK_ESCAPE {
        if down && shared.holding.load(Ordering::SeqCst) {
            if !shared.escape_down.swap(true, Ordering::SeqCst) {
                let _ = shared.sender.try_send(HotkeyEvent::Cancel);
            }
            return 1;
        }
        if up && shared.escape_down.swap(false, Ordering::SeqCst) {
            return 1;
        }
    }

    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}
