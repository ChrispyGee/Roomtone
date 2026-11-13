# Roomtone — specification (v0.5.3)

Project home: <https://github.com/ChrispyGee/Roomtone>

The source of truth for **what Roomtone does**. Code, tests and docs should agree with it, and a change in behaviour should change this file in the same pull request. How the code is organised is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); the speaker protocol is in [docs/PROTOCOL.md](docs/PROTOCOL.md); open work is in [docs/ROADMAP.md](docs/ROADMAP.md).

**Words used here**

| Word | Meaning |
| --- | --- |
| System | One household of speakers, either **S1** (legacy firmware 11.x, build numbers up to 57.x) or **S2** (firmware 12+, builds 58.x and later) |
| Room | One speaker, or a bonded set that acts as one |
| Group | Rooms of one system playing together; led by a *coordinator* room |
| Play along | The app's stand-in for grouping across systems (§4.1) |
| Assistant link | The built-in MCP server (§6) |
| Demo system | Made-up S1 and S2 systems used when no speakers answer |

## 1. Goals and non-goals

**Goals**
1. Control **S1 and S2 systems from one app**, each shown as its own system.
2. Offer what the current official (S2-generation) controller app does: rooms and grouping, queue, search, favorites, playlists, library, radio, music services, line-in/TV, alarms, sleep timer, sound settings and software updates.
3. A modern look of its own (not a copy of any existing app), in a **dark** and a **light** version, or **Automatic** to follow the computer.
4. Work **without a cloud account of its own**: everything goes straight to the speakers on the local network.
5. Run **without a window**, and let **AI assistants** control the speakers through **MCP**, switched off by default.
6. **Localised**: English, German, French, Spanish, Italian, Dutch.
7. One code base for **macOS, Windows and Linux**.

**Non-goals**
- Setting up new speakers, Wi-Fi set-up, Trueplay tuning, surround and sub set-up, or voice assistants. Use the speaker maker's app for those.
- Moving an S1 system to S2, or back.
- Signing in to services that only allow the speaker maker's own app (Apple Music, YouTube Music, SoundCloud).
- Any cloud service run for this app. There is no server, telemetry or analytics.

## 2. Platforms and start-up

- One program for macOS, Windows and Linux, built from the same code.
- On start, the speakers remembered from last time (`speakers.json`) appear at once and are checked directly. A network search (SSDP) then runs in the background and adds anything new.
- If no speakers answer, the **demo system** is shown (one made-up S2 system with 5 rooms and one S1 system with 3), so every screen can be tried. A banner says so and offers "Search again".
- Lost contact (after the computer wakes from sleep, or after three failed polls in a row) triggers an automatic reconnect. Settings also offers a manual **Reconnect**.
- Group state is polled about every second for the selected group, every 4 s for others and every 20 s for grouping (topology). A full background search runs every 2 minutes.
- **Speaker updates** (Settings; off by default): the app subscribes to each speaker's UPnP events (playback on each group's coordinator, volume on every room, room layout on one room per system) and a small listener on this computer receives the `NOTIFY` messages. A notice only says which speaker and what kind of thing changed; the app then reads that group (or the layout) with the same calls polling uses. While a speaker is subscribed and has been heard from, polling for it slows to a safety net (selected group every 10 s, others every 30 s, layout every 60 s); the playback position is counted on screen between answers. If a speaker doesn't send notices within 20 s of subscribing (a firewall, say) the app keeps polling it as before and Settings says so. Subscriptions are renewed at half their lifetime and cancelled when the setting is switched off or the app quits. Only `NOTIFY` messages from a subscribed speaker's address carrying that subscription's id are accepted.

## 3. Window layout

