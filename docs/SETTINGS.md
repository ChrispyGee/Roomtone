# Settings guide

Every setting Roomtone has: what it does, where to change it in the app, what it's called in the settings file, its default, and how to change it without the window (command line or by editing the file).

- [Where settings live](#where-settings-live)
- [Settings page](#settings-page)
- [Music settings (outside the Settings page)](#music-settings-outside-the-settings-page)
- [Settings kept on the speakers](#settings-kept-on-the-speakers)
- [The settings file, field by field](#the-settings-file-field-by-field)
- [Command-line options and environment variables](#command-line-options-and-environment-variables)
- [Other files in the settings folder](#other-files-in-the-settings-folder)
- [Back up, move and reset](#back-up-move-and-reset)

## Where settings live

Roomtone keeps its own settings in one file, `config.json`, in the settings folder:

| Platform | Folder |
| --- | --- |
| macOS | `~/Library/Application Support/Roomtone/` |
| Windows | `%APPDATA%\Roomtone\` |
| Linux | `~/.config/roomtone/` |

Settings › About shows the folder for your computer, with a *Show* button that opens it. `roomtone --help` prints it too.

How the file behaves:

- **The app writes it for you.** Every switch and button in the app saves at once. You never have to edit the file.
- **You can edit it by hand** (it's plain JSON), but **close Roomtone first**. The window reads the file only when it starts, and saves its own copy whenever you change something, which would overwrite your edit. The one exception is `--headless` mode, which re-reads the assistant-link settings (`mcp`) every 2 seconds, so changes made in the window or with `--mcp-on` and friends reach it.
- **A file Roomtone can't read is kept, not overwritten.** If the JSON is broken (a missing comma, say), Roomtone moves it to one side as `config.json.broken-<number>` in the same folder, starts with the defaults, and says so at the top of the Settings page and in the terminal. Nothing you had is lost: fix that file (check it with any JSON validator) and rename it back to `config.json` with Roomtone closed, or copy across what you need. If the file can't even be moved, Roomtone starts with the defaults but doesn't save anything that run, so it can't overwrite your file. Programs that only *look* at the file (`--help`, a headless copy watching for changes) never move or change it.
- **Missing fields get their defaults** and unknown fields are dropped the next time the app saves, so the file stays readable by older and newer versions.
- **It holds secrets**: streaming-service sign-ins and the assistant access key. On macOS and Linux only your user can read it (mode 600). Don't post it or put it in a shared backup.

## Settings page

Open it from the sidebar. Sections, top to bottom:

### Systems and rooms
Each system (S1 and S2 are shown separately) lists its rooms with their model and speaker software version. Choose a room to open its own settings ([kept on the speakers](#settings-kept-on-the-speakers)).

### Software updates
*Check for updates* asks each system's own update service whether newer speaker software exists. *Update all speakers* asks for confirmation, then installs on every speaker of that system. S1 systems are only offered S1 updates; moving S1 to S2 isn't possible from the app. Nothing here is saved by Roomtone.

### Connection
| Control | What it does |
| --- | --- |
| *Search again* | Searches the network for speakers again. (*Reconnect*, in the sidebar under the systems list, retries your remembered speakers after a lost connection.) |
| *Add a speaker by address* | Use when your network blocks discovery: type a speaker's IP address (find it in your router) and choose *Add*. Remembered in `speakers.json`. |
| *Open demo* | Shows a made-up system (5 S2 rooms and 3 S1 rooms) so every screen can be tried. Nothing real is touched. |

### Appearance
*Light or dark*: **Automatic** (follows your computer, live), **Light** or **Dark**. Changes at once.
File: `theme` (`"auto"`, `"light"`, `"dark"`). Default `auto`. For one run only: `--theme`.

### Language
*App language*: **Automatic** (follows your computer's language), or English, German, French, Spanish, Italian or Dutch. Changes at once; anything not yet translated shows in English.
File: `language` (`"auto"` or `"en"`, `"de"`, `"fr"`, `"es"`, `"it"`, `"nl"`). Default `auto`. For one run only: `--lang`, or the `ROOMTONE_LANG` environment variable.

### Speaker updates (instant updates)
*Instant updates from the speakers*: when on, the speakers tell Roomtone when something changes (playing, volume, grouping), so changes made in other apps or on the speakers show up at once, and Roomtone asks them far less often. The status line says how many speakers are sending updates, or that some can't reach this computer.

- Needs your computer to accept connections from the speakers on your home network. If a firewall asks, allow Roomtone. If it can't get through, Roomtone quietly keeps asking the usual way, so nothing breaks.
- Uses a free port on this computer while it's on. Nothing leaves your home network.
- Works with real speakers only (not the demo system).

File: `speaker_events` (`true` / `false`). Default `false`.

### Assistant link (MCP)
Lets AI assistants that support MCP see and control your speakers. **Off until you switch it on.** Details and per-platform setup: [HEADLESS_AND_MCP.md](HEADLESS_AND_MCP.md).

| Setting in the app | File (`mcp.…`) | Default | What it does |
| --- | --- | --- | --- |
| *Let AI assistants use Roomtone* | `enabled` | off | The master switch. Off: assistants are refused. |
| *Assistants can change things* | `allow_control` | on | On: they can play, pause, change volume, edit queues and group rooms. Off: look only. |
| *Allow other computers* | `allow_network` | off | Off: only this computer can connect (`127.0.0.1`). On: assistants elsewhere in your home can connect, with the access key. |
| *Port* | `port` | 8790 | The port of the web address (1024–65535). |
| *Access key* / *New key* | `token` | random | The secret assistants send over the web address. *New key* replaces it; assistants using the old one need the new one. |

The command line can change these too, each saving and quitting: `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port <port>`, and `--mcp-info` to show them with the web address and key. *Copy setup*, *Copy web address* and *Copy access key* are conveniences and store nothing.

### Backup and restore
Saves what you've built up to one file, and puts it back, on this system or a new one. Not stored in `config.json`.

- **Save backup** writes one `.json` file with: the system's **Sonos favorites**, **playlists** (with their songs) and **alarms**, plus Roomtone's own **lists and settings** (music folders, stations, podcasts, saved groups, favorites kept in the app, which sources are on, language, theme, speaker updates). It **never includes** streaming sign-ins or the assistant access key, so it's safe in a normal backup.
- **Choose a backup** opens a backup file and shows what's in it, with a tick box for each part. **Restore** then only **adds**: anything already on the system (same favorite, same playlist name, same alarm) is skipped, and nothing existing is changed or removed. A summary says what was added, skipped and couldn't be restored.
- It works on the system you're looking at (S1 and S2 are separate: choose the system in the sidebar first). Alarms are matched to rooms **by name**, so a room with a different name on the new system is reported and skipped. Saved groups move to the system you restore on.
- **Moving between S1 and S2, or to new speakers:** favorites and playlists that point at your own music library or at radio stations come across. Items from a streaming service use that service's account number on your system, so you may need to be signed in to the same service on the new system (and re-add an item if its account differs).
- **One playlist at a time:** on the Playlists page, a playlist's ⋯ (or right-click) menu has *Export playlist…*, which saves just that playlist and its songs to its own file; *Import playlist* at the top of the page opens a file and takes you to the summary above, where *Restore* adds it (skipped if a playlist of that name already exists). A single-playlist file is an ordinary backup file with one playlist in it, so *Choose a backup* opens it too.
- **M3U playlists:** the same menu has *Export as M3U…*, which writes a standard extended M3U8 file (`.m3u8`, UTF-8) that most players can open: songs from this computer's music folders are written as their real file paths, songs on a network share as `smb://` addresses, and anything else as its web address. *Import playlist* also opens `.m3u` and `.m3u8` files: web addresses and shares are taken as they are, and file paths (absolute, relative to the playlist, or `file://`) are matched to songs in **your music folders** (Music Library › Add folder), also when the list was made on another computer, by the last folder and file name. Anything that can't be used (a file outside your music folders, a path on another computer's drive that matches nothing) is skipped and listed under *Details* after you restore. Songs from streaming services only mean something to speakers and are skipped by other players. Streaming (HLS) `.m3u8` files aren't song lists and are refused.
- **CSV playlists:** *Export as CSV…* writes a spreadsheet-friendly list (UTF-8 with a byte-order mark so Excel shows accents correctly) with the columns *Title, Artist, Album, Duration (seconds), Address*; addresses are written the same way as in M3U. Cells that start with `=`, `+`, `-` or `@` get a leading apostrophe so a spreadsheet won't run them as formulas, and *Import* takes it off again. *Import playlist* reads `.csv` files with comma, semicolon or tab separators and the column names other services use (*Track Name*, *Artist Name(s)*, *Album Name*, *Duration (ms)*, *Track URI*…). A row's address is used if Roomtone can use it; otherwise (for example a Spotify export, whose addresses only work in Spotify) the song is looked up **by title and artist in your music folders**. Rows that match nothing are skipped and listed under *Details*, so a CSV from a streaming service helps you rebuild a playlist from music you have, but it can't bring the streaming songs themselves across.
- With no speakers connected, only the Roomtone part is saved or restored.
- The file format is described below; it's plain JSON, so it can also be read by other tools.

### About
Version, platform, your systems, the language in use, the address your computer serves music from, the assistant link status, the settings folder (*Show*), what's new in this version, help commands, credits, and buttons for the project page, *Report a problem*, *Latest version* and this *Settings guide*. Nothing here is a setting.

## Music settings (outside the Settings page)

| Setting | Where in the app | File | Default |
| --- | --- | --- | --- |
| Which free music sources are on | Music Services: switch each source on or off | `services`: any of `"local"` (folders on this computer), `"radio"`, `"podcasts"`, `"archive"` (Internet Archive) | all four on |
| Music folders on this computer | Music Library › *Add folder* (and *Remove folder*) | `folders`: list of paths | none |
| Radio stations you added | Radio › *Add station* (or *Save to My stations* on a stream) | `stations`: `{ "name", "url", "genre" }` | none |
| Podcast shows you follow | a show's *Subscribe* button | `podcasts`: `{ "title", "author", "feed", "art" }` | none |
| Streaming-service sign-ins | Music Services › *Connect*, *Disconnect*, and the account number if a service needs it | `accounts`: `{ "sid", "name", "uri", "manifest", "household", "device_id", "token", "key", "sn" }` | none |
| Favorites kept in the app (albums, artists, folders, podcast shows, archive recordings the speakers can't store) | the star on those items | `favorites` | none |
| Saved groups of rooms ("Downstairs") | Home › *Groups* (add or edit), or *Save as group* after grouping rooms | `groups`: `{ "name", "household", "rooms" }` | none |

Edit `accounts` and `favorites` in the app, not by hand: they hold tokens and details copied from the services.

## Settings kept on the speakers

These live **on the speakers**, not in Roomtone, so they aren't in `config.json` and they follow the speaker if you use another app: room names, grouping, volume and mute, bass, treble, loudness, night sound and speech enhancement (where the model has them), the status light, alarms, the queue, Sonos favorites and playlists, sleep timer, crossfade and play mode. Change them in the room's page or on the Alarms, Queue and Playlists pages. (Backing these up is on the [roadmap](ROADMAP.md).)

## The settings file, field by field

Complete `config.json`, with every field at its default (the access key is made up):

```json
{
  "folders": [],
  "stations": [],
  "podcasts": [],
  "services": ["local", "radio", "podcasts", "archive"],
  "accounts": [],
  "favorites": [],
  "groups": [],
  "language": "auto",
  "mcp": {
    "enabled": false,
    "allow_control": true,
    "allow_network": false,
    "port": 8790,
    "token": "0123456789abcdef0123456789abcdef"
  },
  "theme": "auto",
  "speaker_events": false
}
```

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `folders` | list of text | `[]` | Music folders on this computer, as full paths. |
| `stations` | list | `[]` | Radio stations you added. Each: `name`, `url` (the stream address), optional `genre`. |
| `podcasts` | list | `[]` | Podcast shows. Each: `title`, `feed` (the feed address), optional `author` and `art`. |
| `services` | list of text | all four | Which free sources are switched on: `local`, `radio`, `podcasts`, `archive`. |
| `accounts` | list | `[]` | Streaming services you signed in to. Secrets; managed by the app. |
| `favorites` | list | `[]` | Favorites Roomtone keeps itself. Managed by the app. |
| `groups` | list | `[]` | Saved groups of rooms. Each: `name`, `household` (which system), `rooms` (room names). |
| `language` | text | `"auto"` | `auto`, or `en`, `de`, `fr`, `es`, `it`, `nl`. |
| `theme` | text | `"auto"` | `auto`, `light` or `dark`. |
| `speaker_events` | true/false | `false` | Instant updates from the speakers. |
| `mcp.enabled` | true/false | `false` | Assistant link on. |
| `mcp.allow_control` | true/false | `true` | Assistants may change things. |
| `mcp.allow_network` | true/false | `false` | Other computers may connect. |
| `mcp.port` | number | `8790` | Port of the assistant web address. |
| `mcp.token` | text | random | The assistant access key. Created on first run and kept. |

Roomtone has **no setting** for telemetry or analytics because it sends none.

## Command-line options and environment variables

For one run unless it says it saves. Full list: `roomtone --help`.

| Option | Effect |
| --- | --- |
| `--lang <code>` | Language for this run (overrides `language`). |
| `--theme <auto\|light\|dark>` | Appearance for this run (overrides `theme`). |
| `--demo` | Use the demo system instead of searching. |
| `--view <page>` | Open a page first: `home`, `search`, `favorites`, `playlists`, `library`, `radio`, `inputs`, `alarms`, `settings`, `nowplaying`, `services`. |
| `--headless` | No window: keep the speakers up to date and serve the assistant link over HTTP while it's on. |
| `--mcp` | No window: serve the assistant link on stdin/stdout (how most assistants start it). |
| `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port <port>` | **Save** the assistant-link settings and quit. |
| `--mcp-info` | Show the assistant-link settings, address and key. |
| `--screenshot <file.png>`, `--write-icon <file> [size]` | Tools for documentation and installers. |
| `--help`, `--help mcp` | Help, and how to connect an assistant on this computer. |

Environment: `ROOMTONE_LANG` and `ROOMTONE_THEME` (what `--lang` and `--theme` set); when the language is `auto`, `LC_ALL`, `LC_MESSAGES` and `LANG` decide. The settings folder follows your home folder (`HOME` on macOS and Linux, `APPDATA` on Windows).

## The backup file

A backup is one JSON file. Top-level fields (every one optional when reading, so older and newer files keep working):

| Field | Holds |
| --- | --- |
| `roomtone_backup` | The format number (1). How Roomtone recognises its own files; a file with a higher number is refused ("update Roomtone"). |
| `created`, `app_version` | When (seconds since 1970) and by which version. |
| `system` | `generation` ("S1" or "S2") and the `rooms` names it came from (for information). |
| `sonos_favorites` | The speakers' favorites: title, address, description, artwork and the speakers' own details (`meta`). |
| `sonos_playlists` | `{ "title", "tracks": [ … ] }` for each playlist, songs in order. |
| `alarms` | Each alarm with its `room` **name**, time, repeat, what plays, volume and duration. |
| `app` | `folders`, `stations`, `podcasts`, `services`, `favorites` (kept in the app), `groups` (`name`, `rooms`), `language`, `theme`, `speaker_events`. |

Edit it by hand only if you know what you're doing; a file that can't be read is refused with a message and nothing is changed.

## Other files in the settings folder

| File | What | Safe to delete? |
| --- | --- | --- |
| `speakers.json` | Speakers found last time, for a fast start. | Yes: Roomtone searches again. |
| `config.json.broken-…` | A settings file Roomtone couldn't read, kept for you. | Yes, once you've recovered what you need. |
| `songs.json` | Details of songs played from this computer. | Yes: rebuilt as needed. |

## Back up, move and reset

- **Favorites, playlists, alarms and Roomtone's lists:** use *Backup and restore* in Settings (above).
- **Back up or move the settings file to another computer:** with Roomtone closed, copy `config.json` into the same folder on the other computer. Folders and stations come across; folder paths may need changing if the other computer's drives differ; streaming sign-ins are tied to your speakers' system and may need *Connect* again. Treat the copy like a password file. This hasn't been tried between different platforms.
- **Keep sign-ins out of a shared backup:** remove the `accounts` list and `mcp.token` first.
- **Reset one setting:** delete its field (or set the default from the table above) with Roomtone closed.
- **Reset everything:** close Roomtone and move `config.json` away (or let a broken file be set aside for you). The next start uses the defaults (and makes a new access key).
- **Uninstalling** doesn't remove the settings folder.
