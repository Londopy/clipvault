// platform/mod.rs
// picks the right platform-specific code at compile time
// each os has its own submodule with the actual implementation

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

// figures out which app the user just copied from
pub fn get_source_app() -> Option<String> {
    #[cfg(target_os = "windows")]
    return windows::get_foreground_app();

    #[cfg(target_os = "macos")]
    return macos::get_frontmost_app();

    #[cfg(target_os = "linux")]
    return linux::get_active_app();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return None;
}

// registers clipvault to run at login
pub fn register_startup() {
    #[cfg(target_os = "windows")]
    windows::register_startup();

    #[cfg(target_os = "macos")]
    macos::register_startup();

    #[cfg(target_os = "linux")]
    linux::register_startup();
}

// removes it from startup
pub fn unregister_startup() {
    #[cfg(target_os = "windows")]
    windows::unregister_startup();

    #[cfg(target_os = "macos")]
    macos::unregister_startup();

    #[cfg(target_os = "linux")]
    linux::unregister_startup();
}

// macos needs accessibility permission for rdev to see keyboard events
pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    return macos::check_accessibility_permission();

    #[cfg(not(target_os = "macos"))]
    return true;
}
