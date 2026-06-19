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
