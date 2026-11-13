# Changelog

All notable changes to Roomtone. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/). While the version is below 1.0, minor versions may change behaviour.

## [Unreleased]

### Added
- **Backup and restore** (Settings › Backup and restore): save the system's Sonos favorites, playlists (with their songs) and alarms, plus Roomtone's own folders, stations, podcasts, saved groups, kept favorites and preferences, to one JSON file (never sign-ins or the access key), and restore from it on the same or a new system. Restoring only adds: duplicates are skipped, nothing existing is changed, alarms are matched to rooms by name, and a summary says what was added, skipped or couldn't be restored. Saving was checked against a real S1 system (read only); restoring to real speakers isn't tried yet.

## [0.5.3] — 2026-11-13

### Fixed
- A settings file that can't be read (a typo from editing by hand, a half-written file) is no longer silently replaced by the defaults: it is moved to `config.json.broken-<n>`, a notice at the top of Settings says where it went, and if it can't be moved nothing is saved that run. Background readers never touch it.

### Added
- **Speaker updates** (Settings › Speaker updates, off by default): instead of asking the speakers again and again, Roomtone subscribes to their UPnP events and re-reads a group the moment its speaker reports a change, so changes made elsewhere show up at once. While events work, polling slows to a safety net (10 s / 30 s / 60 s instead of 1 s / 4 s / 20 s), which also cuts network traffic and CPU use. If the speakers can't reach this computer (a firewall), Roomtone says so and keeps polling as before. Needs an incoming connection from the speakers; only subscribed speakers' notices are accepted. Tried once with a real S1 speaker; not yet with S2. New `speaker_events` field in `config.json`.
- **Settings guide** ([docs/SETTINGS.md](docs/SETTINGS.md)): every setting, where it is in the app, its `config.json` name and default, command-line options and environment variables, and how to back up, move or reset. Linked from Settings › About (*Settings guide*), `--help` and the README.

## [0.5.2] — 2026-11-13

### Added
- **Installers** on each release: a macOS `.dmg` (Roomtone.app with its icon and Local Network prompt text, ad-hoc signed), a Windows setup `.exe` (per-user, Start menu entry, optional desktop shortcut, uninstaller) and a Linux `.deb` and `.rpm` (program, menu entry and icon), next to the plain `.zip`/`.tar.gz`. They aren't code-signed or notarised yet.
- `roomtone --write-icon <file> [size]` saves the app icon (a PNG, or a Windows `.ico`) for the installers.
- The Release workflow can also be started by hand on a branch to try the installers without publishing a release.
- **Light, dark or automatic appearance** (Settings › Appearance; `--theme` for one run). Automatic follows the computer's setting live. Dark mode is unchanged.
- **19 more assistant (MCP) tools** (48 in all): `browse`, `list_recent`, `add_favorite`, `remove_favorite`, `save_queue_as_playlist`, `move_in_queue`, `pause_all`, `rename_room`, `create_alarm`, `delete_alarm`, `save_group`, `delete_saved_group`, `list_services`, `check_service`, `find_speakers`, `add_station`, `remove_station`, `subscribe_podcast`, `unsubscribe_podcast`.
- **MCP resources** (`roomtone://status`, `now-playing`, `favorites`, `alarms`, `queue/{room}`), **prompts** (`play_for_mood`, `bedtime`, `whole_home`, `morning_alarm`) and **completions** for room and group names.
- **97 unit tests** covering the protocol parsers, rules, settings, translations, the demo system and the assistant link, including its HTTP security checks. Parsers for topology, update offers, podcast feeds and radio playlists were split out so they can be tested without a network.

## [0.5.1] — 2026-11-12

### Fixed
- **Windows build**: eframe 0.37 defaults to the wgpu renderer, whose DirectX 12 backend doesn't compile on Windows (wgpu-hal needs `windows` 0.62, the latest gpu-allocator uses 0.58). Roomtone now uses eframe's glow renderer, as before the upgrade, which drops the wgpu dependencies.
- An unused-function warning on Windows and Linux (the icon-to-PNG helper is only used by the macOS About box).

### Added
- The project's home is <https://github.com/ChrispyGee/Roomtone>: linked from Settings › About (source code, report a problem), the macOS About box, `--help`, the README and Cargo.toml.

