# Building ClipVault

## Prerequisites

### Windows
- Rust stable toolchain (`rustup toolchain install stable`)
- Windows SDK (comes with Visual Studio Build Tools)
- Optional: WiX Toolset for MSI packaging

### macOS
- Rust stable toolchain
- Xcode command line tools (`xcode-select --install`)
- Apple Developer account (for notarization/distribution)
- Grant Accessibility permission at first run (System Settings → Privacy & Security → Accessibility)

### Linux
- Rust stable toolchain
- System dependencies:
  ```
  sudo apt-get install -y \
    libx11-dev libxi-dev libxtst-dev \
    libdbus-1-dev pkg-config libssl-dev \
    libxdo-dev libgtk-3-dev
  ```

## Building

```sh
# Debug build
cargo build

# Release build (optimised, stripped binary)
cargo build --release
```

## Running

```sh
cargo run --release
```

## Configuration

On first run, a default config is written to:
- Windows: `%APPDATA%\clipvault\config.toml`
- macOS:   `~/Library/Application Support/clipvault/config.toml`
- Linux:   `~/.config/clipvault/config.toml`

See `config.default.toml` in this repo for all available options.

## Before You Ship

1. Set `GITHUB_OWNER` and `GITHUB_REPO` in `src/updater.rs`
2. Set your Discord application snowflake ID in `config.toml`
3. Add a real `assets/icon.png` (256×256 RGBA recommended)
4. On macOS: add your Apple Developer credentials as GitHub Actions secrets for notarization

## Discord Rich Presence

Register an application at https://discord.com/developers/applications, copy the
numeric Application ID, and set it as `discord.application_id` in your config.
