# Changelog

All notable changes to Roomtone. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/). While the version is below 1.0, minor versions may change behaviour.

## [Unreleased]

### Added
- The project's home is <https://github.com/ChrispyGee/Roomtone>: linked from Settings › About (source code, report a problem), the macOS About box, `--help`, the README and Cargo.toml.

### Changed
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
