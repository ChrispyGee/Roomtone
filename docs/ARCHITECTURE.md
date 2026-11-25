# How the code fits together

This is the map of the code. What the app should do is in [SPEC.md](../SPEC.md); how the speakers are spoken to is in [PROTOCOL.md](PROTOCOL.md). Each source file also starts with a `//!` paragraph saying what it's for, and `cargo doc --no-deps --open` turns those and the `///` comments into browsable pages.

## Big picture

```
            ┌────────────── window (ui/) ──────────────┐      ┌──── assistant link (mcp.rs) ────┐
            │ draws AppState every frame,              │      │ stdin/stdout  or  HTTP :8790    │
            │ pushes Cmd for every click               │      │ tools/call → Cmd, waits for it  │
            └───────────────┬──────────────────────────┘      └───────────────┬─────────────────┘
                            │ Backend::send(Cmd)                              │ send_and_wait(Cmd)
                            ▼                                                 ▼
                 ┌──────────────────────── backend.rs ─────────────────────────────┐
                 │ optimistic(): update AppState at once (sliders feel instant)    │
                 │ mirror():     copy the Cmd to play-along groups                 │
                 │ channel ──► worker thread: run_common() → demo::apply()         │
                 │                                       or run_live()             │
                 │ poller thread: polls groups every 1–4 s, topology every 20 s,   │
                 │                reconnects after sleep or repeated failures      │
                 └──────────────┬──────────────────────────────┬───────────────────┘
                                │ Arc<Mutex<AppState>>         │
                 net/ (speakers, port 1400)          sources/ (this computer, radio, podcasts,
                                                               archive, streaming services)
```

- **One shared state**: `model::AppState` behind `Arc<Mutex<…>>` in `backend::Backend`. The window locks it once per frame to draw; workers lock it briefly to write results. Never hold the lock while waiting on the network.
- **One command type**: `backend::Cmd`. Everything the person (or an assistant) asks for is a `Cmd`. That keeps the window, headless mode and MCP behaving identically.
- **No window needed**: the backend only knows a `backend::Waker`, which asks the window to redraw if there is one and does nothing in `--headless` and `--mcp`.

## Two crates

The code is a Cargo workspace of two crates. **`core/` (`roomtone-core`)** holds everything that talks to speakers and keeps their state: discovery and the speaker protocol (`net/`), `backend.rs`, `model.rs`, `config.rs`, the sources, scenes, backups, limits, the assistant tool layer (`mcp.rs`), the demo system, wording (`i18n.rs`) and the tests' helpers. It has no window code, and knows a window only as a function it is given to call when the state changes (`backend::Waker`); a program using it can also name the folder for its files (`Config::set_data_dir`). **The root crate (`roomtone`)** is the desktop program: `main.rs`, the window (`ui/`), the Mac menu bar and player, the Windows tray, the command line (`cli.rs`), the crash log and window-size memory. It re-exports the core's modules under their old names, so `crate::backend` still works there. Files listed below are in `core/src/` unless they are `main.rs`, `cli.rs`, `ui/`, `traymenu.rs`, `brand.rs`, `crash.rs`, `window.rs`, `macos_*.rs` or `windows_tray.rs` (all in `src/`).

## Modules

