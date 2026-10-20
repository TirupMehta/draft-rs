//! Native window bits that GPUI does not expose on its own.
//!
//! The main window hides to the tray instead of quitting, and the dictation
//! HUD is a transparent, topmost, click-through popup. Both need the raw
//! HWND. Everything here is a no-op away from Windows so the shell still
//! builds on macOS and Linux while those backends are being wired.

use gpui::Window;

#[cfg(target_os = "windows")]
pub fn hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

/// Hide the window without tearing down the application. The request is
/// posted with `ShowWindowAsync` so a synchronous `WM_SHOWWINDOW` cannot
/// re-enter GPUI's message handling while the app is already being updated.
/// Returns true when the window handle was valid.
#[cfg(target_os = "windows")]
pub fn hide(window: &Window) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindowAsync};

    let Some(hwnd) = hwnd(window) else {
        return false;
    };
    unsafe { ShowWindowAsync(hwnd as *mut _, SW_HIDE) };
    true
}

/// Restore and focus a window that [`hide`] put away. Restoring is posted so
/// the resulting visibility messages are handled outside the current update.
#[cfg(target_os = "windows")]
pub fn show(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
    };

    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as *mut _;
    unsafe {
        ShowWindowAsync(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
}

/// Show or hide the HUD without blocking the caller. `ShowWindowAsync` posts
/// the request, which keeps the HUD's `WM_SHOWWINDOW` draw callback out of the
/// update that changed the dictation phase.
#[cfg(target_os = "windows")]
pub fn set_hud_visible(hwnd: isize, visible: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOWNOACTIVATE, ShowWindowAsync,
    };

    if hwnd == 0 {
        return;
    }
    let command = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
    unsafe { ShowWindowAsync(hwnd as *mut _, command) };
}

/// Make the HUD a topmost, non-activating, click-through popup.
pub fn configure_hud(hwnd: isize) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SetWindowLongPtrW, SetWindowPos, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
        };

        if hwnd == 0 {
            return;
        }
        let hwnd = hwnd as *mut _;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                style | (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize,
            );
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = hwnd;
    }
}

#[cfg(not(target_os = "windows"))]
pub fn hwnd(_window: &Window) -> Option<isize> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn hide(_window: &Window) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn show(_hwnd: isize) {}

#[cfg(not(target_os = "windows"))]
pub fn set_hud_visible(_hwnd: isize, _visible: bool) {}
