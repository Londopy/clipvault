// paste.rs
// writes something to the clipboard then simulates ctrl+v so it gets pasted
// the sleep is annoying but necessary - without it the app reads the clipboard before
// we've finished writing and pastes the wrong thing

use std::thread;
use std::time::Duration;

use anyhow::Result;
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use log::debug;

// paste on a background thread after a short delay. use this from the gui:
// it gives the overlay window time to hide and focus to return to the target
// app, and gives the user time to release hotkey modifiers - sending ctrl+v
// while alt/shift are still held would deliver the wrong combo entirely.
pub fn paste_text_deferred(text: String) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        if let Err(e) = paste_text(&text) {
            log::warn!("deferred paste failed: {e}");
        }
    });
}

pub fn paste_text(text: &str) -> Result<()> {
    {
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(text)?;
    }
    debug!("wrote {} chars to clipboard, simulating paste", text.len());
    // give the os a moment to actually register the clipboard write
    thread::sleep(Duration::from_millis(80));
    simulate_paste()?;
    Ok(())
}

// fires ctrl+v (or cmd+v on mac)
pub fn simulate_paste() -> Result<()> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("enigo init: {e:?}"))?;

    // release modifiers the user may still be holding from the hotkey -
    // a held alt or shift would turn our ctrl+v into a different combo
    for m in [Key::Alt, Key::Shift] {
        let _ = enigo.key(m, Direction::Release);
    }

    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| anyhow::anyhow!("key press: {e:?}"))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| anyhow::anyhow!("key click: {e:?}"))?;
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| anyhow::anyhow!("key release: {e:?}"))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| anyhow::anyhow!("key press: {e:?}"))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| anyhow::anyhow!("key click: {e:?}"))?;
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| anyhow::anyhow!("key release: {e:?}"))?;
    }

    Ok(())
}

// same deal but for images
pub fn paste_image(rgba: &[u8], width: usize, height: usize) -> Result<()> {
    {
        let mut clipboard = Clipboard::new()?;
        let img = arboard::ImageData {
            width,
            height,
            bytes: rgba.into(),
        };
        clipboard.set_image(img)?;
    }
    thread::sleep(Duration::from_millis(80));
    simulate_paste()?;
    Ok(())
}