| File | Role |
| --- | --- |
| `main.rs` | Reads options; opens the window (drawing with OpenGL, and again with DirectX on Windows if that fails), or runs headless/MCP, or changes MCP settings from the command line. On Windows it is a window program (no console window) that attaches to the terminal it was started from |
| `help.rs` | `--help` and `--help mcp`, written for the platform being run on (paths, service managers, firewall notes) |
| `backend.rs` | `Cmd`, `Backend` (`send`, `send_and_wait`), the worker and poller threads, discovery and reconnect, streaming playback with account retry, service checks, play-along (`mirror`, `sync_link`), software updates (`check_updates`, `install_update`) |
| `model.rs` | Data the window shows: `Household`, `Room`, `Group`, `NowPlaying`, `Item`, `Alarm`, `AppState` |
| `config.rs` | `Config`, saved as `config.json`: folders, stations, podcasts, services, sign-ins, kept favorites, saved groups, language, `McpConfig` |
| `cli.rs` | The control commands (`roomtone play`, `volume`, `scene`, `backup`…): turns words into assistant tool calls and sends them to the running app's link |
| `scene.rs` | Doing a scene: the same steps an assistant would take, in order, stopping at the first that fails |
| `backup.rs` | The backup file (favorites, playlists, alarms and Roomtone's own lists): building, reading, restoring, and the playlist export path |
| `m3u.rs`, `csv.rs`, `pls_xspf.rs` | Playlist files: writing and reading M3U/M3U8, CSV, PLS and XSPF |
| `lyrics.rs` | The experimental lyrics look-up (lrclib.net): parser for timed lines, exact-then-search order, in-memory store key |
| `history.rs` | The listening history (`history.json`) and the tracker that decides what counts as played |
| `window.rs` | The remembered window size and position |
| `version.rs` | The optional look for a newer Roomtone on GitHub |
| `s2.rs` | Reading what a speaker says about itself, and judging whether it could move to S2 |
| `diagnose.rs`, `status.rs` | *Can't find your speakers?* (checks, advice, redacted report) and wired/wireless from a speaker's network counters |
| `stream_probe.rs` | The experimental stream look-up: a playing stream's address and its bitrate from the reply headers (off by default) |
| `quality.rs` | Sound-quality labels read from what the speaker reports for the playing track |
| `crash.rs` | The local crash log: a panic hook that writes `crash.log` (addresses and the home folder blanked out); nothing is sent |
| `known.rs` | `speakers.json`: speakers from last time, for an instant start; forgets systems unseen for 30 days |
| `demo.rs` | The made-up S1 and S2 systems, and how commands change them (`apply`, `tick`) |
| `frame.rs` | The now-playing display: a read-only HTTP service (page, `art.jpg`, `now.json`) behind a secret in the address; `start` opens and closes the port as settings change |
| `limits.rs` | Volume safeguards: `limit` lowers a volume command that would take a room over its limit (called from `Backend::send`), quiet-hours and cap arithmetic, `rooms_over` for the optional keep-down guard |
| `mcp.rs` | The MCP server: tool list, `Mcp::handle` (JSON-RPC), `serve_stdio`, `start_http` (opens and closes the port as settings change) |
| `i18n.rs` | `t!`, `tn!`, `n_!`, language detection, the tables in `locales/*.tsv`, and the tests that check every phrase is translated |
| `net/announce.rs` | Listens for speakers announcing themselves (SSDP `NOTIFY`) so a new or returning speaker is searched for at once |
| `net/discovery.rs` | SSDP search (out of every network connection) and reading each system's topology |
| `net/soap.rs` | A small UPnP/SOAP client for the speakers' port 1400 |
| `net/speaker.rs` | Every action on a speaker: transport, volume, browse, queue, favorites, grouping, inputs, EQ, alarms, music-service list, account numbers |
| `net/events.rs` | UPnP events: subscribing, the listener for `NOTIFY` messages, renewing and cancelling |
| `net/didl.rs` | DIDL-Lite metadata, reading and writing |
| `sources/mod.rs` | Registry of sources and services (`SERVICES`), browse/search dispatch, favorites by service |
| `sources/local.rs` | Scanning music folders, and the file server the speakers fetch from (port 52880) |
| `sources/radio.rs`, `podcasts.rs`, `archive.rs` | The free online sources |
| `sources/smapi.rs` | Streaming services through the speakers' music-service API (sign-in links, browse, search, playable addresses) |
| `sources/progress.rs` | Podcast progress and played marks (`podcast-progress.json`) |
| `sources/memory.rs` | `songs.json`: details of songs the speakers only know by address |
| `ui/mod.rs` | The window frame: sidebar, rooms/queue panel, bottom bar, dialogs, toasts, the screenshot option |
| `ui/pages.rs` | Every page: home, search, library, radio, services, inputs, now playing, alarms, settings, room settings |
| `ui/layout.rs` | The Speaker setup card (Settings › Speakers) and its confirmation window |
| `ui/scenes.rs`, `ui/lists.rs` | The Scenes page, its editor window and the Home row; sorting, filtering and the A–Z strip for long lists |
| `ui/widgets.rs`, `theme.rs`, `art.rs` | Drawn icons and controls (each one must name itself for screen readers with `describe`, and show `focus_ring`), the dark and light palettes (`TEXT()`, `SURFACE()`… switch with the appearance), album art |
| `mcp/notify.rs` | Change notifications: subscriptions per session and the watcher that compares what's known once a second |
| `mcp/extra.rs` | More assistant tools (browse, favorites, queue editing, rooms, alarms, saved groups, scenes, S2 check, network check, playlist export, services, stations, podcasts), resources, prompts and completions |
| `test_support.rs` | Test helpers: a scratch settings folder, English wording, a running demo backend |
| `brand.rs` | The app icon, drawn in code (RGBA and PNG) |
| `layout.rs` | Speaker layouts: reads stereo pairs and soundbar sets from the topology (`parse`), builds the channel maps the speakers want (`pair_map`, `set_map`) |
| `traymenu.rs` | The menu behind the menu-bar and notification-area icons: `entries` works out what it shows from the state, `run` does what an item says; shared and tested on every system |
| `windows_tray.rs` | Windows only: the notification-area icon (`tray-icon`), the menu rebuilt on right-click, and hiding and showing the window with Win32 calls |
| `ui/popup.rs` | The small player window the Windows notification-area icon opens (drawn with the app's own widgets as a second window; `ROOMTONE_TEST_POPUP=1` draws it beside the window on any system, to picture it) |
| `macos_player.rs` | macOS only: the pop-over player that opens from the menu-bar icon (native AppKit controls; what it shows is `traymenu::player`, what it does goes through the normal commands) |
| `macos_menubar.rs` | macOS only: the menu-bar icon and its menu, built each time it opens from the app's state (`entries`, pure and tested), plus hide and show for *Keep running when the window is closed* |
| `macos_about.rs` | macOS only: the filled-in About box, the Dock icon and translated application-menu titles |

## Life of a command

1. A click calls `App::send(Cmd::Pause(gid))` (the window queues commands during drawing and sends them after it releases the lock). An MCP `tools/call` calls `Backend::send_and_wait`.
2. `Backend::send` applies `optimistic()` so the screen changes at once, adds play-along copies from `mirror()`, and puts `(ticket, Cmd)` on the channel.
3. The worker drains the channel and folds repeated slider moves into the latest one (`coalesce_key`). Then:
   - `run_common()` handles what doesn't need speakers, or does its own demo/live split: settings, other sources, search, grouping plans, play-along links, service checks, software updates. It returns `Some(cmd)` to pass the command on.
   - In demo mode `demo::apply()` changes the made-up state; otherwise `run_live()` calls `net::speaker`.
4. The worker records `done_seq` (and any error under its ticket), shows errors as toasts and wakes the window. `send_and_wait` returns when its ticket is done.
5. The poller later reads the true state back from the speakers.

Slow work (discovery, browsing, search, service sign-in) runs on its own short-lived thread and writes into `AppState.browse`, keyed by list id (`FV:2`, `SQ:`, `A:ALBUM`, `search:<source>:<term>`, `SVC:<sid>/<id>`, `FVSVC:<sid>`, …).

## Threads

| Thread | Lives | Does |
| --- | --- | --- |
| Main (window) | Whole run | Draws every frame (at least every 250 ms) and sends commands; never waits on the network |
| Worker | Whole run | Runs commands in order (`worker`) |
| Poller | Whole run | Polls groups and topology (slower while speaker events work); demo ticks; reconnects (`poller`) |
| Events manager | Whole run, active only while *Speaker updates* is on | Keeps UPnP subscriptions in line with the known speakers and turns notices into reads of the affected group (`events_manager`, `net::events`) |
| Event listener | While subscribed | Accepts `NOTIFY` messages from the speakers, one short thread per connection (`net::events::listen`) |
| Short-lived | Per task | Discovery, browsing, search per source, sign-in polling, service checks, updates, play-along sync |
| Music server | Whole run | Serves this computer's folders to the speakers (`sources::local`) |
| Version checker | Whole run, active only while *New versions* is on | Looks at the latest release once a day, or when *Check now* is pressed |
| Change watcher | One per assistant session that follows resources | Compares what's known once a second and sends `notifications/resources/updated` (stdout or the HTTP event stream) |
| Assistant link | While on | HTTP listener and one thread per connection (`mcp::start_http`), or the stdin loop (`mcp::serve_stdio`) |

## Conventions

- **Wording the person sees** is plain, friendly English in the code, wrapped in `t!` and translated (below). No jargon or bare error codes: say what happened and what to do.
- **Never block the window**: anything that touches the network goes through a `Cmd` or a short-lived thread. Lock `AppState` briefly and never across a network call.
- **Demo parity**: every new `Cmd` must also work in `demo::apply` (or in `run_common`'s demo branch), so every screen can be tried without speakers and the automatic screenshot checks keep working.
- **Same behaviour everywhere**: the window, `--headless` and the assistant link share `Cmd`s. Don't put speaker logic in `ui/` or `mcp.rs`.
- **Irreversible actions** (software updates, anything that could move S1 to S2) need an explicit confirmation in the window and `confirm: true` for assistants. Re-check guards just before acting.
- **Stored files** (`config.json` and friends) must stay readable by older and newer versions: new fields get `#[serde(default)]`, and nothing is renamed.
- **Colours**: always use the palette functions in `ui/theme.rs` (`TEXT()`, `SURFACE()`, `overlay(a)` for faint washes), never fixed colours, so light and dark both work. Check new screens with `--theme light` and `--theme dark`.
- **Tests**: add a unit test for every parser and rule; drive behaviour through the demo backend (`test_support::demo_backend()`), never real speakers.
- **Style**: `cargo fmt` (see `rustfmt.toml`: lines up to 180 characters) and `cargo clippy --all-targets` with no warnings. Comments explain *why*; every file starts with a `//!` summary.
- **Platform code** uses `cfg!(target_os = ...)` where all branches compile everywhere, and `#[cfg]` only for platform crates (`macos_about.rs`, `macos_menubar.rs`, `windows_tray.rs`, `objc2`, `tray-icon`).

## Translations

- Write English in the code: `t!("Play")`, `t!("Playing in {room}", room = name)`, `tn!(n, "1 room", "{n} rooms")`. For constants use `n_!("Artists")` and show the value with `i18n::tr(value)`.
- Each `locales/<code>.tsv` line is `English<TAB>Translation`. Placeholders must survive unchanged.
- `cargo test` fails if any marked phrase lacks a translation in any language, or a translation drops a placeholder.
- List every phrase with `ROOMTONE_PHRASES=phrases.txt cargo test dump -- --ignored`.
- **Add a language**: add `(code, name)` to `LANGUAGES` and an `include_str!` line to `TABLES_TSV` in `i18n.rs`, create `locales/<code>.tsv`, then run the tests.
- Text sent to assistants (MCP) and the command-line help stay English.

## Adding things

- **A new action**: add a `Cmd` variant, handle it in `run_common` (no speakers needed) or `run_live` (speakers) and in `demo::apply`, add an `optimistic` arm if the screen should react at once, and the `mirrored_group` / `retarget` arms if play-along groups should follow (they follow from either side, and `keep_links_in_step` brings drifted groups back every five seconds).
- **A new assistant tool**: add a row to `tools()` in `mcp.rs` (name, description, JSON schema, whether it changes things) and an arm in `Mcp::call`. Tools that change things are hidden and refused automatically when the person allows looking only.
- **A new streaming service**: add it to `sources::SERVICES` with its service number (`sid`). Sign-in, browse and search come from `smapi.rs`. Give it a `note` if it can only play favorites.
- **A new page**: add a `View` variant, a sidebar entry in `ui/mod.rs` and a function in `ui/pages.rs`.

## Tests

| Command | Checks |
| --- | --- |
| `cargo test` | 269 unit tests: protocol parsers (DIDL, topology, SOAP replies, update offers, streaming-service replies, podcast feeds, radio playlists), rules (volume scaling, play-along mirroring, account numbers, S1/S2 update guard), settings compatibility, translations, the demo system, and the assistant link end to end, including its HTTP security checks |
| `cargo test -- --ignored --nocapture` with `ROOMTONE_SPEAKER=<ip>` | Read-only checks against real speakers: discovery, sign-in pages for each service, account numbers |
| `cargo test service_deep -- --ignored --nocapture` | Read-only: home, one level down and a search on each connected service (uses your real settings) |
| `roomtone --mcp --demo` with JSON-RPC lines on stdin | The assistant tools against the demo system |
| `roomtone --screenshot shots/x.png` | A picture of the window (demo system), the same as the automatic checks take |
| `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` | Style and lints, as CI runs them |

When testing the assistant link, point `HOME` (on Windows, `APPDATA`) at a scratch folder so your real settings aren't changed. **Never play audio, regroup rooms or install updates on speakers you share with other people without asking them first**; the ignored network tests are read-only on purpose.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request, on macOS, Windows and Linux: build, `cargo test` and `cargo clippy -D warnings`, plus (on Linux) `cargo fmt --check` and `cargo doc`. Dependabot (`.github/dependabot.yml`) proposes dependency and Actions updates weekly.
