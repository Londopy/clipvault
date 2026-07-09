// daemon.rs
// background thread that watches the clipboard for changes
// polls every 50ms which feels fast enough without eating too much cpu

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use arboard::Clipboard;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::ImageFormat;
use log::{debug, warn};

use crate::config::Config;
use crate::platform;
use crate::store::{ClipEntry, Store};

const POLL_MS: u64 = 50;

pub fn run(store: Arc<Mutex<Store>>, config: Arc<Mutex<Config>>) -> Result<()> {
    let mut clipboard = Clipboard::new()?;

    // track the last thing we saw so we only push when something actually changes
    let mut last_text: Option<String> = None;
    let mut last_image: Option<Vec<u8>> = None;

    loop {
        thread::sleep(Duration::from_millis(POLL_MS));

        // grab the settings we need for this tick
        let (paused, incognito, deduplicate, excluded_apps, mask_passwords, persist) = {
            let s = store.lock().unwrap();
            let cfg = config.lock().unwrap();
            (
                s.paused,
                s.incognito,
                cfg.general.deduplicate,
                cfg.security.excluded_apps.clone(),
                cfg.security.mask_passwords,
                cfg.general.persist_history,
            )
        };

        if paused || incognito {
            continue;
        }

        // check for new text
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() && Some(&text) != last_text.as_ref() {
                last_text = Some(text.clone());
                // only resolve the source app when something actually changed -
                // on macos/linux this spawns a process, so doing it every 50ms
                // tick would peg a core (SHIPPING.md cross-cutting issue #1)
                let source_app = platform::get_source_app();
                if source_is_excluded(&source_app, &excluded_apps) {
                    continue;
                }
                let text = if mask_passwords && looks_like_password(&text) {
                    "[masked]".to_string()
                } else {
                    text
                };
                let entry = ClipEntry::new_text(text, source_app.clone());
                let pushed = store.lock().unwrap().push(entry, deduplicate);
                if pushed && persist {
                    if let Err(e) = store.lock().unwrap().save() {
                        warn!("failed to save history: {e}");
                    }
                }
            }
        }

        // check for new images - convert to a webp thumbnail before storing
        if let Ok(img) = clipboard.get_image() {
            let raw = img.bytes.to_vec();
            if Some(&raw) != last_image.as_ref() {
                last_image = Some(raw.clone());
                let source_app = platform::get_source_app();
                if source_is_excluded(&source_app, &excluded_apps) {
                    continue;
                }
                match make_thumbnail(&raw, img.width as u32, img.height as u32) {
                    Ok(thumb_b64) => {
                        let entry = ClipEntry::new_image(thumb_b64, None, source_app.clone());
                        let pushed = store.lock().unwrap().push(entry, deduplicate);
                        if pushed && persist {
                            if let Err(e) = store.lock().unwrap().save() {
                                warn!("failed to save history: {e}");
                            }
                        }
                    }
                    Err(e) => warn!("image thumbnail error: {e}"),
                }
            }
        }
    }
}

// true if the app the user copied from is on the exclusion list
fn source_is_excluded(source_app: &Option<String>, excluded: &[String]) -> bool {
    if let Some(app) = source_app {
        if excluded.iter().any(|ex| app.contains(ex.as_str())) {
            debug!("skipping clipboard from excluded app: {app}");
            return true;
        }
    }
    false
}

// shrinks the image down to at most 200x200 and encodes it as base64 webp
fn make_thumbnail(rgba: &[u8], width: u32, height: u32) -> Result<String> {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("invalid image dimensions"))?;
    let dyn_img = image::DynamicImage::ImageRgba8(img);
    let thumbnail = dyn_img.thumbnail(200, 200);
    let mut buf = std::io::Cursor::new(Vec::new());
    thumbnail.write_to(&mut buf, ImageFormat::WebP)?;
    Ok(B64.encode(buf.into_inner()))
}

// rough heuristic to guess if something is a password
// single line, no spaces, has uppercase + lowercase + digit + symbol = probably a password
fn looks_like_password(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.contains('\n') || trimmed.contains(' ') {
        return false;
    }
    let has_upper = trimmed.chars().any(|c| c.is_uppercase());
    let has_lower = trimmed.chars().any(|c| c.is_lowercase());
    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    let has_symbol = trimmed.chars().any(|c| !c.is_alphanumeric());
    let len = trimmed.len();
    (8..=64).contains(&len) && has_upper && has_lower && has_digit && has_symbol
}
