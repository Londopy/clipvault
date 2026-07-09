// platform/windows.rs
// windows-specific stuff: getting the foreground app name and startup registry

use log::{debug, warn};

// uses win32 api to get the title of whatever window is focused right now
pub fn get_foreground_app() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    // unsafe because we're calling raw win32 ffi, but this is pretty standard stuff
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        // GetWindowTextW fills our buffer with the window title as utf-16
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len == 0 {
            return None;
        }
        let title = OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .into_owned();
        Some(title)
    }
}

// link to user32.dll which has the window functions we need
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetWindowTextW(hwnd: *mut std::ffi::c_void, lp_string: *mut u16, n_max_count: i32) -> i32;
}

// registry path for programs that run at login
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "ClipVault";

pub fn register_startup() {
    match get_startup_path() {
        Some(path) => {
            if let Err(e) = write_registry_run(APP_NAME, &path) {
                warn!("Could not register startup: {e}");
            } else {
                debug!("Registered startup: {path}");
            }
        }
        None => warn!("Could not determine exe path for startup registration"),
    }
}

pub fn unregister_startup() {
    let _ = delete_registry_run(APP_NAME);
}

fn get_startup_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| format!("\"{}\"", s)))
}

// writes the Run key via reg.exe so we don't need a registry crate.
// CREATE_NO_WINDOW keeps the helper from flashing a console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn write_registry_run(name: &str, value: &str) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let key = format!(r"HKCU\{RUN_KEY}");
    let status = std::process::Command::new("reg")
        .args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", value, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("reg add exited with {status}")
    }
}

fn delete_registry_run(name: &str) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let key = format!(r"HKCU\{RUN_KEY}");
    let status = std::process::Command::new("reg")
        .args(["delete", &key, "/v", name, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("reg delete exited with {status}")
    }
}
