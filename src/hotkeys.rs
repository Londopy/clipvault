// hotkeys.rs
// listens for global keyboard shortcuts using rdev and sends events to the gui
// rdev gives us every keypress system-wide which is exactly what we need

use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use log::debug;
use rdev::{listen, Event, EventType, Key};

use crate::config::Config;
use crate::gui::AppEvent;

// represents a keyboard shortcut like ctrl+shift+v
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool, // cmd on mac
    pub key: Key,
}

impl KeyCombo {
    // turns a string like "ctrl+shift+v" into a KeyCombo
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.to_lowercase().split('+').collect();
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut meta = false;
        let mut key = None;
        for part in &parts {
            match *part {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                "meta" | "cmd" | "super" | "win" => meta = true,
                k => {
                    key = str_to_key(k);
                }
            }
        }
        key.map(|k| KeyCombo {
            ctrl,
            shift,
            alt,
            meta,
            key: k,
        })
    }

    // checks if the currently held keys + the just-pressed key match this combo
    pub fn matches(&self, held: &HashSet<Key>, just_pressed: &Key) -> bool {
        let ctrl_ok =
            self.ctrl == (held.contains(&Key::ControlLeft) || held.contains(&Key::ControlRight));
        let shift_ok =
            self.shift == (held.contains(&Key::ShiftLeft) || held.contains(&Key::ShiftRight));
        let alt_ok = self.alt == (held.contains(&Key::Alt) || held.contains(&Key::AltGr));
        let meta_ok =
            self.meta == (held.contains(&Key::MetaLeft) || held.contains(&Key::MetaRight));
        let key_ok = *just_pressed == self.key;
        ctrl_ok && shift_ok && alt_ok && meta_ok && key_ok
    }
}

// maps key name strings to rdev Key variants
fn str_to_key(s: &str) -> Option<Key> {
    match s {
        "a" => Some(Key::KeyA),
        "b" => Some(Key::KeyB),
        "c" => Some(Key::KeyC),
        "d" => Some(Key::KeyD),
        "e" => Some(Key::KeyE),
        "f" => Some(Key::KeyF),
        "g" => Some(Key::KeyG),
        "h" => Some(Key::KeyH),
        "i" => Some(Key::KeyI),
        "j" => Some(Key::KeyJ),
        "k" => Some(Key::KeyK),
        "l" => Some(Key::KeyL),
        "m" => Some(Key::KeyM),
        "n" => Some(Key::KeyN),
        "o" => Some(Key::KeyO),
        "p" => Some(Key::KeyP),
        "q" => Some(Key::KeyQ),
        "r" => Some(Key::KeyR),
        "s" => Some(Key::KeyS),
        "t" => Some(Key::KeyT),
        "u" => Some(Key::KeyU),
        "v" => Some(Key::KeyV),
        "w" => Some(Key::KeyW),
        "x" => Some(Key::KeyX),
        "y" => Some(Key::KeyY),
        "z" => Some(Key::KeyZ),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "0" => Some(Key::Num0),
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        "space" => Some(Key::Space),
        "return" | "enter" => Some(Key::Return),
        "escape" | "esc" => Some(Key::Escape),
        "tab" => Some(Key::Tab),
        "delete" | "del" => Some(Key::Delete),
        "backspace" => Some(Key::Backspace),
        _ => None,
    }
}

// keeps track of which keys are currently held and what combos to watch for
struct HotkeyState {
    held: HashSet<Key>,
    open_history: Option<KeyCombo>,
    open_snippets: Option<KeyCombo>,
    clear_clipboard: Option<KeyCombo>,
    paste_last: Option<KeyCombo>,
    incognito: Option<KeyCombo>,
    // the modifier part of ctrl+alt+1..9 for instant paste
    instant_mod: (bool, bool, bool), // (ctrl, alt, shift)
}