### Changed
- **egui/eframe 0.31 → 0.37** (Dependabot #2): the app now draws through `App::ui`; panels use `egui::Panel` attached to the root `Ui`; popups use `egui::Popup::menu`; menus close with `ui.close()`; `--screenshot` receives the picture through a `ScreenshotCallback`; the global style is set with `set_global_style` and the theme is pinned to dark; `Ui::id` → `Ui::scope_id` and `Id::new` → `Id::unique`. Every page was compared before and after with `--screenshot`: same layout (text anti-aliasing differs slightly).
- Dependencies: lofty 0.25, rfd 0.18 and roxmltree 0.21 (from Dependabot #3, #4, #5); GitHub Actions checkout v7 (#1).
- CI: `cargo clippy -D warnings` must now pass on Windows and Linux too.
- `.gitignore` also keeps out the app's own data files (`config.json`, `speakers.json`, `songs.json`), `.env` files and key files; CONTRIBUTING has a privacy check; RELEASING explains keeping your commit email private.

## [0.5.0] — 2026-11-12

### Added
- **Software updates** in Settings: check each system for newer speaker software and install it on every speaker after a confirmation. S1 systems are only ever offered S1 updates. Assistant tools `check_for_updates` and `install_update` (needs `confirm: true`).
- **About** section in Settings: version, platform, systems and their software versions, language, music-server address, assistant-link status, settings folder (with *Show*), what's new, help commands, credits and an independence notice.
- **macOS About box** filled in (name, version, icon, description, credits, copyright), and translated application-menu titles (*About Roomtone*, *Quit Roomtone*…).
- An **app icon** drawn in code, used for the window, Dock/taskbar and About box.
- **Favorites from Apple Music, YouTube Music and SoundCloud**: a "Your favorites" list on each service card, cards in Music Library, labels and service-name matching in Search.
- **Platform-aware help**: `--help` and `--help mcp` print steps and real paths for macOS, Windows or Linux; Settings › Assistant link has a *How to connect* panel.
- Project documents: README, SPEC, ARCHITECTURE, PROTOCOL, HEADLESS_AND_MCP, ROADMAP, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, AGENTS; issue and pull-request templates; CI for macOS, Windows and Linux; Dependabot.
- Dual licence: MIT or Apache-2.0.

### Changed
- Windows settings now live in `%APPDATA%\Roomtone\` (previously `%APPDATA%\.config\roomtone`). Move the old folder's files across to keep them.
- The code is formatted with `rustfmt` (`rustfmt.toml`) and is free of `clippy` warnings.

## [0.4.0] — 2026-11-11

### Added
- **Headless mode** (`--headless`) and an **MCP server** for AI assistants (`--mcp` on stdin/stdout, or HTTP on `127.0.0.1:8790`). Off by default; look-only mode; access key; origin and host checks; settings from the command line (`--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port`, `--mcp-info`).
- **Six languages**: English, German, French, Spanish, Italian, Dutch; *Automatic* follows the computer; `--lang` overrides.
- **Play along across systems**: a group in one system plays the same music as a group in another (for S1 with S2) and follows its controls.
- Streaming services: *Check* button, *Ready to connect* badges, account number detected from what's playing, automatic retry with other account numbers, and a notice when playback doesn't start.

## [0.3.x and earlier]

The first versions of the controller:
- Discovery of S1 and S2 systems (SSDP, remembered speakers, manual IP), the demo system, automatic reconnect after sleep.
- Rooms, grouping, saved groups, group and room volume, Pause all.
- Playback controls, queue editing, save queue as playlist, sleep timer, crossfade, Now Playing page.
- Search across every source; Favorites (including ones kept in the app), Playlists, Music Library (system library and this computer's folders), Radio (My stations and a free directory), Podcasts, Internet Archive, streaming services through the speakers' own music-service connection.
- Line-In & TV, Alarms, room settings (EQ, loudness, night sound, speech enhancement, status light, rename).
- `--screenshot`, `--demo` and `--view` for testing.

[Unreleased]: https://github.com/ChrispyGee/Roomtone/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/ChrispyGee/Roomtone/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/ChrispyGee/Roomtone/releases/tag/v0.4.0
