// platform/macos.rs
// mac-specific stuff: getting the frontmost app, startup launchagent, accessibility check

use log::{debug, warn};
use std::path::PathBuf;

// runs osascript to ask system events which app is in front
// way easier than doing objc ffi even if it's a bit slow
pub fn get_frontmost_app() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get name of first process whose frontmost is true"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

// checks if accessibility is granted - returns true as a placeholder
// if it's not granted rdev will just fail at runtime which is fine for now
// the real check would call AXIsProcessTrustedWithOptions via ffi
pub fn check_accessibility_permission() -> bool {
    true
}

// installs a launchagent plist so we start at login
pub fn register_startup() {
    let plist_path = launch_agent_path();
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            warn!("Could not find exe path: {e}");
            return;
        }
    };

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.clipvault.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>"#,
        exe.display()
    );

    if let Some(parent) = plist_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&plist_path, plist) {
        Ok(_) => debug!("LaunchAgent installed: {}", plist_path.display()),
        Err(e) => warn!("Failed to install LaunchAgent: {e}"),
    }
}

pub fn unregister_startup() {
    let path = launch_agent_path();
    let _ = std::fs::remove_file(&path);
    // also unload it from launchd if it's currently running
    let _ = std::process::Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .output();
}

fn launch_agent_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library")
        .join("LaunchAgents")
        .join("com.clipvault.daemon.plist")
}