impl HotkeyState {
    fn from_config(cfg: &Config) -> Self {
        let hk = &cfg.hotkeys;
        let mod_parts: Vec<&str> = hk.instant_paste_mod.to_lowercase().split('+').collect();
        let im_ctrl = mod_parts.contains(&"ctrl") || mod_parts.contains(&"control");
        let im_alt = mod_parts.contains(&"alt");
        let im_shift = mod_parts.contains(&"shift");
        Self {
            held: HashSet::new(),
            open_history: KeyCombo::parse(&hk.open_history),
            open_snippets: KeyCombo::parse(&hk.open_snippets),
            clear_clipboard: KeyCombo::parse(&hk.clear_clipboard),
            paste_last: KeyCombo::parse(&hk.paste_last),
            incognito: KeyCombo::parse(&hk.incognito),
            instant_mod: (im_ctrl, im_alt, im_shift),
        }
    }

    // returns which slot to paste (1-9) if the instant-paste combo is active
    fn instant_paste_n(&self, key: &Key) -> Option<usize> {
        let (im_ctrl, im_alt, im_shift) = self.instant_mod;
        let ctrl_ok = im_ctrl
            == (self.held.contains(&Key::ControlLeft) || self.held.contains(&Key::ControlRight));
        let alt_ok = im_alt == (self.held.contains(&Key::Alt) || self.held.contains(&Key::AltGr));
        let shift_ok = im_shift
            == (self.held.contains(&Key::ShiftLeft) || self.held.contains(&Key::ShiftRight));
        if !(ctrl_ok && alt_ok && shift_ok) {
            return None;
        }
        match key {
            Key::Num1 => Some(1),
            Key::Num2 => Some(2),
            Key::Num3 => Some(3),
            Key::Num4 => Some(4),
            Key::Num5 => Some(5),
            Key::Num6 => Some(6),
            Key::Num7 => Some(7),
            Key::Num8 => Some(8),
            Key::Num9 => Some(9),
            _ => None,
        }
    }
}

pub fn run(config: Arc<Mutex<Config>>, tx: Sender<AppEvent>) -> Result<()> {
    let state = {
        let cfg = config.lock().unwrap();
        Arc::new(Mutex::new(HotkeyState::from_config(&cfg)))
    };

    listen(move |event: Event| {
        let mut st = state.lock().unwrap();
        match event.event_type {
            EventType::KeyPress(key) => {
                st.held.insert(key.clone());

                // check each combo and send the right event
                if st
                    .open_history
                    .as_ref()
                    .map_or(false, |c| c.matches(&st.held, &key))
                {
                    debug!("hotkey: open_history");
                    let _ = tx.send(AppEvent::OpenHistory);
                } else if st
                    .open_snippets
                    .as_ref()
                    .map_or(false, |c| c.matches(&st.held, &key))
                {
                    debug!("hotkey: open_snippets");
                    let _ = tx.send(AppEvent::OpenSnippets);
                } else if st
                    .clear_clipboard
                    .as_ref()
                    .map_or(false, |c| c.matches(&st.held, &key))
                {
                    debug!("hotkey: clear_clipboard");
                    let _ = tx.send(AppEvent::ClearClipboard);
                } else if st
                    .paste_last
                    .as_ref()
                    .map_or(false, |c| c.matches(&st.held, &key))
                {
                    debug!("hotkey: paste_last");
                    let _ = tx.send(AppEvent::PasteLast);
                } else if st
                    .incognito
                    .as_ref()
                    .map_or(false, |c| c.matches(&st.held, &key))
                {
                    debug!("hotkey: toggle_incognito");
                    let _ = tx.send(AppEvent::ToggleIncognito);
                } else if let Some(n) = st.instant_paste_n(&key) {
                    debug!("hotkey: instant_paste({n})");
                    let _ = tx.send(AppEvent::InstantPaste(n));
                }
            }
            EventType::KeyRelease(key) => {
                st.held.remove(&key);
            }
            _ => {}
        }
    })
    .map_err(|e| anyhow::anyhow!("rdev listen error: {:?}", e))?;

    Ok(())
}
