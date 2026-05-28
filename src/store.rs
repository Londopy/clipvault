// store.rs
// the main clipboard history - basically a ring buffer that saves to disk
// also handles pinned items and the optional encryption stuff

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::rand_core::RngCore;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;

// the three types of things you can copy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Text,
    Image,
    FilePath,
}

// one entry in the clipboard history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id:           String,
    pub content_type: ContentType,
    // for text: the actual text. for images: base64 webp thumbnail. for files: the path
    pub data:         String,
    // full res image path if we have it (images only)
    pub image_path:   Option<PathBuf>,
    pub source_app:   Option<String>,
    pub timestamp:    DateTime<Utc>,
    pub char_count:   Option<usize>,
    pub is_pinned:    bool,
    pub tags:         Vec<String>,
}

impl ClipEntry {
    pub fn new_text(text: String, source_app: Option<String>) -> Self {
        let char_count = Some(text.chars().count());
        Self {
            id:           Uuid::new_v4().to_string(),
            content_type: ContentType::Text,
            data:         text,
            image_path:   None,
            source_app,
            timestamp:    Utc::now(),
            char_count,
            is_pinned:    false,
            tags:         Vec::new(),
        }
    }

    pub fn new_image(thumbnail_b64: String, image_path: Option<PathBuf>, source_app: Option<String>) -> Self {
        Self {
            id:           Uuid::new_v4().to_string(),
            content_type: ContentType::Image,
            data:         thumbnail_b64,
            image_path,
            source_app,
            timestamp:    Utc::now(),
            char_count:   None,
            is_pinned:    false,
            tags:         Vec::new(),
        }
    }

    pub fn new_filepath(path: String, source_app: Option<String>) -> Self {
        Self {
            id:           Uuid::new_v4().to_string(),
            content_type: ContentType::FilePath,
            data:         path,
            image_path:   None,
            source_app,
            timestamp:    Utc::now(),
            char_count:   None,
            is_pinned:    false,
            tags:         Vec::new(),
        }
    }

    // short version of the content for showing in the list
    pub fn preview(&self, max_chars: usize) -> String {
        match self.content_type {
            ContentType::Text => {
                let s: String = self.data.chars().take(max_chars).collect();
                if self.data.chars().count() > max_chars {
                    format!("{}…", s)
                } else {
                    s
                }
            }
            ContentType::Image    => "📷 Image".into(),
            ContentType::FilePath => format!("📄 {}", self.data),
        }
    }
}

// this is what gets written to history.json
#[derive(Serialize, Deserialize)]
struct HistoryFile {
    version:  u32,
    entries:  Vec<ClipEntry>,
}

pub struct Store {
    // newest items at the front
    pub history:   VecDeque<ClipEntry>,
    pub max_size:  usize,
    pub incognito: bool,
    pub paused:    bool,
    encrypt:       bool,
    enc_key:       Option<[u8; 32]>,
}

impl Store {
    pub fn new(cfg: &Config) -> Self {
        Self {
            history:   VecDeque::new(),
            max_size:  cfg.general.history_limit,
            incognito: false,
            paused:    false,
            encrypt:   cfg.security.encrypt_history,
            enc_key:   None,
        }
    }

    // load history from disk - handles encryption if its turned on
    pub fn load(cfg: &Config) -> Result<Self> {
        let mut store = Self::new(cfg);
        if !cfg.general.persist_history {
            return Ok(store);
        }
        let path = Self::history_path();
        if !path.exists() {
            return Ok(store);
        }
        let raw = fs::read(&path)?;
        let json_bytes = if cfg.security.encrypt_history {
            store.init_encryption()?;
            store.decrypt_bytes(&raw)?
        } else {
            raw
        };
        let file: HistoryFile = serde_json::from_slice(&json_bytes)
            .context("parsing history file")?;
        store.history = file.entries.into_iter().collect();
        // trim down in case the history_limit changed since last run
        while store.history.len() > store.max_size {
            store.history.pop_back();
        }
        Ok(store)
    }

