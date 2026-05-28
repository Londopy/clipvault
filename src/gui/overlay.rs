// gui/overlay.rs
// the popup that shows your clipboard history, pinned items, snippets, and settings
// opens when you hit the hotkey, closes when you press escape or paste something

use std::sync::{Arc, Mutex};

use egui::{Context, Key, Modifiers, RichText, ScrollArea, Ui};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::config::Config;
use crate::snippets::SnippetStore;
use crate::store::{ClipEntry, Store};
use crate::transforms::{apply as apply_transform, Transform};
use super::theme::Palette;
use super::preview::PreviewPane;

// which tab is selected in the overlay
#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    History,
    Pinned,
    Snippets,
    Settings,
}

impl Tab {
    // tab key cycles through the content tabs but skips settings
    pub fn next(&self) -> Tab {
        match self {
            Tab::History => Tab::Pinned,
            Tab::Pinned => Tab::Snippets,
            Tab::Snippets => Tab::History,
            Tab::Settings => Tab::History,
        }
    }
}

// state for the transform popup (uppercase, base64, etc)
pub struct TransformMenu {
    pub open: bool,
    pub entry_id: Option<String>,
    pub regex_pattern: String,
    pub regex_replace: String,
    pub result: Option<Result<String, String>>,
}

impl Default for TransformMenu {
    fn default() -> Self {
        Self {
            open: false,
            entry_id: None,
            regex_pattern: String::new(),
            regex_replace: String::new(),
            result: None,
        }
    }
}

// all the ui state for the overlay - search query, selected row, etc
pub struct Overlay {
    pub tab: Tab,
    pub search_query: String,
    pub selected_idx: usize,
    pub focus_search: bool,
    pub transform_menu: TransformMenu,
    matcher: SkimMatcherV2,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            tab: Tab::History,
            search_query: String::new(),
            selected_idx: 0,
            focus_search: false,
            transform_menu: TransformMenu::default(),
            matcher: SkimMatcherV2::default(),
        }
    }
}

// what the overlay wants to do after this frame
pub enum OverlayAction {
    None,
    PasteEntry(String), // user picked something to paste
    DeleteEntry(String), // user deleted an entry (by id)
    Close, // user pressed escape or clicked away
    SettingsChanged, // user toggled something in settings - parent should rebuild palette etc
}

impl Overlay {
    pub fn reset_for_open(&mut self, initial_tab: Tab) {
        self.tab = initial_tab;
        self.search_query = String::new();
        self.selected_idx = 0;
        self.focus_search = false;
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
        config: &Arc<Mutex<Config>>,
        palette: &Palette,
    ) -> OverlayAction {
        let mut action = OverlayAction::None;

        // Main overlay window
        egui::Window::new("ClipVault")
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .frame(egui::Frame::window(&ctx.style()))
            .show(ctx, |ui| {
                action = self.draw_content(ui, store, snippets, config, palette);
            });

        action
    }

