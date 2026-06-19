// config.rs
// loads/saves the config.toml and holds all the settings structs
// if the file doesnt exist it just writes the defaults, pretty convenient

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// top level config - each section maps to a [section] in the toml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub hotkeys: HotkeyConfig,

    #[serde(default)]
    pub gui: GuiConfig,

    #[serde(default)]
    pub notifications: NotificationConfig,

    #[serde(default)]
    pub discord: DiscordConfig,

    #[serde(default)]
    pub updater: UpdaterConfig,

    #[serde(default)]
    pub security: SecurityConfig,
}

// general stuff - history size, whether to save to disk, etc
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub history_limit: usize,
    pub persist_history: bool,
    pub deduplicate: bool,
    pub auto_start: bool,
    pub pause_on_lock: bool,
    pub auto_clear_hours: u64,
}

// all the keyboard shortcuts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub open_history: String,
    pub open_snippets: String,
    pub clear_clipboard: String,
    pub paste_last: String,
    pub instant_paste_mod: String,
    pub incognito: String,
}

// window appearance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub position: String,
    pub theme: String,
    pub accent_color: String,
    pub max_visible_items: usize,
    pub show_timestamps: bool,
    pub show_source_app: bool,
    pub animate_open: bool,
}

// notification settings per event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub on_copy: bool,
    pub on_paste: bool,
    pub on_update: bool,
    pub duration_ms: u64,
    pub sound: bool,
}

// discord rich presence stuff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub rich_presence: bool,
    pub application_id: String,
}

// auto updater settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    pub enabled: bool,
    pub channel: String,
    pub check_on_startup: bool,
    pub auto_install: bool,
}

// privacy and security options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub excluded_apps: Vec<String>,
    pub mask_passwords: bool,
    pub encrypt_history: bool,
    pub incognito_hotkey: String,
    pub auto_clear_on_lock: bool,
}

// defaults for everything - these match config.default.toml

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            history_limit: 200,
            persist_history: true,
            deduplicate: true,
            auto_start: true,
            pause_on_lock: false,
            auto_clear_hours: 0,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            open_history: "ctrl+shift+v".into(),
            open_snippets: "ctrl+shift+c".into(),
            clear_clipboard: "ctrl+shift+x".into(),
            paste_last: "ctrl+shift+p".into(),
            instant_paste_mod: "ctrl+alt".into(),
            incognito: "ctrl+shift+i".into(),
        }
    }
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            position: "cursor".into(),
            theme: "dark".into(),
            accent_color: "#4f8ef7".into(),
            max_visible_items: 12,
            show_timestamps: true,
            show_source_app: true,
            animate_open: true,
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_copy: true,
            on_paste: false,
            on_update: true,
            duration_ms: 2500,
            sound: false,
        }
    }
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            rich_presence: true,
            application_id: "REPLACE_WITH_YOUR_APP_ID".into(),
        }
    }
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: "stable".into(),
            check_on_startup: true,
            auto_install: false,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            excluded_apps: vec!["1Password".into(), "KeePassXC".into(), "Bitwarden".into()],
            mask_passwords: true,
            encrypt_history: false,
            incognito_hotkey: "ctrl+shift+i".into(),
            auto_clear_on_lock: false,
        }
    }
}

impl Config {
    // where the config file lives on each platform
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipvault")
            .join("config.toml")
    }

    // tries to load from disk, if the file isnt there it writes the defaults and returns those
    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    // write current config back to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(&path, toml_str)?;
        Ok(())
    }

    // basic sanity checks - dont want someone setting history_limit to 0 and breaking everything
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            (1..=10_000).contains(&self.general.history_limit),
            "history_limit must be between 1 and 10000"
        );
        anyhow::ensure!(
            (1..=100).contains(&self.gui.max_visible_items),
            "max_visible_items must be between 1 and 100"
        );
        anyhow::ensure!(
            matches!(self.updater.channel.as_str(), "stable" | "beta"),
            "updater.channel must be 'stable' or 'beta'"
        );
        anyhow::ensure!(
            matches!(
                self.gui.position.as_str(),
                "cursor" | "center" | "top-right" | "top-left"
            ),
            "gui.position must be one of: cursor, center, top-right, top-left"
        );
        Ok(())
    }

    // opens the folder where config.toml lives so the user can find it easily
    pub fn open_config_dir() {
        let dir = Self::path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&dir).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
        }
    }
}