| Area | Contents |
| --- | --- |
| Sidebar | Home, Search, Favorites, Playlists, Music Library, Radio, Music Services, Line-In & TV, Alarms, Settings; then the **Systems** list (S1/S2, room count, how many are playing); connection status at the bottom (opens the speaker status list) |
| Page area | The selected page; opened items stack, with a back button |
| Right panel | **Rooms** tab: a card per group (art, track, play/pause, volume, group button, "Plays along with …"), then Group all, Pause all, Ungroup, and the **Groups** section of saved groups. **Queue** tab: the selected group's queue |
| Bottom bar | Art, title, artist, room and system; shuffle, previous, play/pause, next, repeat; position slider with times (or "Live" for streams); favorite star, queue, group, volume, open player |

- The space bar toggles play/pause when no text field has focus.
- Minimum window size is 1040 × 660; the default is 1320 × 840.
- **Appearance**: dark and light palettes share one design. *Automatic* follows the computer's light/dark setting and switches the moment it changes. The brand orange is a little deeper in light mode for contrast; album art and gradients are the same in both.
- The app has its own icon (an orange orb on a dark rounded square), drawn in code and used for the window, the Dock or taskbar, and the About box.
- **macOS application menu**: *About Roomtone*, *Services*, *Hide Roomtone*, *Hide Others*, *Show All*, *Quit Roomtone*, all in the chosen language. *About Roomtone* opens the standard About box with the name, version, platform and architecture, icon, a short description, credits and copyright.

## 4. Features

### 4.1 Rooms and groups
- Group volume, and each room's own volume and mute. Group volume moves members proportionally, as the speakers do.
- **Group editor**: switch rooms of the same system in or out of a group; *Select all*, *Clear*, *Save as group*.
- **Saved groups** (for example "Downstairs"): one click groups exactly those rooms, leaving other rooms alone. They can be edited or deleted. A "Grouped" chip shows when the live group matches.
- **Group all**, **Pause all**, **Ungroup** for the active system.
- **Play along across systems.** S1 and S2 speakers can't share a group: grouping is done by the speakers themselves, the two firmware generations can't sync with each other, and they are separate households. So the group editor can link a group to a group in another system:
  - The app starts the same stream, or the same queue at the same track and position (about 1 s ahead, to allow for start-up), on the other group.
  - It then repeats play, pause, next, previous, seek, play mode, play-item, queue jumps, clearing the queue and the sleep timer there. Volumes stay separate.
  - The two may drift up to a second or two apart.
  - One link per group, no chains. Rooms of the same system are refused with "group them instead".
  - Links last until the app quits; they aren't saved.

### 4.2 Playback
- Play, pause, next, previous, seek, shuffle, repeat (off/all/one), crossfade, sleep timer (15, 30, 45 min, 1 h, 2 h, or off).
- **Now Playing** page: large art, track details, the **sound** of what's playing as small labels (format such as FLAC or MP3, sample rate, bit depth, bitrate, and *Lossless*) wherever the speaker says (nothing is shown that isn't reported; it's read from details fetched with each poll, so no extra traffic), with **Technical details** (what the speaker sent, with addresses reduced to their kind) and *Copy details* for reports, artist link ("everything by this artist"), the group's room volumes, play mode, crossfade, sleep timer, upcoming queue.
- **Queue**: play from any position, move up or down, remove, clear, save as a playlist.
- Play options on anything: *Play now*, *Play next*, *Add to end of queue*, *Replace queue*.

### 4.3 Music sources
| Source | What it offers | Needs |
| --- | --- | --- |
| System library | Artists, album artists, albums, songs, genres, composers, imported playlists, shared folders | Music shares set up on the system |
| This computer | Folders on this computer or mounted drives, scanned for audio. A built-in web server serves them to the speakers, so S1 and S2 can both play them while the app runs | Folders added in Music Library |
| Favorites | The system's favorites, plus albums, artists, folders, podcast shows and archive recordings the speakers can't store, kept in the app instead | — |
| Playlists | Playlists saved on the system | — |
| Radio | *My stations* (any stream address; .pls/.m3u links followed), the free Radio Browser directory (popular, by tag, search) | Internet |
| Podcasts | Search the public Apple Podcasts directory, subscribe, play episodes from each show's feed | Internet |
| Internet Archive | Featured collections, search, play whole recordings | Internet |
| Streaming services | Browse, search and play through the speakers' own music-service connection | Sign-in (§4.4) |