    // save to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = HistoryFile {
            version: 1,
            entries: self.history.iter().cloned().collect(),
        };
        let json = serde_json::to_vec_pretty(&file)?;
        let bytes = if self.encrypt && self.enc_key.is_some() {
            self.encrypt_bytes(&json)?
        } else {
            json
        };
        fs::write(&path, bytes)?;
        Ok(())
    }

    pub fn history_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipvault")
            .join("history.json")
    }

    // add a new item - returns false if dedup kicked in and skipped it
    pub fn push(&mut self, entry: ClipEntry, deduplicate: bool) -> bool {
        if self.incognito || self.paused {
            return false;
        }
        // skip if its the same as the last thing we copied
        if deduplicate {
            if let Some(front) = self.history.front() {
                if front.data == entry.data && front.content_type == entry.content_type {
                    return false;
                }
            }
        }
        self.history.push_front(entry);
        // if we're over the limit, drop the oldest non-pinned item
        if self.history.len() > self.max_size {
            if let Some(idx) = self.history.iter().rposition(|e| !e.is_pinned) {
                self.history.remove(idx);
            }
        }
        true
    }

    pub fn remove(&mut self, id: &str) {
        self.history.retain(|e| e.id != id);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        if let Some(e) = self.history.iter_mut().find(|e| e.id == id) {
            e.is_pinned = !e.is_pinned;
        }
    }

    // add or remove a tag from an entry
    pub fn tag(&mut self, id: &str, tag: &str) {
        if let Some(e) = self.history.iter_mut().find(|e| e.id == id) {
            if e.tags.contains(&tag.to_string()) {
                e.tags.retain(|t| t != tag);
            } else {
                e.tags.push(tag.to_string());
            }
        }
    }

    // clear history, optionally keeping pinned items
    pub fn clear(&mut self, keep_pinned: bool) {
        if keep_pinned {
            self.history.retain(|e| e.is_pinned);
        } else {
            self.history.clear();
        }
    }

    pub fn pinned(&self) -> Vec<&ClipEntry> {
        self.history.iter().filter(|e| e.is_pinned).collect()
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    // encryption stuff below - aes-256-gcm with a random key stored next to the history file
    // not ideal (key next to the data) but good enough for now, TODO: use OS keychain

    fn ensure_key(&mut self) -> Result<[u8; 32]> {
        if let Some(k) = self.enc_key {
            return Ok(k);
        }
        let key_path = Self::history_path().with_extension("key");
        let key: [u8; 32] = if key_path.exists() {
            let raw = fs::read(&key_path)?;
            raw.try_into().map_err(|_| anyhow::anyhow!("invalid key length"))?
        } else {
            // generate a new random key and save it
            let mut k = [0u8; 32];
            OsRng.fill_bytes(&mut k);
            fs::write(&key_path, &k)?;
            k
        };
        self.enc_key = Some(key);
        Ok(key)
    }

    fn encrypt_bytes(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key_bytes = self.enc_key.ok_or_else(|| anyhow::anyhow!("no encryption key"))?;
        let key    = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        // random nonce prepended to the ciphertext so we can decrypt later
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce      = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| anyhow::anyhow!("encrypt: {e}"))?;
        let mut out = nonce_bytes.to_vec();
        out.extend(ciphertext);
        Ok(out)
    }

    fn decrypt_bytes(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key_bytes = self.enc_key.ok_or_else(|| anyhow::anyhow!("no encryption key"))?;
        anyhow::ensure!(data.len() > 12, "ciphertext too short");
        let key    = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce  = Nonce::from_slice(&data[..12]);
        let plain  = cipher.decrypt(nonce, &data[12..])
            .map_err(|e| anyhow::anyhow!("decrypt: {e}"))?;
        Ok(plain)
    }

    pub fn set_key(&mut self, key: [u8; 32]) {
        self.enc_key = Some(key);
        self.encrypt = true;
    }

    pub fn init_encryption(&mut self) -> Result<()> {
        let key = self.ensure_key()?;
        self.enc_key = Some(key);
        Ok(())
    }
}

// helper for pushing a text entry from outside the store
pub fn push_text(
    store:       &Arc<Mutex<Store>>,
    text:        String,
    source_app:  Option<String>,
    deduplicate: bool,
) -> bool {
    let entry = ClipEntry::new_text(text, source_app);
    store.lock().unwrap().push(entry, deduplicate)
}
