// platform/linux.rs
// linux-specific stuff: active window via xdotool, startup via systemd or xdg autostart
// wayland support is basically nonexistent here, sorry

use log::{debug, warn};
use std::path::PathBuf;

// uses xdotool to get the active window title - works fine on x11
// wayland doesn't have a standard way to do this so we just give up there
pub fn get_active_app() -> Option<String> {
    let output = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    // wayland fallback would go here if there was a standard way to do it
    None
}

// try systemd first, fall back to xdg autostart .desktop file
pub fn register_startup() {
    if try_systemd_startup() {
        return;
    }
    xdg_autostart();
}

pub fn unregister_startup() {
    let unit_path = systemd_unit_path();
    if unit_path.exists() {
        let _ = std::fs::remove_file(&unit_path);
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
    }
    let desktop_path = xdg_autostart_path();
    let _ = std::fs::remove_file(&desktop_path);
}

fn try_systemd_startup() -> bool {
    let unit_path = systemd_unit_path();
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let unit = format!(
        "[Unit]\nDescription=ClipVault clipboard manager\nAfter=graphical-session.target\n\n\
         [Service]\nType=simple\nExecStart={}\nRestart=on-failure\n\n\
         [Install]\nWantedBy=default.target\n",
        exe.display()
    );

    if let Some(parent) = unit_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&unit_path, unit).is_err() {
        return false;
    }
    let reload = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    if reload.is_err() {
        return false;
    }
    let enable = std::process::Command::new("systemctl")
        .args(["--user", "enable", "clipvault.service"])
        .output();
    if enable.map(|o| o.status.success()).unwrap_or(false) {
        debug!("Registered systemd user unit");
        return true;
    }
    false
}

fn xdg_autostart() {
    let desktop_path = xdg_autostart_path();
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => { warn!("Could not find exe path: {e}"); return; }
    };

    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=ClipVault\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    );

    if let Some(parent) = desktop_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&desktop_path, desktop) {
        Ok(_) => debug!("XDG autostart installed: {}", desktop_path.display()),
        Err(e) => warn!("XDG autostart failed: {e}"),
    }
}

fn systemd_unit_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("systemd")
        .join("user")
        .join("clipvault.service")
}

fn xdg_autostart_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("autostart")
        .join("clipvault.desktop")
}
