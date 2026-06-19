// gui/overlay.rs
// the popup that shows your clipboard history, pinned items, snippets, and settings
// opens when you hit the hotkey, closes when you press escape or paste something

use std::sync::{Arc, Mutex};

use egui::{Context, Key, Modifiers, RichText, ScrollArea, Ui};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use super::preview::PreviewPane;
use super::theme::Palette;
use crate::config::Config;
use crate::snippets::SnippetStore;
use crate::store::{ClipEntry, Store};
use crate::transforms::{apply as apply_transform, Transform};

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
    PasteEntry(String),  // user picked something to paste
    DeleteEntry(String), // user deleted an entry (by id)
    Close,               // user pressed escape or clicked away
    SettingsChanged,     // user toggled something in settings - parent should rebuild palette etc
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
                let text = RichText::new(label).color(if selected {
                    palette.accent
                } else {
                    palette.text_dim
                });
                if ui.selectable_label(selected, text).clicked() {
                    self.tab = tab;
                    self.selected_idx = 0;
                }
            }
            // settings gear sits on the far right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let selected = self.tab == Tab::Settings;
                let text = RichText::new("⚙").color(if selected {
                    palette.accent
                } else {
                    palette.text_dim
                });
                if ui
                    .selectable_label(selected, text)
                    .on_hover_text("Settings")
                    .clicked()
                {
                    self.tab = if selected {
                        Tab::History
                    } else {
                        Tab::Settings
                    };
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
            (
                cfg.gui.max_visible_items,
                cfg.gui.show_timestamps,
                cfg.gui.show_source_app,
            )
        };

        // render whichever tab is active
        let list_action = match self.tab {
            Tab::History => {
                let entries: Vec<ClipEntry> = {
                    let s = store.lock().unwrap();
                    s.history.iter().filter(|e| !e.is_pinned).cloned().collect()
                };
                self.draw_entry_list(ui, &entries, palette, max_items, show_ts, show_app, false)
            }
            Tab::Pinned => {
                let entries: Vec<ClipEntry> = {
                    let s = store.lock().unwrap();
                    s.history.iter().filter(|e| e.is_pinned).cloned().collect()
                };
                self.draw_entry_list(ui, &entries, palette, max_items, show_ts, show_app, false)
            }
            Tab::Snippets => self.draw_snippets_list(ui, snippets, palette, max_items),
            Tab::Settings => self.draw_settings(ui, config, store, palette),
        };

        if !matches!(list_action, OverlayAction::None) {
            action = list_action;
        }

        // ── Preview pane for selected entry ───────────────────────────────────
        if self.tab != Tab::Snippets && self.tab != Tab::Settings {
            let entries: Vec<ClipEntry> = {
                let s = store.lock().unwrap();
                s.history
                    .iter()
                    .filter(|e| {
                        if self.tab == Tab::History {
                            !e.is_pinned
                        } else {
                            e.is_pinned
                        }
                    })
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
                    let bg_color = if selected {
                        palette.bg_highlight
                    } else {
                        palette.bg_secondary
                    };

                    let item_resp = egui::Frame::none()
                        .fill(bg_color)
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                // show a number for the first 9 so you know what ctrl+alt+n does
                                if idx < 9 {
                                    ui.label(
                                        RichText::new(format!("{}.", idx + 1))
                                            .color(palette.text_dim)
                                            .small(),
                                    );
                                }
                                // Pin indicator
                                if entry.is_pinned {
                                    ui.label(RichText::new("📌").small());
                                }
                                // Preview text
                                ui.label(RichText::new(entry.preview(80)).color(if selected {
                                    palette.text
                                } else {
                                    palette.text_dim
                                }));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if show_ts {
                                            let ts = entry.timestamp.format("%H:%M").to_string();
                                            ui.label(
                                                RichText::new(ts).color(palette.text_dim).small(),
                                            );
                                        }
                                        if show_app {
                                            if let Some(ref app) = entry.source_app {
                                                ui.label(
                                                    RichText::new(app.as_str())
                                                        .color(palette.text_dim)
                                                        .small(),
                                                );
                                            }
                                        }
                                    },
                                );
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

        let filtered: Vec<_> = sn
            .snippets
            .iter()
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
                    let bg_color = if selected {
                        palette.bg_highlight
                    } else {
                        palette.bg_secondary
                    };

                    let item_resp = egui::Frame::none()
                        .fill(bg_color)
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&sn.name).color(palette.text));
                                if let Some(ref sc) = sn.shortcode {
                                    ui.label(
                                        RichText::new(format!(";;{sc}"))
                                            .color(palette.accent)
                                            .small(),
                                    );
                                }
                                if let Some(ref cat) = sn.category {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(cat.as_str())
                                                    .color(palette.text_dim)
                                                    .small(),
                                            );
                                        },
                                    );
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
                ui.label(
                    RichText::new("Launch ClipVault automatically when you log in.")
                        .color(palette.text_dim)
                        .small(),
                );
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
                if cfg.general.persist_history != prev {
                    changed = true;
                }

                ui.add_space(4.0);

                let prev = cfg.general.deduplicate;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.general.deduplicate, "");
                    ui.label(RichText::new("Skip duplicate entries").color(palette.text));
                });
                if cfg.general.deduplicate != prev {
                    changed = true;
                }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("History limit:").color(palette.text));
                    ui.add(
                        egui::DragValue::new(&mut cfg.general.history_limit)
                            .range(10..=10_000usize)
                            .speed(1.0),
                    );
                });
                ui.label(
                    RichText::new("Max number of items to keep.")
                        .color(palette.text_dim)
                        .small(),
                );
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
                if cfg.gui.show_timestamps != prev {
                    changed = true;
                }

                ui.add_space(4.0);

                let prev = cfg.gui.show_source_app;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.gui.show_source_app, "");
                    ui.label(RichText::new("Show source app").color(palette.text));
                });
                if cfg.gui.show_source_app != prev {
                    changed = true;
                }

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
                ui.label(
                    RichText::new("Hides entries that look like passwords.")
                        .color(palette.text_dim)
                        .small(),
                );
                if cfg.security.mask_passwords != prev {
                    changed = true;
                }

                ui.add_space(4.0);

                let prev = cfg.security.encrypt_history;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut cfg.security.encrypt_history, "");
                    ui.label(RichText::new("Encrypt history file").color(palette.text));
                });
                ui.label(
                    RichText::new("AES-256-GCM. Takes effect on next save.")
                        .color(palette.text_dim)
                        .small(),
                );
                if cfg.security.encrypt_history != prev {
                    changed = true;
                }
            }

            ui.add_space(8.0);

            // danger zone: clear history button
            let btn =
                egui::Button::new(RichText::new("Clear All History").color(egui::Color32::WHITE))
                    .fill(palette.danger);
            if ui.add(btn).clicked() {
                store.lock().unwrap().clear(false);
            }
            ui.label(
                RichText::new("Removes everything including pinned items.")
                    .color(palette.text_dim)
                    .small(),
            );

            ui.add_space(8.0);

            // save config if anything changed
            if changed {
                let _ = config.lock().unwrap().save();
            }
        });

        if changed {
            OverlayAction::SettingsChanged
        } else {
            OverlayAction::None
        }
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
                let entry_data: Option<String> =
                    self.transform_menu.entry_id.as_ref().and_then(|id| {
                        store
                            .lock()
                            .unwrap()
                            .history
                            .iter()
                            .find(|e| &e.id == id)
                            .map(|e| e.data.clone())
                    });

                if let Some(data) = entry_data {
                    // show buttons for each transform
                    for t in Transform::all_simple() {
                        if ui.button(t.label()).clicked() {
                            match apply_transform(&data, &t) {
                                Ok(result) => {
                                    self.transform_menu.result = Some(Ok(result.clone()));
                                    action = OverlayAction::PasteEntry(result);
                                    self.transform_menu.open = false;
                                }
                                Err(e) => {
                                    self.transform_menu.result = Some(Err(e.to_string()));
                                }
                            }
                        }
                    }

                    ui.separator();
                    ui.label(RichText::new("Regex Replace").color(palette.text));
                    ui.horizontal(|ui| {
                        ui.label("Pattern:");
                        ui.text_edit_singleline(&mut self.transform_menu.regex_pattern);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Replace:");
                        ui.text_edit_singleline(&mut self.transform_menu.regex_replace);
                    });
                    if ui.button("Apply Regex").clicked() {
                        let t = Transform::RegexReplace {
                            pattern: self.transform_menu.regex_pattern.clone(),
                            replacement: self.transform_menu.regex_replace.clone(),
                        };
                        match apply_transform(&data, &t) {
                            Ok(result) => {
                                action = OverlayAction::PasteEntry(result);
                                self.transform_menu.open = false;
                            }
                            Err(e) => {
                                self.transform_menu.result = Some(Err(e.to_string()));
                            }
                        }
                    }
                }

                // show errors in red if a transform fails
                if let Some(Err(ref err)) = self.transform_menu.result {
                    ui.label(RichText::new(format!("Error: {err}")).color(palette.danger));
                }

                ui.separator();
                if ui.button("Cancel").clicked() {
                    self.transform_menu.open = false;
                    self.transform_menu.result = None;
                }
            });

        action
    }

    fn handle_keyboard(
        &mut self,
        ui: &mut Ui,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
    ) -> OverlayAction {
        let ctx = ui.ctx();

        // escape closes the overlay
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            return OverlayAction::Close;
        }

        // tab cycles between history/pinned/snippets tabs
        if ctx.input(|i| i.key_pressed(Key::Tab) && i.modifiers == Modifiers::NONE) {
            self.tab = self.tab.next();
            self.selected_idx = 0;
        }

        // arrow keys move the selection up and down
        let total = self.current_count(store, snippets);
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            if self.selected_idx + 1 < total {
                self.selected_idx += 1;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            self.selected_idx = self.selected_idx.saturating_sub(1);
        }

        // enter pastes the selected item
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(data) = self.selected_data(store, snippets) {
                return OverlayAction::PasteEntry(data);
            }
        }

        // delete removes the selected item
        if ctx.input(|i| i.key_pressed(Key::Delete)) {
            if let Some(id) = self.selected_id(store) {
                return OverlayAction::DeleteEntry(id);
            }
        }

        // / jumps focus to the search box
        if ctx.input(|i| i.key_pressed(Key::Slash)) {
            self.focus_search = true;
        }

        // p pins or unpins the selected item
        if ctx.input(|i| i.key_pressed(Key::P) && i.modifiers == Modifiers::NONE) {
            if let Some(id) = self.selected_id(store) {
                store.lock().unwrap().toggle_pin(&id);
            }
        }

        // e opens the transforms menu for the selected item
        if ctx.input(|i| i.key_pressed(Key::E) && i.modifiers == Modifiers::NONE) {
            if let Some(id) = self.selected_id(store) {
                self.transform_menu.open = true;
                self.transform_menu.entry_id = Some(id);
            }
        }

        // 1-9 instant-paste by position
        for (key, idx) in [
            (Key::Num1, 0),
            (Key::Num2, 1),
            (Key::Num3, 2),
            (Key::Num4, 3),
            (Key::Num5, 4),
            (Key::Num6, 5),
            (Key::Num7, 6),
            (Key::Num8, 7),
            (Key::Num9, 8),
        ] {
            if ctx.input(|i| i.key_pressed(key) && i.modifiers == Modifiers::NONE) {
                let entries = self.all_current_entries(store, snippets);
                if let Some(data) = entries.get(idx) {
                    return OverlayAction::PasteEntry(data.clone());
                }
            }
        }

        OverlayAction::None
    }

    // returns entries matching the current search query, sorted by fuzzy score
    fn filtered_entries<'a>(&self, entries: &'a [ClipEntry]) -> Vec<&'a ClipEntry> {
        let q = self.search_query.trim();
        if q.is_empty() {
            return entries.iter().collect();
        }
        let mut scored: Vec<(i64, &ClipEntry)> = entries
            .iter()
            .filter_map(|e| self.matcher.fuzzy_match(&e.data, q).map(|score| (score, e)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, e)| e).collect()
    }

    fn current_count(
        &self,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
    ) -> usize {
        match self.tab {
            Tab::History => store
                .lock()
                .unwrap()
                .history
                .iter()
                .filter(|e| !e.is_pinned)
                .count(),
            Tab::Pinned => store
                .lock()
                .unwrap()
                .history
                .iter()
                .filter(|e| e.is_pinned)
                .count(),
            Tab::Snippets => snippets.lock().unwrap().snippets.len(),
            Tab::Settings => 0,
        }
    }

    fn selected_id(&self, store: &Arc<Mutex<Store>>) -> Option<String> {
        let s = store.lock().unwrap();
        let entries: Vec<&ClipEntry> = match self.tab {
            Tab::History => s.history.iter().filter(|e| !e.is_pinned).collect(),
            Tab::Pinned => s.history.iter().filter(|e| e.is_pinned).collect(),
            Tab::Snippets | Tab::Settings => return None,
        };
        entries.get(self.selected_idx).map(|e| e.id.clone())
    }

    fn selected_data(
        &self,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
    ) -> Option<String> {
        match self.tab {
            Tab::Snippets => snippets
                .lock()
                .unwrap()
                .snippets
                .get(self.selected_idx)
                .map(|s| s.expanded_content()),
            Tab::Settings => None,
            _ => {
                let s = store.lock().unwrap();
                let entries: Vec<&ClipEntry> = match self.tab {
                    Tab::History => s.history.iter().filter(|e| !e.is_pinned).collect(),
                    Tab::Pinned => s.history.iter().filter(|e| e.is_pinned).collect(),
                    _ => unreachable!(),
                };
                entries.get(self.selected_idx).map(|e| e.data.clone())
            }
        }
    }

    fn all_current_entries(
        &self,
        store: &Arc<Mutex<Store>>,
        snippets: &Arc<Mutex<SnippetStore>>,
    ) -> Vec<String> {
        match self.tab {
            Tab::Snippets => snippets
                .lock()
                .unwrap()
                .snippets
                .iter()
                .map(|s| s.expanded_content())
                .collect(),
            Tab::Settings => Vec::new(),
            _ => {
                let s = store.lock().unwrap();
                s.history
                    .iter()
                    .filter(|e| {
                        if self.tab == Tab::History {
                            !e.is_pinned
                        } else {
                            e.is_pinned
                        }
                    })
                    .map(|e| e.data.clone())
                    .collect()
            }
        }
    }
}
