// discord.rs
// shows what youre doing in discord rich presence
// shows the item count and how long youve been running
// if discord isnt open or the app id isnt set it just skips quietly

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use discord_presence::Client;
use log::{debug, warn};
use tokio::time::sleep;

use crate::config::Config;
use crate::store::Store;

// how long to wait before trying to reconnect after discord drops
const RECONNECT_DELAY_SECS: u64 = 15;
// how often to refresh the presence (every 10 seconds)
const UPDATE_INTERVAL_SECS: u64 = 10;

pub async fn run(store: Arc<Mutex<Store>>, config: Arc<Mutex<Config>>) -> Result<()> {
    // bail early if its disabled or the user never set their app id
    let (enabled, app_id_str) = {
        let cfg = config.lock().unwrap();
        (cfg.discord.rich_presence, cfg.discord.application_id.clone())
    };

    if !enabled || app_id_str == "REPLACE_WITH_YOUR_APP_ID" {
        debug!("Discord Rich Presence disabled or app_id not configured.");
        return Ok(());
    }

    let app_id: u64 = app_id_str.parse()
        .map_err(|_| anyhow::anyhow!("discord.application_id must be a numeric snowflake ID"))?;

    // record when we started so the "elapsed" timer in discord is accurate
    let session_start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // keep reconnecting if discord closes, its pretty normal for that to happen
    loop {
        match connect_and_update(app_id, session_start, &store, &config).await {
            Ok(_) => {}
            Err(e) => {
                warn!("Discord presence disconnected: {e}. Reconnecting in {RECONNECT_DELAY_SECS}s…");
                sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
            }
        }
    }
}

async fn connect_and_update(
    app_id: u64,
    session_start: u64,
    store: &Arc<Mutex<Store>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    let mut client = Client::new(app_id);

    client.on_ready(move |_ctx| {
        debug!("Discord IPC ready");
    });

    client.start();

    loop {
        // check if user turned it off while we were running
        let enabled = config.lock().unwrap().discord.rich_presence;
        if !enabled {
            return Ok(());
        }

        let item_count = store.lock().unwrap().len();
        let state = format!("{item_count} items in history");

        client.set_activity(|act| {
            act.state(&state)
               .details("Managing clipboard")
               .timestamps(|ts| ts.start(session_start))
        })
        .ok();

        debug!("Discord presence updated: {state}");
        sleep(Duration::from_secs(UPDATE_INTERVAL_SECS)).await;
    }
}