// gui/mod.rs
// the main app struct and all the event handling glue
// eframe calls update() on every frame and we dispatch events from there

pub mod overlay;
pub mod preview;
pub mod theme;

use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};
use std::time::Instant;

use anyhow::Result;
use arboard::Clipboard;
use eframe::egui::{self, Context};
use log::debug;

use crate::config::Config;
use crate::notify;
use crate::paste;
use crate::snippets::SnippetStore;
use crate::store::Store;
use crate::tray::{
    self, Tray, ID_CLEAR, ID_OPEN, ID_OPEN_CONFIG, ID_PASTE_LAST, ID_PAUSE, ID_QUIT,
    ID_TOGGLE_START,
};

use self::overlay::{Overlay, OverlayAction, Tab};
use self::theme::{apply_style, build_visuals, parse_hex_color, Palette};

// all the things that can happen - hotkeys and tray clicks both send these
#[derive(Debug)]
pub enum AppEvent {
    OpenHistory,
    OpenSnippets,
    ClearClipboard,
    PasteLast,
    ToggleIncognito,
    InstantPaste(usize),
    UpdateAvailable(String),
    OpenSettings,
}

// the main app state - everything lives in here
struct ClipVaultApp {
    store: Arc<Mutex<Store>>,
    config: Arc<Mutex<Config>>,
    snippets: Arc<Mutex<SnippetStore>>,
    event_tx: Sender<AppEvent>,
    event_rx: Receiver<AppEvent>,

    tray: Option<Tray>,
    overlay: Overlay,
    show_overlay: bool,
    palette: Palette,

    // For periodic save
    last_save: Instant,
}

impl ClipVaultApp {
    fn new(
        cc: &eframe::CreationContext,
        store: Arc<Mutex<Store>>,
        config: Arc<Mutex<Config>>,
        snippets: Arc<Mutex<SnippetStore>>,
        event_tx: Sender<AppEvent>,
        event_rx: Receiver<AppEvent>,
    ) -> Self {
        let palette = build_palette(&config.lock().unwrap());
        cc.egui_ctx.set_visuals(build_visuals(&palette));
        apply_style(&cc.egui_ctx);

        // using egui default fonts for now, can customize later
        cc.egui_ctx.set_fonts(egui::FontDefinitions::default());

        let auto_start = config.lock().unwrap().general.auto_start;
        let tray = Tray::new(auto_start).ok();

        Self {
            store,
            config,
            snippets,
            event_tx,
            event_rx,
            tray,
            overlay: Overlay::default(),
            show_overlay: false,
            palette,
            last_save: Instant::now(),
        }
    }

    fn open_overlay(&mut self, tab: Tab) {
        self.overlay.reset_for_open(tab);
        self.show_overlay = true;
    }

    fn close_overlay(&mut self) {
        self.show_overlay = false;
    }

    fn handle_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::OpenHistory => {
                    debug!("Event: OpenHistory");
                    self.open_overlay(Tab::History);
                }
                AppEvent::OpenSnippets => {
                    debug!("Event: OpenSnippets");
                    self.open_overlay(Tab::Snippets);
                }
                AppEvent::ClearClipboard => {
                    debug!("Event: ClearClipboard");
                    self.store.lock().unwrap().clear(false);
                    let _ = Clipboard::new().and_then(|mut c| c.clear());
                }
                AppEvent::PasteLast => {
                    debug!("Event: PasteLast");
                    let data = self
                        .store
                        .lock()
                        .unwrap()
                        .history
                        .front()
                        .map(|e| e.data.clone());
                    if let Some(text) = data {
                        let _ = paste::paste_text(&text);
                    }
                }
                AppEvent::ToggleIncognito => {
                    debug!("Event: ToggleIncognito");
                    let mut s = self.store.lock().unwrap();
                    s.incognito = !s.incognito;
                    let on = s.incognito;
                    drop(s);
                    let cfg = self.config.lock().unwrap();
                    if cfg.notifications.enabled {
                        let msg = if on {
                            "Incognito mode ON"
                        } else {
                            "Incognito mode OFF"
                        };
                        let _ = notify::send_notification("ClipVault", msg, &cfg);
                    }
                }
                AppEvent::InstantPaste(n) => {
                    debug!("Event: InstantPaste({n})");
                    let data = {
                        let s = self.store.lock().unwrap();
                        s.history
                            .iter()
                            .filter(|e| !e.is_pinned)
                            .nth(n.saturating_sub(1))
                            .map(|e| e.data.clone())
                    };
                    if let Some(text) = data {
                        let _ = paste::paste_text(&text);
                    }
                }
                AppEvent::UpdateAvailable(version) => {
                    debug!("Event: UpdateAvailable({version})");
                    let cfg = self.config.lock().unwrap();
                    if cfg.notifications.enabled && cfg.notifications.on_update {
                        let _ = notify::send_notification(
                            "ClipVault Update",
                            &format!("Version {version} is available"),
                            &cfg,
                        );
                    }
                }
                AppEvent::OpenSettings => {
                    debug!("Event: OpenSettings");
                    self.open_overlay(Tab::Settings);
                }
            }
        }
    }

    fn handle_tray_events(&mut self, ctx: &Context) {
        if let Some(id) = tray::poll_menu_event() {
            match id.as_str() {
                ID_OPEN => self.open_overlay(Tab::History),
                ID_PASTE_LAST => {
                    let _ = self.event_tx.send(AppEvent::PasteLast);
                }
                ID_PAUSE => {
                    let mut s = self.store.lock().unwrap();
                    s.paused = !s.paused;
                    let paused = s.paused;
                    drop(s);
                    if let Some(ref mut tray) = self.tray {
                        tray.set_paused(paused);
                    }
                }
                ID_CLEAR => {
                    self.store.lock().unwrap().clear(true);
                }
                ID_TOGGLE_START => {
                    // flip the auto_start setting, save it, and update the platform startup entry
                    let new_val = {
                        let mut cfg = self.config.lock().unwrap();
                        cfg.general.auto_start = !cfg.general.auto_start;
                        cfg.general.auto_start
                    };
                    let _ = self.config.lock().unwrap().save();
                    if new_val {
                        crate::platform::register_startup();
                    } else {
                        crate::platform::unregister_startup();
                    }
                    if let Some(ref mut tray) = self.tray {
                        tray.set_auto_start(new_val);
                    }
                }
                ID_OPEN_CONFIG => {
                    Config::open_config_dir();
                }
                ID_QUIT => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }
    }

    fn update_tray_count(&mut self) {
        let count = self.store.lock().unwrap().len();
        if let Some(ref mut tray) = self.tray {
            tray.set_count(count);
        }
    }

    fn periodic_save(&mut self) {
        // save every 30 seconds in case something crashes
        if self.last_save.elapsed().as_secs() >= 30 {
            let cfg = self.config.lock().unwrap();
            if cfg.general.persist_history {
                let _ = self.store.lock().unwrap().save();
            }
            self.last_save = Instant::now();
        }
    }

    fn rebuild_palette(&mut self, ctx: &Context) {
        self.palette = build_palette(&self.config.lock().unwrap());
        ctx.set_visuals(build_visuals(&self.palette));
        apply_style(ctx);
    }
}