On S1 systems, `https` addresses are relayed through this computer as plain `http`, because S1 speakers can't play some secure streams.

### 4.4 Streaming services
Sign-in happens on the service's own web page, with a code when the service uses one. Status as tested on 2026-11-11 against a real S2 system:

| Service | In this app |
| --- | --- |
| Spotify, Amazon Music, Deezer, TIDAL, Qobuz | Sign in, then browse, search and play. Spotify browsing and search confirmed |
| Apple Music, YouTube Music, SoundCloud | **Favorites only.** The services refuse third-party sign-in (Apple sends back an empty encrypted link, YouTube a 403 for unregistered callers, SoundCloud `Client.NOT_AUTHORIZED`). Favorites saved from them in the speaker maker's app show under "Your favorites" on the service card and as a card in Music Library. They're labelled with their service, can be found in Search (also by the service's name) and play here |

- Music plays with the account already added to the system in the speaker maker's app. The account number (`sn`) comes from what's playing, then from favorites, the queue and playlists. If the speakers refuse a song, accounts 1–6 are tried and the one that works is saved. If playback stops within 5 seconds, a note suggests another account number. The number can also be set by hand on the service card.
- **Check** (each connected service): tries browsing, a search and finding the account, without playing anything, and shows the result in green or red.
- The Music Services page shows each service as *Connected*, *Ready to connect*, *Favorites only* or *Not offered on this system*.

### 4.5 Search
One box searches every source in parallel: library, favorites (by title, artist, album, description or service name), playlists, this computer, My stations, the radio directory, podcasts, the Internet Archive and each connected service. Results are grouped by type (artists, albums, songs, playlists, stations, podcasts, episodes, folders). There are source and type filters, a genre filter and sort orders. A source that fails says so; the others still show.

### 4.6 Line-In & TV
Plays a speaker's line-in or TV input in the selected room. Which speakers have them is guessed from the model name. A speaker without the socket refuses.

### 4.7 Alarms
Create, edit, enable/disable and delete alarms. Each has a time, how often it repeats (every day, weekdays, weekends, once and so on), a room, what to play (the chime or any favorite), a volume and a duration.

### 4.8 Settings
- **Systems and rooms** (model, version, address). A room opens its **room settings**: rename, volume, bass and treble (−10…+10), loudness, night sound and speech enhancement (TV speakers), status light, and details.
- **Software updates**, for each system:
  - *Check for updates* asks the speakers' own update service.
  - When an update is offered, *Update all speakers* asks for confirmation (speakers restart and music stops for a few minutes), then tells every speaker of the system to install it, one after another, showing progress.
  - Updates are only offered and installed for the same generation: an S1 system is never moved to S2. The generation is checked again just before installing.
  - An offer matching the speaker's current build counts as up to date.
- **Connection**: search again, add a speaker by IP address, open the demo.
- **Appearance**: *Automatic* (follows the computer's light/dark setting), *Light* or *Dark*. It changes at once.
- **Language**: *Automatic* (follows the computer) or a fixed language. It changes at once.
- **Speaker updates**: a switch for *Instant updates from the speakers* (§2) with a status line (how many speakers are sending updates, or a hint if they can't reach this computer).
- **Backup and restore**: *Save backup* writes one JSON file (`roomtone_backup` format 1) with the active system's favorites, playlists (with songs) and alarms, and Roomtone's own folders, stations, podcasts, saved groups, kept favorites, sources, language, theme and speaker-updates setting. Never sign-ins or the access key. *Choose a backup* shows what's inside with a tick box per part; *Restore* only adds (duplicates skipped by favorite title+address, playlist name, alarm room+time+repeat+sound, station address, podcast feed, folder path, group name), maps alarms to rooms by name, moves saved groups to the active system, and reports added / skipped / failed. Works on the demo system (favorites, playlists and alarms there). **One playlist**: *Export playlist…* in a playlist's menu (Playlists page) saves just it, with its songs, as a one-playlist backup file; *Export as M3U…* writes an extended M3U8 (real paths for this computer's songs, `smb://` for shares, addresses otherwise); *Import playlist* on that page opens an M3U/M3U8 or backup file and shows the restore summary in Settings (M3U file paths are matched to songs in the music folders, by exact file or by last folder and name; entries that can't be matched are skipped and reported). *Export as CSV…* writes `Title, Artist, Album, Duration (seconds), Address` (UTF-8 with BOM; spreadsheet-formula guard); import reads CSV with comma/semicolon/tab separators and other services' column names, uses a row's address if usable, otherwise matches title and artist against the music folders, and reports rows it can't place. With no speakers, only Roomtone's own part is saved or restored. Backup files from a newer format are refused.
- **Assistant link (MCP)**: §6.
- Every setting, its place in the app, its `config.json` name, defaults, options and back-up advice is written up in [docs/SETTINGS.md](docs/SETTINGS.md); the About section and `--help` link to it (*Settings guide*). Keep that file in step with this section.
- **About**:
  - version and platform; each system with its room count and speaker software versions;
  - the language in use, the address this computer serves music from, and the assistant link status;
  - the settings folder, with *Show* to open it in the file browser;
  - what's new in this version, help commands to copy, and buttons for the project page, *Settings guide*, *Report a problem* and *Latest version* (on GitHub);
  - credits (Radio Browser, the Apple Podcasts directory search, the Internet Archive, the open-source libraries) and a note that the app is independent of the speaker maker and the music services.

### 4.9 Languages
- English, German, French, Spanish, Italian and Dutch.
- Every phrase is looked up by its English text; anything missing falls back to English, and a test fails the build if a phrase is untranslated.
- *Automatic* reads `ROOMTONE_LANG`, `LC_ALL`, `LC_MESSAGES` and `LANG`, and on macOS the system's language list. Windows uses English unless one of those is set.
- Sample content in the demo system stays English. Text for assistants (MCP replies) and command-line help are English on purpose.

## 5. Running without a window

| Command | Behaviour |
| --- | --- |
| `roomtone --headless` | Finds the speakers, keeps them polled and serves the assistant link over HTTP while it's switched on. Prints a status line on stderr when anything changes. Runs until stopped (Ctrl+C or the service manager) |
| `roomtone --mcp` | Serves the assistant link on stdin/stdout (newline-delimited JSON-RPC). Waits up to 6 s for speakers before answering. Logs go to stderr. Ends when stdin closes |
| `--demo` with either | Uses the demo system |

Both modes re-read the settings file every 2 s, so switching the link off in the window (another process) takes effect straight away. Platform steps are in [docs/HEADLESS_AND_MCP.md](docs/HEADLESS_AND_MCP.md) and in `roomtone --help mcp`.

## 6. Assistant link (MCP)

### 6.1 Settings
| Setting | Default | Meaning |
| --- | --- | --- |
| Let AI assistants use Roomtone | **off** | When off, every request is refused (stdio: error −32001) and the HTTP port is closed |
| Assistants can change things | on | Off: only the look-only tools are listed; the others are refused |
| Allow other computers | off | Off: listen on 127.0.0.1 only and accept only local `Host` names. On: listen on all interfaces (Settings only, not on the command line) |
| Port | 8790 | HTTP port |
| Access key | random 32 hex characters | Required as `Authorization: Bearer <key>` on HTTP. *New key* replaces it |

Command line, each one saves and quits: `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port <port>`. `--mcp-info` prints the address and key. Settings also offers *Copy setup* (a ready-made `mcpServers` block), *Copy web address*, *Copy access key*, and a collapsible **How to connect on <platform>** with copyable commands.

### 6.2 Protocol
- JSON-RPC 2.0 with MCP protocol versions `2025-06-18`, `2025-03-26` and `2024-11-05` (the client's choice if supported, otherwise the newest).
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, `resources/read`, `prompts/list`, `prompts/get` and `completion/complete`. Notifications get no reply. Batches are accepted. Capabilities: tools, resources, prompts and completions (none of them send change notifications).
- HTTP (Streamable HTTP, JSON responses only):
  - `POST /mcp` only; `GET` and `DELETE` return 405.
  - Requires `Content-Type: application/json` (otherwise 415) and a valid key (otherwise 401).
  - Refuses an `Origin` other than localhost (403), and with *Allow other computers* off, a non-local `Host` (403).
  - Bodies up to 1 MB. A notification-only request returns 202.

### 6.3 Tools
Rooms are given by name (exact, or a unique part of a name) or id. Leaving the room out means the room selected in the app. Look-only tools are marked *(read)*; the others are hidden and refused in look-only mode.

| Tool | Does |
| --- | --- |
| `get_status` *(read)* | Every system, room, group, what's playing, volumes, play mode, play-along links and saved groups (JSON) |
| `now_playing` *(read)* | One room's group: track, position, state, volume, play mode, sleep timer |
| `playback` | `play`, `pause`, `toggle`, `next`, `previous` |
| `set_volume` | Absolute `volume` 0–100 or relative `change`; `whole_group` for the group |
| `set_mute` | Mute or unmute a room or its group |
| `set_play_mode` | `shuffle`, `repeat` (off/all/one), `crossfade` |
| `seek` | Jump to `seconds` |
| `sleep_timer` | Stop after `minutes` (0 = off) |
| `list_favorites` *(read)* | Favorites with refs (`fav1`…) |
| `list_playlists` *(read)* | Playlists with refs (`pl1`…) |
| `search` *(read)* | Everything search covers, with refs (`r1`…), up to `limit` |
| `play` | A `ref`, or a favorite/playlist `name`; `how`: now, next, end, replace |
| `get_queue` *(read)* | The queue numbered from 1, the current song marked ▶ |
| `play_from_queue`, `remove_from_queue`, `clear_queue` | Queue control by position |
| `group_rooms`, `ungroup_room`, `group_all`, `use_saved_group` | Grouping within one system |
| `play_along`, `stop_play_along` | Play along across systems (§4.1) |
| `switch_input` | Line-in or TV input, optionally from another room |
| `get_sound` *(read)*, `set_sound` | Bass, treble, loudness, night sound, speech enhancement, status light |
| `list_alarms` *(read)*, `set_alarm_enabled` | Alarms |
| `check_for_updates` *(read)* | Whether each system has newer software (same generation only) |
| `install_update` | Installs on every speaker of a room's system; refused unless `confirm: true` |
| `browse` *(read)* | Step through the sources: without a ref the top level (favorites, playlists, library, this computer, radio, podcasts, Internet Archive, services); with a ref, what's inside (refs `b1`…) |
| `list_recent` *(read)* | Recently played from this app (refs `rec1`…) |
| `add_favorite`, `remove_favorite` | Save a ref or what's playing (`current: true`); remove by ref from `list_favorites` |
| `save_queue_as_playlist`, `move_in_queue` | Queue editing |
| `pause_all` | Pause every playing group (or one system's) |
| `rename_room` | New room name (up to 40 characters) |
| `create_alarm`, `delete_alarm` | `time` "HH:MM", `days` (daily, weekdays, weekends, once, or day names), `volume`, `minutes`, optional `ref` to play (otherwise the chime) |
| `save_group`, `delete_saved_group` | Named groups for `use_saved_group` (rooms of one system) |
| `list_services` *(read)*, `check_service` *(read)* | Each service's state; an end-to-end check of a connected one without playing anything |
| `find_speakers` *(read)* | Search the network again |
| `add_station`, `remove_station` | My stations |
| `subscribe_podcast`, `unsubscribe_podcast` | Podcast shows (subscribe by a ref from `search`) |

### 6.4 Resources, prompts and completions

| Resource | Contents |
| --- | --- |
| `roomtone://status` | The same JSON as `get_status` |
| `roomtone://now-playing` | What every group is playing (JSON) |
| `roomtone://favorites`, `roomtone://alarms` | As `list_favorites` / `list_alarms` |
| `roomtone://queue/{room}` (template) | A room's queue; the room name is URL-encoded |

Prompts (ready-made instructions an assistant can offer the person): `play_for_mood` (mood, room), `bedtime` (room, minutes), `whole_home` (what), `morning_alarm` (room, time). `completion/complete` suggests room names, saved-group names, alarm days and moods while an argument is typed.

Each changing tool waits until the speakers have answered (up to 20 s). It then returns a one-line summary, or the speakers' error with `isError: true`. Refs last until the next list of the same kind.

## 7. Command-line options

| Option | Effect |
| --- | --- |
| *(none)* | Open the window |
| `--demo` | Demo system |
| `--lang <code>` | Language for this run |
| `--theme <mode>` | Appearance for this run: `auto`, `light` or `dark` |
| `--view <page>` | Open a page first: home, search, favorites, playlists, library, radio, inputs, alarms, settings, nowplaying, services |
| `--screenshot <file.png>` | Draw about 120 frames, save the window as PNG and quit. Uses the demo system, opens without taking focus (and behind other windows on Windows) and doesn't open the assistant port |
| `--headless`, `--mcp` | §5 |
| `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port <n>`, `--mcp-info` | §6.1 |
| `--help`, `--help mcp` | Options; the assistant guide for this platform |

Test-only environment variables: `ROOMTONE_LIVE` (screenshots with real speakers), `ROOMTONE_SHOT_FRAME`, `ROOMTONE_SCROLL`, `ROOMTONE_TEST_GROUP`, `ROOMTONE_TEST_RECONNECT`, `ROOMTONE_TEST_EDITOR`, `ROOMTONE_TEST_ABOUT` (macOS: also saves a picture of the About box), and `ROOMTONE_SPEAKER` / `ROOMTONE_TERM` for the ignored network tests.

## 8. Stored files

All in the app's data folder: `~/Library/Application Support/Roomtone/` (macOS), `%APPDATA%\Roomtone\` (Windows) or `~/.config/roomtone/` (Linux). Unknown fields are ignored and missing fields take defaults, so older files keep working. **Add new fields with `#[serde(default)]`.**

| File | Contents |
| --- | --- |
| `config.json` (mode 600 on Unix) | `folders` (paths), `stations` [{`name`, `url`, `genre`}], `podcasts` [{`title`, `author`, `feed`, `art`}], `services` (switched-on free sources: `local`, `radio`, `podcasts`, `archive`), `accounts` [{`sid`, `name`, `uri`, `manifest`, `household`, `device_id`, `token`, `key`, `sn`}], `favorites` (items kept in the app), `groups` [{`name`, `household`, `rooms`}], `language` (`auto` or a code), `theme` (`auto`, `light` or `dark`), `speaker_events` (`true` or `false`), `mcp` {`enabled`, `allow_control`, `allow_network`, `port`, `token`} |
| `speakers.json` | Systems from last time: [{`id`, `s1`, `rooms` [{`uuid`, `name`, `ip`, `model`, `version`}], `groups` [{`coordinator`, `members`}]}] |
| `songs.json` | `songs`: details (title, artist, album, art) by address, for songs the speakers know only by address; `relays`: relay codes for S1 and the https address each stands for |

## 9. Network, security and privacy

| Port | Direction | Use |
| --- | --- | --- |
| UDP 1900 (multicast 239.255.255.250) | out/in | Finding speakers (SSDP) |
| TCP 1400 on each speaker | out | Control (UPnP/SOAP), artwork, device details |
| TCP 52880 (or a free port) on this computer | in | Speakers fetch music from this computer and S1 https relays |
| A free TCP port on this computer (only while *Speaker updates* is on) | in | Speakers send change notices (UPnP events); anything not from a subscribed speaker with its subscription id is refused |
| TCP 8790 (configurable) | in | Assistant link over HTTP, 127.0.0.1 unless other computers are allowed |
| HTTPS out | out | Radio Browser, Apple Podcasts search and feeds, Internet Archive, streaming services' own APIs |

- No telemetry. Nothing is sent anywhere except the speakers, the music sources listed above, and the services the person signs in to.
- Sign-in tokens and the access key live only in `config.json`, readable only by the user.
- A settings file that can't be read is never overwritten: at start-up it is renamed `config.json.broken-<n>`, the defaults are used, and Settings shows a notice with the new name. If it can't be renamed, saving is switched off for that run. Read-only callers (`--help`, the headless copy's change watcher, streaming-service lookups) never rename or write it.
- Actions that can't be undone are never done without the person's confirmation: software updates; never moving S1 to S2.

## 10. Verification status

| Area | Verified how |
| --- | --- |
| Discovery, live status, browsing, favorites on real **S2** speakers | Real speakers (2026-11-11) |
| Discovery and control on a real **S1** speaker | Reported working by the maintainer in an earlier version |
| Streaming sign-in pages (8 services), Spotify browse and search | Real services through an S2 system |
| Streaming **playback** and the account-number retry | Not yet heard on real speakers |
| Play along between real S1 and S2 speakers | Demo system only |
| Software updates (check and install) | Demo system only |
| Backup: saving favorites, playlists and alarms | One real S1 system, read only (counts and round trip); demo system for everything else |
| Backup: **restoring** favorites, playlists and alarms | Pretend system in unit tests and the demo system; **not yet tried on real speakers** (the playlist calls `CreateSavedQueue`/`AddURIToSavedQueue` are unverified) |
| Sound quality labels | Unit tests on made-up and real-S1-library descriptions (format from the MIME type is what a real S1 system sends for library tracks); **sample rate, bit depth and bitrate from real speakers not yet seen** (nothing was playing when checked) |
| Speaker updates (UPnP events) | Unit tests against a pretend speaker (subscribe, notice, renew, cancel, refusals); tried once with a real S1 speaker (Connect:Amp, firmware 11.16): all three subscriptions accepted, a first notice for each within about 5 s, a 1800 s lease, renewing and cancelling work; no S2 or multi-room tests yet |
| Assistant link: every tool, on/off, look-only, key, origin check | Demo system, stdio and HTTP |
| Translations | Automated completeness test; not reviewed by native speakers |
| Logic without speakers (parsers, rules, settings, demo system, assistant link incl. HTTP security) | 103 unit tests (`cargo test`), run on every platform in CI |
| Light mode | Screenshots of every page in the demo system |
| Windows and Linux builds | Continuous integration (`.github/workflows/ci.yml`); not run by hand |

## 11. Acceptance checks

Quick checks that a change hasn't broken behaviour. Each needs no real speakers unless marked.

1. `cargo test` passes: 103 unit tests, including that every phrase is translated and placeholders are kept.
2. `roomtone --screenshot shots/x.png` saves a non-blank picture of the demo system within a few seconds.
3. `roomtone --mcp --demo` with an `initialize` line, then `tools/list`, lists 48 tools when control is allowed and 14 when look-only.
4. With the link off, `tools/call` answers error −32001.
5. Over HTTP, a request without the key gets 401, one with `Origin: https://example.com` gets 403, and `--mcp-off` closes the port within ~2 s.
6. In the demo: `play_along` from "Living Room" to "Office", then `playback next` on Living Room, changes Office's track too.
7. In the demo: `check_for_updates` offers an S2 update and none for S1. `install_update` without `confirm` is refused; with it, the update is installed.
8. *(real speakers)* Settings › Music Services › *Check* on a connected service shows browsing and search working.
9. `roomtone --theme light --screenshot shots/light.png` draws the light palette (and `--theme dark` matches the earlier dark pictures).
