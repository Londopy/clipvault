// updater.rs
// checks github releases for a newer version and sends a notification if one exists
// if auto_install is on it'll just download and restart automatically

use std::sync::{mpsc::Sender, Arc, Mutex};

use anyhow::Result;
use log::{debug, info};
use semver::Version;

use crate::config::Config;
use crate::gui::AppEvent;

// change these to your actual github username and repo before shipping
const GITHUB_OWNER: &str = "Londopy";
const GITHUB_REPO:  &str = "clipvault";

pub async fn check_and_notify(
    config:   Arc<Mutex<Config>>,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let (enabled, check_on_startup, auto_install) = {
        let cfg = config.lock().unwrap();
        (cfg.updater.enabled, cfg.updater.check_on_startup, cfg.updater.auto_install)
    };

    if !enabled || !check_on_startup {
        return Ok(());
    }

    debug!("Checking for updates…");

    // spawn_blocking because the http call would block the async runtime
    let latest = tokio::task::spawn_blocking(|| fetch_latest_version()).await??;

    let current = Version::parse(env!("CLIPVAULT_VERSION"))
        .unwrap_or_else(|_| Version::new(0, 0, 0));

    if latest > current {
        info!("Update available: {current} → {latest}");
        let _ = event_tx.send(AppEvent::UpdateAvailable(latest.to_string()));
        if auto_install {
            tokio::task::spawn_blocking(install_update).await??;
        }
    } else {
        debug!("Already up-to-date ({current})");
    }

    Ok(())
}

// hits the github api and parses the latest release tag as a semver version
fn fetch_latest_version() -> Result<Version> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(GITHUB_OWNER)
        .repo_name(GITHUB_REPO)
        .build()?
        .fetch()?;

    let latest = releases.first()
        .ok_or_else(|| anyhow::anyhow!("no releases found"))?;

    let tag = latest.version.trim_start_matches('v');
    Version::parse(tag).map_err(|e| anyhow::anyhow!("semver parse: {e}"))
}

// downloads the update, replaces the binary, then restarts the process
fn install_update() -> Result<()> {
    info!("Installing update…");
    let status = self_update::backends::github::Update::configure()
        .repo_owner(GITHUB_OWNER)
        .repo_name(GITHUB_REPO)
        .bin_name("clipvault")
        .show_download_progress(false)
        .current_version(env!("CLIPVAULT_VERSION"))
        .build()?
        .update()?;

    match status {
        self_update::Status::UpToDate(v)  => debug!("Already up-to-date: {v}"),
        self_update::Status::Updated(v) => {
            info!("Updated to {v}. Restarting…");
            let exe  = std::env::current_exe()?;
            let args: Vec<String> = std::env::args().skip(1).collect();
            std::process::Command::new(exe).args(&args).spawn()?;
            std::process::exit(0);
        }
    }

    Ok(())
}
