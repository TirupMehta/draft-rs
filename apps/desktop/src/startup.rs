//! Per-user Windows startup registration.
//!
//! Draft keeps the choice in settings and projects it onto the normal
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value. The entry starts
//! the installed executable with `--background`, so signing in leaves Draft in
//! the tray rather than putting a window on screen.

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Draft";

    pub fn apply(enabled: bool) -> io::Result<()> {
        if enabled { install() } else { remove() }
    }

    fn install() -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let mut command = OsString::from("\"");
        command.push(exe.as_os_str());
        command.push("\" --background");
        set_run_value(command.as_os_str())
    }

    fn set_run_value(command: &OsStr) -> io::Result<()> {
        let key = open_run_key()?;
        let name = wide(VALUE_NAME);
        let mut data: Vec<u16> = command.encode_wide().collect();
        data.push(0);
        let status = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * std::mem::size_of::<u16>()) as u32,
            )
        };
        unsafe { RegCloseKey(key) };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    }

    fn remove() -> io::Result<()> {
        let key = open_run_key()?;
        let name = wide(VALUE_NAME);
        let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        unsafe { RegCloseKey(key) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    }

    fn open_run_key() -> io::Result<HKEY> {
        let subkey = wide(RUN_KEY);
        let mut key: HKEY = ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        };
        if status == ERROR_SUCCESS {
            Ok(key)
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(target_os = "windows")]
pub use windows::apply;

#[cfg(not(target_os = "windows"))]
pub fn apply(_enabled: bool) -> std::io::Result<()> {
    Ok(())
}
