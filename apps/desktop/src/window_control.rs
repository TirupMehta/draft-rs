//! Native window bits that GPUI does not expose on its own.
//!
//! The main window hides to the tray instead of quitting, and the dictation
//! HUD is a transparent, topmost, click-through popup. Both need the raw
//! native window: an `HWND` on Windows, the `NSView` pointer on macOS. The
//! handle is passed around as an `isize` so the call sites do not care.
//!
//! Everything here is a no-op on Linux so the shell still builds while that
//! backend is being wired.

// macOS reaches GPUI's `Window` through the module below; the other targets
// use it in the free functions here.
#[cfg(not(target_os = "macos"))]
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
#[cfg(target_os = "windows")]
pub fn configure_hud(hwnd: isize) {
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

/// AppKit window behavior for the main window and the HUD.
///
/// GPUI creates both windows, but it does not expose the two things Draft
/// needs: the HUD has to float above every other application and ignore
/// clicks, and the main window has to disappear without the process exiting.
#[cfg(target_os = "macos")]
mod macos {
    use gpui::Window;
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSApplication, NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior, NSWorkspace,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// `NSStatusWindowLevel`. Above the normal and floating levels and below
    /// the menu bar, which is where a dictation HUD belongs.
    const STATUS_WINDOW_LEVEL: isize = 25;

    /// The `NSView` GPUI renders into, as an opaque handle. Windows stores an
    /// `HWND`; macOS stores the view because every window call goes through
    /// it and the view cannot be confused with a window number.
    pub fn hwnd(window: &Window) -> Option<isize> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr() as isize),
            _ => None,
        }
    }

    /// The window that owns a view handle.
    ///
    /// `NSView::window()` returns `None` while a view is not in a window, so
    /// this is also the honest "the window is gone" case.
    fn window_for(handle: isize) -> Option<Retained<NSWindow>> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle came from `hwnd()`, which borrows it from a live
        // GPUI window, and every caller holds that window for the call.
        let view = unsafe { &*(handle as *const NSView) };
        view.window()
    }

    /// Hide the window without tearing down the application.
    pub fn hide(window: &Window) -> bool {
        let Some(window) = hwnd(window).and_then(window_for) else {
            return false;
        };
        window.orderOut(None);
        true
    }

    /// Restore and focus a window that [`hide`] put away.
    pub fn show(handle: isize) {
        let Some(window) = window_for(handle) else {
            return;
        };
        window.makeKeyAndOrderFront(None);

        // A menu-bar application is not active when its menu is clicked, so
        // ordering the window front is not enough to give it the keyboard.
        // The app has to ask for activation as well.
        if let Some(mtm) = MainThreadMarker::new() {
            #[allow(deprecated)]
            NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
        }
    }

    /// Whether a native window is currently visible. The main window hides to
    /// the menu bar, and a hidden window does not need a continuous level-meter
    /// repaint.
    pub fn is_visible(handle: isize) -> bool {
        window_for(handle).is_some_and(|window| window.isVisible())
    }

    /// Show or hide the HUD. `orderFrontRegardless` shows the window without
    /// activating Draft, which is what keeps the dictation target focused.
    pub fn set_hud_visible(handle: isize, visible: bool) {
        let Some(window) = window_for(handle) else {
            return;
        };
        if visible {
            window.orderFrontRegardless();
        } else {
            window.orderOut(None);
        }
    }

    /// Make the HUD float over other applications and let clicks pass through.
    pub fn configure_hud(handle: isize) {
        let Some(window) = window_for(handle) else {
            return;
        };
        window.setLevel(STATUS_WINDOW_LEVEL);
        window.setIgnoresMouseEvents(true);
        // A HUD that disappears when Draft loses focus is useless: the whole
        // point is that someone else has focus.
        window.setHidesOnDeactivate(false);
        window.setCanHide(false);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }

    /// macOS draws no native title bar for Draft, so there is nothing to
    /// recolour. The window is one surface painted by the interface.
    pub fn set_title_bar_dark(_handle: isize, _dark: bool) {}

    /// Draft draws its own minimize and close controls on the right, matching
    /// the Windows build. AppKit's traffic lights sit at the top left, where
    /// they would overlap the brand mark and duplicate those controls. Hide
    /// them; the window's close handler still runs for Cmd+W and the app's own
    /// close button.
    pub fn configure_main_window(handle: isize) {
        let Some(window) = window_for(handle) else {
            return;
        };
        for kind in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            if let Some(button) = window.standardWindowButton(kind) {
                button.setHidden(true);
            }
        }
    }

    /// macOS calls this "reduce motion".
    pub fn system_animations_enabled() -> bool {
        !NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
}

#[cfg(target_os = "macos")]
pub use macos::{
    configure_hud, configure_main_window, hide, hwnd, is_visible, set_hud_visible,
    set_title_bar_dark, show, system_animations_enabled,
};

/// Windows and Linux have no AppKit traffic lights to hide. Draft draws its own
/// window controls on every platform, so this is a no-op there.
#[cfg(not(target_os = "macos"))]
pub fn configure_main_window(_handle: isize) {}

// The Linux shell has no window backend wired yet. Keeping the same signatures
// means the call sites compile unchanged; visibility reports true so the level
// meter keeps moving rather than silently freezing.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn hwnd(_window: &Window) -> Option<isize> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn hide(_window: &Window) -> bool {
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn show(_hwnd: isize) {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_hud_visible(_hwnd: isize, _visible: bool) {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn is_visible(_hwnd: isize) -> bool {
    true
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn configure_hud(_hwnd: isize) {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_title_bar_dark(_hwnd: isize, _dark: bool) {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn system_animations_enabled() -> bool {
    true
}
