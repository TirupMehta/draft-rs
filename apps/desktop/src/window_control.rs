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

/// Whether a native window is currently visible. The main window hides to the
/// tray, and a hidden window does not need a continuous level-meter repaint.
#[cfg(target_os = "windows")]
pub fn is_visible(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    if hwnd == 0 {
        return false;
    }
    unsafe { IsWindowVisible(hwnd as *mut _) != 0 }
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

/// Tell Windows which appearance the native title bar should use. The window
/// frame belongs to the operating system, so it cannot follow the interface
/// palette on its own.
#[cfg(target_os = "windows")]
pub fn set_title_bar_dark(hwnd: isize, dark: bool) {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};

    if hwnd == 0 {
        return;
    }
    let enabled: i32 = if dark { 1 } else { 0 };
    unsafe {
        DwmSetWindowAttribute(
            hwnd as *mut _,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &enabled as *const i32 as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_title_bar_dark(_hwnd: isize, _dark: bool) {}

/// Whether the system asks for animations. Windows exposes this in the
/// accessibility settings, and an interface that respects it should not move
/// when the person has turned motion off.
#[cfg(target_os = "windows")]
pub fn system_animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    let mut enabled: i32 = 0;
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut i32 as *mut _,
            0,
        );
    }
    enabled != 0
}

#[cfg(not(target_os = "windows"))]
pub fn system_animations_enabled() -> bool {
    true
}

/// Maximize or restore a window that draws its own title bar.
#[cfg(target_os = "windows")]
pub fn toggle_maximize(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsZoomed, SW_MAXIMIZE, SW_RESTORE, ShowWindowAsync,
    };

    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as *mut _;
    let zoomed = unsafe { IsZoomed(hwnd) } != 0;
    unsafe {
        ShowWindowAsync(hwnd, if zoomed { SW_RESTORE } else { SW_MAXIMIZE });
    }
}

#[cfg(not(target_os = "windows"))]
pub fn toggle_maximize(_hwnd: isize) {}

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

#[cfg(not(target_os = "windows"))]
pub fn is_visible(_hwnd: isize) -> bool {
    // There is no cross-platform visibility query here; keep the level meter
    // live rather than silently freezing it on the other shells.
    true
}