    fn draw_content(
        &mut self,
        ui: &mut Ui,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
        config: &Arc<Mutex<Config>>,
        palette: &Palette,
    ) -> OverlayAction {
        let mut action = OverlayAction::None;

        // ── Header / tabs ─────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::History, "History"),
                (Tab::Pinned, "Pinned"),
                (Tab::Snippets, "Snippets"),
            ] {
                let selected = self.tab == tab;
                let text = RichText::new(label).color(if selected { palette.accent } else { palette.text_dim });
                if ui.selectable_label(selected, text).clicked() {
                    self.tab = tab;
                    self.selected_idx = 0;
                }
            }
            // settings gear sits on the far right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let selected = self.tab == Tab::Settings;
                let text = RichText::new("⚙").color(if selected { palette.accent } else { palette.text_dim });
                if ui.selectable_label(selected, text).on_hover_text("Settings").clicked() {
                    self.tab = if selected { Tab::History } else { Tab::Settings };
                }
            });
        });

        ui.separator();

        // ── Search bar ────────────────────────────────────────────────────────
        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut self.search_query)
                .hint_text("Search…")
                .desired_width(f32::INFINITY)
                .frame(true),
        );
        if self.focus_search {
            search_resp.request_focus();
            self.focus_search = false;
        }

        ui.add_space(4.0);

        // ── Item list ─────────────────────────────────────────────────────────
        let (max_items, show_ts, show_app) = {
            let cfg = config.lock().unwrap();
            (cfg.gui.max_visible_items, cfg.gui.show_timestamps, cfg.gui.show_source_app)
        };

        // render whichever tab is active
        let list_action = match self.tab {
            Tab::History => {
                let entries: Vec<ClipEntry> = {
                    let s = store.lock().unwrap();
                    s.history.iter()
                        .filter(|e| !e.is_pinned)
                        .cloned()
                        .collect()
                };
                self.draw_entry_list(ui, &entries, palette, max_items, show_ts, show_app, false)
            }
            Tab::Pinned => {
                let entries: Vec<ClipEntry> = {
                    let s = store.lock().unwrap();
                    s.history.iter()
                        .filter(|e| e.is_pinned)
                        .cloned()
                        .collect()
                };
                self.draw_entry_list(ui, &entries, palette, max_items, show_ts, show_app, false)
            }
            Tab::Snippets => {
                self.draw_snippets_list(ui, snippets, palette, max_items)
            }
            Tab::Settings => {
                self.draw_settings(ui, config, store, palette)
            }
        };

        if !matches!(list_action, OverlayAction::None) {
            action = list_action;
        }

        // ── Preview pane for selected entry ───────────────────────────────────
        if self.tab != Tab::Snippets && self.tab != Tab::Settings {
            let entries: Vec<ClipEntry> = {
                let s = store.lock().unwrap();
                s.history.iter()
                    .filter(|e| if self.tab == Tab::History { !e.is_pinned } else { e.is_pinned })
                    .cloned()
                    .collect()
            };
            let filtered = self.filtered_entries(&entries);
            if let Some(entry) = filtered.get(self.selected_idx) {
                PreviewPane::show(ui, entry, palette);
            }
        }

        // ── Transform menu (not shown on settings tab) ────────────────────────
        if self.transform_menu.open && self.tab != Tab::Settings {
            let tm_action = self.draw_transform_menu(ui, store, palette);
            if !matches!(tm_action, OverlayAction::None) {
                action = tm_action;
            }
        }

        // ── Keyboard navigation ───────────────────────────────────────────────
        let kb_action = self.handle_keyboard(ui, store, snippets);
        if !matches!(kb_action, OverlayAction::None) {
            action = kb_action;
        }

        action
    }

    fn draw_entry_list(
        &mut self,
        ui: &mut Ui,
        entries: &[ClipEntry],
        palette: &Palette,
        max_items: usize,
        show_ts: bool,
        show_app: bool,
        _is_pinned: bool,
    ) -> OverlayAction {
        let mut action = OverlayAction::None;
        let filtered = self.filtered_entries(entries);

        ScrollArea::vertical()
            .max_height(max_items as f32 * 44.0)
            .show(ui, |ui| {
                for (idx, entry) in filtered.iter().enumerate().take(max_items) {
                    let selected = idx == self.selected_idx;
                    let bg_color = if selected { palette.bg_highlight } else { palette.bg_secondary };

                    let item_resp = egui::Frame::none()
                        .fill(bg_color)
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                // show a number for the first 9 so you know what ctrl+alt+n does
                                if idx < 9 {
                                    ui.label(RichText::new(format!("{}.", idx + 1))
                                        .color(palette.text_dim).small());
                                }
                                // Pin indicator
                                if entry.is_pinned {
                                    ui.label(RichText::new("📌").small());
                                }
                                // Preview text
                                ui.label(
                                    RichText::new(entry.preview(80))
                                        .color(if selected { palette.text } else { palette.text_dim })
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if show_ts {
                                        let ts = entry.timestamp.format("%H:%M").to_string();
                                        ui.label(RichText::new(ts).color(palette.text_dim).small());
                                    }
                                    if show_app {
                                        if let Some(ref app) = entry.source_app {
                                            ui.label(RichText::new(app.as_str()).color(palette.text_dim).small());
                                        }
                                    }
                                });
                            });
                        });

                    if item_resp.response.clicked() {
                        self.selected_idx = idx;
                    }

                    // double click pastes immediately
                    if item_resp.response.double_clicked() {
                        action = OverlayAction::PasteEntry(entry.data.clone());
                    }

                    // right click for more options
                    item_resp.response.context_menu(|ui| {
                        if ui.button("Paste").clicked() {
                            action = OverlayAction::PasteEntry(entry.data.clone());
                            ui.close_menu();
                        }
                        if ui.button("Delete").clicked() {
                            action = OverlayAction::DeleteEntry(entry.id.clone());
                            ui.close_menu();
                        }
                        if ui.button("Transforms…").clicked() {
                            self.transform_menu.open = true;
                            self.transform_menu.entry_id = Some(entry.id.clone());
                            ui.close_menu();
                        }
                    });

                    ui.add_space(2.0);
                }
            });

        action
    }

    fn draw_snippets_list(
        &mut self,
        ui: &mut Ui,
        snippets: &Arc<Mutex<SnippetStore>>,
        palette: &Palette,
        max_items: usize,
    ) -> OverlayAction {
        let mut action = OverlayAction::None;
        let sn = snippets.lock().unwrap();
        let query = self.search_query.to_lowercase();

        let filtered: Vec<_> = sn.snippets.iter()
            .filter(|s| {
                query.is_empty()
                    || s.name.to_lowercase().contains(&query)
                    || s.content.to_lowercase().contains(&query)
                    || s.shortcode.as_deref().unwrap_or("").contains(&query)
            })
            .collect();

        ScrollArea::vertical()
            .max_height(max_items as f32 * 44.0)
            .show(ui, |ui| {
                for (idx, sn) in filtered.iter().enumerate().take(max_items) {
                    let selected = idx == self.selected_idx;
                    let bg_color = if selected { palette.bg_highlight } else { palette.bg_secondary };

                    let item_resp = egui::Frame::none()
                        .fill(bg_color)
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&sn.name).color(palette.text));
                                if let Some(ref sc) = sn.shortcode {
                                    ui.label(RichText::new(format!(";;{sc}")).color(palette.accent).small());
                                }
                                if let Some(ref cat) = sn.category {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(cat.as_str()).color(palette.text_dim).small());
                                    });
                                }
                            });
                        });

                    if item_resp.response.clicked() {
                        self.selected_idx = idx;
                    }
                    if item_resp.response.double_clicked() {
                        action = OverlayAction::PasteEntry(sn.expanded_content());
                    }
                    ui.add_space(2.0);
                }
            });

        action
    }

    fn draw_settings(
        &mut self,
        ui: &mut Ui,
        config: &Arc<Mutex<Config>>,
        store: &Arc<Mutex<Store>>,
        palette: &Palette,
    ) -> OverlayAction {
        let mut changed = false;

        ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            ui.add_space(6.0);

            // ── General ──────────────────────────────────────────────────────
            ui.label(RichText::new("General").color(palette.accent).strong());
            ui.separator();
            ui.add_space(4.0);

            {
                let mut cfg = config.lock().unwrap();

                // startup toggle - the main one the user asked for
                let prev_auto_start = cfg.general.auto_start;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.general.auto_start, "");
                    ui.label(RichText::new("Start with system").color(palette.text));
                });
                ui.label(RichText::new("Launch ClipVault automatically when you log in.")
                    .color(palette.text_dim).small());
                if cfg.general.auto_start != prev_auto_start {
                    // actually register/unregister with the OS right now
                    if cfg.general.auto_start {
                        crate::platform::register_startup();
                    } else {
                        crate::platform::unregister_startup();
                    }
                    changed = true;
                }

                ui.add_space(8.0);

                let prev = cfg.general.persist_history;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.general.persist_history, "");
                    ui.label(RichText::new("Save history between sessions").color(palette.text));
                });
                if cfg.general.persist_history != prev { changed = true; }

                ui.add_space(4.0);

                let prev = cfg.general.deduplicate;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.general.deduplicate, "");
                    ui.label(RichText::new("Skip duplicate entries").color(palette.text));
                });
                if cfg.general.deduplicate != prev { changed = true; }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("History limit:").color(palette.text));
                    ui.add(egui::DragValue::new(&mut cfg.general.history_limit)
                        .clamp_range(10..=10_000usize)
                        .speed(1.0));
                });
                ui.label(RichText::new("Max number of items to keep.")
                    .color(palette.text_dim).small());
            }

            ui.add_space(12.0);

            // ── Display ───────────────────────────────────────────────────────
            ui.label(RichText::new("Display").color(palette.accent).strong());
            ui.separator();
            ui.add_space(4.0);

            {
                let mut cfg = config.lock().unwrap();

                let prev = cfg.gui.show_timestamps;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.gui.show_timestamps, "");
                    ui.label(RichText::new("Show timestamps").color(palette.text));
                });
                if cfg.gui.show_timestamps != prev { changed = true; }

                ui.add_space(4.0);

                let prev = cfg.gui.show_source_app;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.gui.show_source_app, "");
                    ui.label(RichText::new("Show source app").color(palette.text));
                });
                if cfg.gui.show_source_app != prev { changed = true; }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Theme:").color(palette.text));
                    egui::ComboBox::from_id_source("theme_combo")
                        .selected_text(cfg.gui.theme.clone())
                        .show_ui(ui, |ui| {
                            for t in ["dark", "light", "system"] {
                                if ui.selectable_label(cfg.gui.theme == t, t).clicked() {
                                    cfg.gui.theme = t.to_string();
                                    changed = true;
                                }
                            }
                        });
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Overlay position:").color(palette.text));
                    egui::ComboBox::from_id_source("pos_combo")
                        .selected_text(cfg.gui.position.clone())
                        .show_ui(ui, |ui| {
                            for p in ["cursor", "center", "top-right", "top-left"] {
                                if ui.selectable_label(cfg.gui.position == p, p).clicked() {
                                    cfg.gui.position = p.to_string();
                                    changed = true;
                                }
                            }
                        });
                });
            }

            ui.add_space(12.0);

            // ── Privacy ───────────────────────────────────────────────────────
            ui.label(RichText::new("Privacy").color(palette.accent).strong());
            ui.separator();
            ui.add_space(4.0);

            {
                let mut cfg = config.lock().unwrap();

                let prev = cfg.security.mask_passwords;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.security.mask_passwords, "");
                    ui.label(RichText::new("Mask passwords").color(palette.text));
                });
                ui.label(RichText::new("Hides entries that look like passwords.")
                    .color(palette.text_dim).small());
                if cfg.security.mask_passwords != prev { changed = true; }

                ui.add_space(4.0);

                let prev = cfg.security.encrypt_history;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.security.encrypt_history, "");
                    ui.label(RichText::new("Encrypt history file").color(palette.text));
                });
                ui.label(RichText::new("AES-256-GCM. Takes effect on next save.")
                    .color(palette.text_dim).small());
                if cfg.security.encrypt_history != prev { changed = true; }
            }

            ui.add_space(8.0);

            // danger zone: clear history button
            let btn = egui::Button::new(
                RichText::new("Clear All History").color(egui::Color32::WHITE)
            ).fill(palette.danger);
            if ui.add(btn).clicked() {
                store.lock().unwrap().clear(false);
            }
            ui.label(RichText::new("Removes everything including pinned items.")
                .color(palette.text_dim).small());

            ui.add_space(8.0);

            // save config if anything changed
            if changed {
                let _ = config.lock().unwrap().save();
            }
        });

        if changed { OverlayAction::SettingsChanged } else { OverlayAction::None }
    }

    fn draw_transform_menu(
        &mut self,
        ui: &mut Ui,
        store: &Arc<Mutex<Store>>,
        palette: &Palette,
    ) -> OverlayAction {
        let mut action = OverlayAction::None;

        egui::Window::new("Transforms")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                // look up the entry we're transforming
                let entry_data: Option<String> = self.transform_menu.entry_id.as_ref().and_then(|id| {
                    store.lock().unwrap().history.iter().find(|e| &e.id == id).map(|e| e.data.clone())
                });

                if let Some(d