// snippets.rs
// saved text snippets you can paste with a shortcode like ;;email
// also handles import/export so you can back them up or share them

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub content: String,
    // e.g. "email" means you can trigger it by typing ";;email"
    pub shortcode: Option<String>,
    pub category: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Snippet {
    pub fn new(
        name: String,
        content: String,
        shortcode: Option<String>,
        category: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            content,
            shortcode,
            category,
            created_at: now,
            updated_at: now,
        }
    }

    // replaces literal \n with actual newlines so multi-line snippets work
    pub fn expanded_content(&self) -> String {
        self.content.replace("\\n", "\n")
    }
}

// this is the json format we save to disk
#[derive(Debug, Default, Serialize, Deserialize)]
struct SnippetsFile {
    version: u32,
    snippets: Vec<Snippet>,
}

pub struct SnippetStore {
    pub snippets: Vec<Snippet>,
    // maps shortcode string -> snippet id, rebuilt whenever things change
    shortcode_map: HashMap<String, String>,
}

impl SnippetStore {
    pub fn load() -> Result<Self> {
        let path = Self::path();
        let snippets = if path.exists() {
            let raw: SnippetsFile =
                serde_json::from_str(&fs::read_to_string(&path).context("reading snippets file")?)
                    .context("parsing snippets file")?;
            raw.snippets
        } else {
            Vec::new()
        };
        let mut store = Self {
            snippets,
            shortcode_map: HashMap::new(),
        };
        store.rebuild_shortcode_map();
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = SnippetsFile {
            version: 1,
            snippets: self.snippets.clone(),
        };
        fs::write(&path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }

    pub fn path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipvault")
            .join("snippets.json")
    }

    // add a new snippet, errors if the shortcode is already taken
    pub fn add(&mut self, snippet: Snippet) -> Result<()> {
        if let Some(ref sc) = snippet.shortcode {
            anyhow::ensure!(
                !self.shortcode_map.contains_key(sc.as_str()),
                "shortcode '{sc}' already in use"
            );
        }
        self.snippets.push(snippet);
        self.rebuild_shortcode_map();
        self.save()
    }

    // update any fields that are Some, leave the rest alone
    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        content: Option<String>,
        shortcode: Option<Option<String>>,
        category: Option<Option<String>>,
    ) -> Result<()> {
        let sn = self
            .snippets
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("snippet not found: {id}"))?;
        if let Some(n) = name {
            sn.name = n;
        }
        if let Some(c) = content {
            sn.content = c;
        }
        if let Some(sc) = shortcode {
            sn.shortcode = sc;
        }
        if let Some(cat) = category {
            sn.category = cat;
        }
        sn.updated_at = Utc::now();
        self.rebuild_shortcode_map();
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.snippets.retain(|s| s.id != id);
        self.rebuild_shortcode_map();
        self.save()
    }

    pub fn by_id(&self, id: &str) -> Option<&Snippet> {
        self.snippets.iter().find(|s| s.id == id)
    }

    // returns a sorted list of all category names (deduped)
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .snippets
            .iter()
            .filter_map(|s| s.category.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        cats
    }

    // pass in the shortcode without the ";;" prefix
    pub fn resolve_shortcode(&self, sc: &str) -> Option<&Snippet> {
        let id = self.shortcode_map.get(sc)?;
        self.by_id(id)
    }

    // clears and rebuilds the shortcode->id map whenever snippets change
    fn rebuild_shortcode_map(&mut self) {
        self.shortcode_map.clear();
        for sn in &self.snippets {
            if let Some(ref sc) = sn.shortcode {
                self.shortcode_map.insert(sc.clone(), sn.id.clone());
            }
        }
    }

    pub fn export_json(&self, path: &PathBuf) -> Result<()> {
        let file = SnippetsFile {
            version: 1,
            snippets: self.snippets.clone(),
        };
        fs::write(path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }

    pub fn import_json(&mut self, path: &PathBuf) -> Result<usize> {
        let raw: SnippetsFile =
            serde_json::from_str(&fs::read_to_string(path).context("reading import file")?)
                .context("parsing import file")?;
        let count = raw.snippets.len();
        for mut sn in raw.snippets {
            // give each imported snippet a fresh uuid so ids don't collide
            sn.id = Uuid::new_v4().to_string();
            sn.created_at = Utc::now();
            sn.updated_at = Utc::now();
            self.snippets.push(sn);
        }
        self.rebuild_shortcode_map();
        self.save()?;
        Ok(count)
    }
}
