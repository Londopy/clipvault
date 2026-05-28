// main.rs
// entry point. spins up all the background threads then gives the main
// thread to egui (it needs the main thread on windows/mac, annoying but whatever)

mod config;
mod daemon;
mod discord;
mod gui;
mod hotkeys;
mod notify;
mod paste;
mod platform;
mod snippets;
mod store;
mod transforms;
mod tray;
mod updater;

use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use log::info;
use tokio::runtime::Runtime;

use crate::config::Config;
use crate::gui::AppEvent;
use crate::store::Store;

fn main() -> Result<()> {
    // set up logging so i can actually see whats happening
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("ClipVault {} starting", env!("CLIPVAULT_VERSION"));

    // load config from disk (or write defaults if it doesnt exist yet)
    let config = Arc::new(Mutex::new(Config::load()?));

    // shared clipboard history - wrapped in Arc<Mutex> so threads can all use it
    let store = Arc::new(Mutex::new({
        let cfg = config.lock().unwrap();
        Store::load(&cfg)?
    }));

    // channel for sending events from hotkeys/tray/updater to the gui
    let (event_tx, event_rx) = std::sync::mpsc::channel::<AppEvent>();

    // tokio runtime for the async stuff (discord + updater)
    let rt = Arc::new(Runtime::new()?);

    // clipboard watcher thread - runs forever in the background
    {
        let store = Arc::clone(&store);
        let config = Arc::clone(&config);
        thread::Builder::new()
            .name("clipvault-daemon".into())
            .spawn(move || {
                if let Err(e) = daemon::run(store, config) {
                    log::error!("clipboard daemon crashed: {e}");
                }
            })?;
    }

    // global hotkey listener thread
    {
        let config = Arc::clone(&config);
        let event_tx2 = event_tx.clone();
        thread::Builder::new()
            .name("clipvault-hotkeys".into())
            .spawn(move || {
                if let Err(e) = hotkeys::run(config, event_tx2) {
                    log::error!("hotkey listener crashed: {e}");
                }
            })?;
    }

    // discord rich presence - async so it doesnt block anything
    {
        let store = Arc::clone(&store);
        let config = Arc::clone(&config);
        let rt2 = Arc::clone(&rt);
        thread::Builder::new()
            .name("clipvault-discord".into())
            .spawn(move || {
                rt2.block_on(async {
                    if let Err(e) = discord::run(store, config).await {
                        log::error!("discord presence error: {e}");
                    }
                });
            })?;
    }

    // check for updates on startup (non-blocking, just sends a notification if there is one)
    {
        let config = Arc::clone(&config);
        let event_tx2 = event_tx.clone();
        let rt2 = Arc::clone(&rt);
        thread::Builder::new()
            .name("clipvault-updater".into())
            .spawn(move || {
                rt2.block_on(async {
                    if let Err(e) = updater::check_and_notify(config, event_tx2).await {
                        log::debug!("updater: {e}");
                    }
                });
            })?;
    }

    // tray + gui both need the main thread so they go last
    gui::run(store, config, event_tx, event_rx)?;

    info!("shutting down");
    Ok(())
}
