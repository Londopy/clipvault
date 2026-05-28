// tray.rs
// system tray icon and the little right-click menu
// tray-icon 0.24 uses string ids now instead of numbers, kinda weird but whatever

use anyhow::Result;
use log::debug;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

// string ids for each menu item - we match against these later
pub const ID_OPEN: &str = "open";
pub const ID_PASTE_LAST: &str = "paste_last";
pub const ID_PAUSE: &str = "pause";
pub const ID_CLEAR: &str = "clear";
pub const ID_OPEN_CONFIG: &str = "open_config";
pub const ID_TOGGLE_START: &str = "toggle_startup";
pub const ID_QUIT: &str = "quit";

pub struct Tray {
    _icon: TrayIcon,
    item_pause: MenuItem, // need to hold onto this so we can change its label
    item_startup: MenuItem, // same for the launch at login toggle
}

impl Tray {
    pub fn new(auto_start: bool) -> Result<Self> {
        let icon = load_icon();

        let item_open = MenuItem::with_id(ID_OPEN, "Open ClipVault", true, None);
        let item_paste_last = MenuItem::with_id(ID_PASTE_LAST, "Paste Last Item", true, None);
        let item_pause = MenuItem::with_id(ID_PAUSE, "Pause Recording", true, None);
        let item_clear = MenuItem::with_id(ID_CLEAR, "Clear History", true, None);
        let item_open_config = MenuItem::with_id(ID_OPEN_CONFIG, "Open Config Folder", true, None);
        // shows a checkmark when startup is enabled
        let startup_label = if auto_start { "✓ Launch at Login" } else { "Launch at Login" };
        let item_startup = MenuItem::with_id(ID_TOGGLE_START, startup_label, true, None);
        let item_quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

        let menu = Menu::with_items(&[
            &item_open,
            &item_paste_last,
            &PredefinedMenuItem::separator(),
            &item_pause,
            &item_clear,
            &PredefinedMenuItem::separator(),
            &item_startup,
            &item_open_config,
            &PredefinedMenuItem::separator(),
            &item_quit,
        ])?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ClipVault")
            .with_icon(icon)
            .build()?;

        Ok(Self { _icon: tray_icon, item_pause, item_startup })
    }

    // shows how many items are in history in the tooltip
    pub fn set_count(&mut self, count: usize) {
        debug!("Tray count: {count}");
        let _ = self._icon.set_tooltip(Some(format!("ClipVault — {count} items")));
    }

    // toggles the pause button text so the user knows what state its in
    pub fn set_paused(&mut self, paused: bool) {
        self.item_pause.set_text(if paused { "Resume Recording" } else { "Pause Recording" });
    }

    // updates the launch at login label to show current state
    pub fn set_auto_start(&mut self, enabled: bool) {
        self.item_startup.set_text(if enabled { "✓ Launch at Login" } else { "Launch at Login" });
    }
}

// checks if there's a menu click waiting and returns the id string if so
pub fn poll_menu_event() -> Option<String> {
    MenuEvent::receiver().try_recv().ok().map