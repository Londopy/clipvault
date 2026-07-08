// notify.rs
// sends desktop notifications - each platform has its own api so we just
// pick the right one at compile time using cfg flags

use anyhow::Result;
use log::debug;

use crate::config::Config;

// call this anywhere you want to show a notification, it handles the rest
pub fn send_notification(title: &str, body: &str, config: &Config) -> Result<()> {
    if !config.notifications.enabled {
        return Ok(());
    }
    debug!("Notification: {title} — {body}");
    platform_notify(title, body, config.notifications.duration_ms)
}

// windows uses winrt toast notifications
#[cfg(target_os = "windows")]
fn platform_notify(title: &str, body: &str, _duration_ms: u64) -> Result<()> {
    use winrt_notification::{Duration, Toast};
    Toast::new(Toast::POWERSHELL_APP_ID)
        .title(title)
        .text1(body)
        .duration(Duration::Short)
        .sound(None)
        .show()
        .map_err(|e| anyhow::anyhow!("WinRT notification error: {e:?}"))
}

// mac uses its own notification system
#[cfg(target_os = "macos")]
fn platform_notify(title: &str, body: &str, _duration_ms: u64) -> Result<()> {
    mac_notification_sys::send_notification(title, None, body, None)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("macOS notification error: {e}"))
}

// linux uses notify-rust which wraps libnotify / dbus
#[cfg(target_os = "linux")]
fn platform_notify(title: &str, body: &str, duration_ms: u64) -> Result<()> {
    notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(duration_ms as u32))
        .show()?;
    Ok(())
}

// anything else just does nothing silently
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_notify(_title: &str, _body: &str, _duration_ms: u64) -> Result<()> {
    Ok(())
}
