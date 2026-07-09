# ClipVault — Cross-Platform Shipping Readiness

Assessment of what it takes to actually ship ClipVault on Windows, macOS, and Linux.
"CI is green" only proves the crate **builds, passes clippy, and passes the unit tests**
(which are just the `transforms.rs` text tests — fully platform-independent). Nothing in
CI exercises the clipboard, hotkeys, tray, paste, or GUI at runtime. So build-pass ≠ works.

Verdict at a glance:

| Platform | Builds | Runs | Ships today | Main blocker |
|----------|:------:|:----:|:-----------:|--------------|
| Windows  | ✅ | ⚠️ | ❌ | console window shows; "start on login" is a stub; no installer |
| macOS    | ✅ | ⚠️ | ❌ | needs `.app` bundle + permissions prompt; no signing/notarization |
| Linux/X11| ✅ | ⚠️ | ❌ | tray event loop; `xdotool` runtime dep; no package |
| Linux/Wayland | ✅ | ❌ | ❌ | hotkeys + paste + app-detection are X11-only |

---

## Cross-cutting issues (hit every OS)

**1. Source-app polling spawns a process 20×/sec (perf bug, macOS + Linux).**
`daemon.rs` calls `platform::get_source_app()` on every 50 ms loop tick, *before* checking
whether the clipboard actually changed. On Windows that's a cheap Win32 call. On macOS it
spawns an `osascript` process and on Linux an `xdotool` process — **20 times per second**.
That will peg a CPU core and drain battery. Fix: only resolve the source app when new
clipboard content is actually detected.

**2. "Start with system" is a no-op on Windows.**
`platform/windows.rs::write_registry_run()` is a stub (`Ok(())` with a TODO). The setting
toggles and "saves," but nothing is written to the registry. macOS (LaunchAgent) and Linux
(systemd/xdg) are actually implemented.

**3. `system_is_dark()` always returns `true`.**
The "system" theme is always dark on every OS (this is the function reconstructed from the
truncated file). Cosmetic, but worth knowing.

**4. No release/packaging pipeline.**
`ci.yml` only lints/tests. There's no job that produces installable artifacts for any OS.

---

## Windows

Closest to shippable, but not there.

- **Console window.** `build.rs` embeds the icon via `winres`, but hiding the console
  requires `#![windows_subsystem = "windows"]` in `main.rs` — it's not present. Right now a
  console window pops up behind the GUI. One-line fix.
- **Startup stub** (see cross-cutting #2). Implement with the `winreg` crate writing to
  `HKCU\...\CurrentVersion\Run`.
- **Packaging.** No installer. Options: MSI via `cargo-wix`, or a portable signed `.exe` +
  zip. Code-signing optional but avoids SmartScreen warnings.

Effort: **low.** A day to make it feel like a real Windows app.

## macOS

Builds now (after the `notify.rs` return-type fix), but a bare binary is not a shippable Mac app.

- **Permissions.** `rdev` (hotkeys), `enigo` (paste), and the `osascript` app-detection all
  require Accessibility / Automation permission. `check_accessibility_permission()` is a stub
  returning `true`, so the app never prompts — it just silently receives no hotkeys until the
  user manually enables it in System Settings. Needs a real `AXIsProcessTrustedWithOptions`
  check that triggers the prompt.
- **App bundle.** Must ship as a `.app` with an `Info.plist` (including
  `NSAppleEventsUsageDescription` for the osascript call) and `LSUIElement` if you want it
  menu-bar-only. As a loose binary, permissions attach to the parent terminal — unusable for
  end users.
- **Signing + notarization.** Unsigned/un-notarized apps are blocked by Gatekeeper
  ("unidentified developer"). Requires an Apple Developer account ($99/yr) for real
  distribution.

Effort: **medium.** The Apple Developer account + notarization is the long pole.

## Linux

Two very different stories depending on display server.

- **X11:** mostly works. `rdev` (hotkeys), `enigo` (paste), and `xdotool` (app-detection)
  are all X11-based.
- **Wayland (default on modern Ubuntu/Fedora/GNOME):** hotkeys, paste-injection, and
  app-detection silently do nothing. `arboard` *can* read the Wayland clipboard, so history
  capture partly works, but the core UX (global hotkey → paste) is dead. The code comments
  acknowledge this.
- **`xdotool` is an external binary**, not a Rust dependency — the user must have it
  installed, or app-detection fails even on X11.
- **Tray event loop (needs verification).** `tray-icon` on Linux relies on a GTK/glib main
  loop to deliver menu events, but eframe drives a winit loop instead. Tray clicks may never
  arrive. This is the one item I'd actually test on a Linux box before trusting it.
- **Packaging.** Best bet is an AppImage (bundles libs, runs across distros) or a `.deb`.
  Runtime deps today: `libgtk-3`, `libxdo`, and `xdotool`.

Effort: **high**, mostly because of Wayland. Realistic near-term stance: **support X11,
explicitly mark Wayland unsupported**, rather than half-working silently.

---

## Recommended path

Ship in the order that matches reality: Windows → macOS → Linux/X11, with Wayland deferred.

**Phase 1 — make each build behave like a real app (code).**
- Add `#![windows_subsystem = "windows"]` (Windows console).
- Implement Windows startup via `winreg`.
- Move `get_source_app()` so it only runs on an actual clipboard change (perf).
- Real macOS accessibility check + prompt.
- On Linux, detect Wayland and surface a clear "X11 required" message instead of silent failure.

**Phase 2 — packaging & release CI.**
- Add a `release.yml` triggered on tags that builds per-OS artifacts:
  Windows `.msi`/zip, macOS `.app`/`.dmg`, Linux AppImage/`.deb`.
- `cargo-dist` can scaffold most of this in one shot; macOS notarization and Windows signing
  are bolt-ons.

**Phase 3 — verification.**
- Actually launch on each OS and confirm: hotkey opens overlay, paste works, tray menu
  responds, notifications appear, "start on login" persists.
- This is the step CI cannot do for you.

---

## What I can do next

Pick a target and I'll implement it:
- **Windows polish** (console + startup registry + perf fix) — fastest visible win.
- **macOS bundling** (Info.plist, accessibility prompt, `.app` build job).
- **Linux Wayland guard** + AppImage packaging.
- **`release.yml`** that cross-builds and uploads artifacts on tag.