impl eframe::App for ClipVaultApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // drain the event queue first
        self.handle_events();
        self.handle_tray_events(ctx);

        // housekeeping stuff
        self.update_tray_count();
        self.periodic_save();

        // repaint fast enough that tray clicks and hotkeys feel instant
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        if self.show_overlay {
            let snippets = Arc::clone(&self.snippets);
            let action =
                self.overlay
                    .show(ctx, &self.store, &snippets, &self.config, &self.palette);

            match action {
                OverlayAction::None => {}
                OverlayAction::SettingsChanged => {
                    // theme or something visual changed - rebuild the palette
                    self.rebuild_palette(ctx);
                }
                OverlayAction::PasteEntry(data) => {
                    let cfg = self.config.lock().unwrap();
                    let notify = cfg.notifications.enabled && cfg.notifications.on_paste;
                    drop(cfg);
                    self.close_overlay();
                    let _ = paste::paste_text(&data);
                    if notify {
                        let cfg = self.config.lock().unwrap();
                        let _ = notify::send_notification("ClipVault", "Pasted from history", &cfg);
                    }
                }
                OverlayAction::DeleteEntry(id) => {
                    self.store.lock().unwrap().remove(&id);
                }
                OverlayAction::Close => {
                    self.close_overlay();
                }
            }
        } else {
            // need an empty panel here or eframe will exit, kinda annoying
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |_ui| {});
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // save one last time when the app closes
        let cfg = self.config.lock().unwrap();
        if cfg.general.persist_history {
            let _ = self.store.lock().unwrap().save();
        }
    }
}

// entry point - sets up the window and hands control to eframe
pub fn run(
    store: Arc<Mutex<Store>>,
    config: Arc<Mutex<Config>>,
    event_tx: Sender<AppEvent>,
    event_rx: Receiver<AppEvent>,
) -> Result<()> {
    let snippets = Arc::new(Mutex::new(SnippetStore::load()?));

    // register with the os startup if the user wants that
    {
        let cfg = config.lock().unwrap();
        if cfg.general.auto_start {
            crate::platform::register_startup();
        }
    }

    let (viewport_w, viewport_h) = (480.0, 600.0);

    // load the icon for the window (egui uses it as the taskbar/alt-tab icon)
    let window_icon = load_window_icon();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ClipVault")
            .with_inner_size([viewport_w, viewport_h])
            .with_min_inner_size([360.0, 400.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_icon(window_icon)
            .with_visible(false), // hidden by default, shows up when you hit the hotkey
        ..Default::default()
    };

    eframe::run_native(
        "ClipVault",
        native_options,
        Box::new(|cc| {
            Ok(Box::new(ClipVaultApp::new(
                cc, store, config, snippets, event_tx, event_rx,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

// loads the best available icon from assets/ for the window titlebar/taskbar
// tries 64px first (looks nicest), then 32px, then the big one
fn load_window_icon() -> egui::IconData {
    for path in &[
        "assets/icon_64.png",
        "assets/icon_32.png",
        "assets/icon.png",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                return egui::IconData {
                    rgba: rgba.into_raw(),
                    width: w,
                    height: h,
                };
            }
        }
    }
    // fallback solid blue square if assets folder is missing
    let rgba: Vec<u8> = [0x4f, 0x8e, 0xf7, 0xff].repeat(32 * 32);
    egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}

// builds the color palette based on the user's theme setting
fn build_palette(cfg: &Config) -> Palette {
    let accent = parse_hex_color(&cfg.gui.accent_color);
    let is_dark = match cfg.gui.theme.as_str() {
        "light" => false,
        "system" => theme::system_is_dark(),
        _ => true, // "dark" or "custom", default to dark
    };
    if is_dark {
        Palette::dark(accent)
    } else {
        Palette::light(accent)
    }
}
