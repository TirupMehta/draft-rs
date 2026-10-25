//! Per-user sign-in registration.
//!
//! Draft keeps the choice in settings and projects it onto the platform's own
//! mechanism, so the operating system, not a private flag, is the record. On
//! Windows that is the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! value; on macOS it is a LaunchAgent in `~/Library/LaunchAgents`. Both start
//! the installed executable with `--background`, so signing in leaves Draft in
//! the menu bar rather than putting a window on screen.
//!
//! macOS 13 prefers `SMAppService`, which requires the signed `.app` that stage
//! 7 of `docs/MACOS.md` has not produced yet. A LaunchAgent needs no bundle and
//! has the same sign-in semantics as the Windows value, so it is the honest
//! implementation until there is something to register with `SMAppService`.

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

#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::path::{Path, PathBuf};

    /// The reverse-DNS label the login item uses. The bundle identifier in
    /// stage 7 should use the same name so the two records never diverge.
    const LABEL: &str = "in.tirup.draft";

    pub fn apply(enabled: bool) -> io::Result<()> {
        if enabled { install() } else { remove() }
    }

    fn install() -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let path = plist_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, plist(&exe))
    }

    fn remove() -> io::Result<()> {
        match std::fs::remove_file(plist_path()?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// `~/Library/LaunchAgents/<label>.plist`. Launchd reads it at sign-in, so
    /// enabling the setting has the same effect as the Windows `Run` value: it
    /// applies to the next sign-in, not the current session.
    fn plist_path() -> io::Result<PathBuf> {
        let home = std::env::var_os("HOME").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "HOME is not set in this session")
        })?;
        Ok(Path::new(&home)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")))
    }

    fn plist(exe: &Path) -> String {
        let exe = escape(&exe.to_string_lossy());
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--background</string>
  </array>
  <key>LimitLoadToSessionType</key>
  <string>Aqua</string>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#
        )
    }

    /// Escape the five XML entities. A path can legitimately contain `&`.
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    #[cfg(test)]
    mod tests {
        /// Launchd reads this file at sign-in, so a malformed plist fails
        /// silently a day later. A path can contain `&`; the escaping is the
        /// part that can break the document.
        #[test]
        fn the_plist_escapes_the_executable_path() {
            let plist = super::plist(std::path::Path::new("/Users/a&b/Draft.app/draft"));
            assert!(plist.contains("/Users/a&amp;b/Draft.app/draft"));
            assert!(plist.contains("<string>--background</string>"));
            assert!(plist.contains("<key>RunAtLoad</key>"));
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::apply;

#[cfg(target_os = "macos")]
pub use macos::apply;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn apply(_enabled: bool) -> std::io::Result<()> {
    Ok(())
}
