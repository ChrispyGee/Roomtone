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

Open it from the sidebar. It is a row of seven tabs with one shown at a time, in this order: **About**, **Playback**, **Services**, **Speakers**, **Appearance**, **Assistants** and **Backups**. **About** is selected when the app starts. Where each setting below lives: *Playback*: listening history, lyrics; *Services*: the free sources and streaming-service sign-ins; *Speakers*: systems and rooms, connection, software updates, instant updates; *Appearance*: light or dark, *Pages you use*, language; *About*: the about card and new versions of Roomtone. The sections, in this guide's order:

### Arranging the rooms
In the Rooms panel (right-hand side), *Arrange* shows up and down arrows on every room card; use them to put the rooms in the order you like, then choose *Done*. The order is remembered on this computer for each system (saved as `room_order`). A group of rooms stands where its earliest-listed room is, and new rooms appear after the ones you've arranged. Nothing changes on the speakers.

### Systems and rooms
Each system (S1 and S2 are shown separately) lists its rooms with their model and speaker software version. Choose a room to open its own settings ([kept on the speakers](#settings-kept-on-the-speakers)).

### Pages you use
*Show Alarms* and *Show Scenes* add those pages to the sidebar (Scenes also adds its shortcuts to Home). Both are **off by default** so the app stays simple if you don't use them; someone who already had scenes keeps seeing them until they switch the page off. Fields: `show_alarms`, `show_scenes` (true/false; absent means "not chosen yet"). The speakers' own alarms keep working whether or not the page is shown. Alarms is tagged *Experimental* (little testing on real speakers). If the system has no time zone set (a new speaker not fully set up), the page says its alarms won't ring and to set the time zone in the Sonos app.

### Software updates (experimental)
*Allow installing updates* (`allow_update_install`, off by default) decides whether the *Update all speakers* button is offered. Installing hasn't been tried on real speakers yet, so it's off until you switch it on; checking is always possible.
*Check for updates* asks each system's own update service whether newer speaker software exists. *Update all speakers* asks for confirmation, then installs on every speaker of that system and shows how many speakers are on the new software (speakers that aren't answering are counted as restarting) until all are, or says so after 45 minutes if they aren't. S1 systems are only offered S1 updates; moving S1 to S2 isn't possible from the app. Nothing here is saved by Roomtone.

### Can this system move to S2?
Under an S1 system in *Software updates*, *Check my speakers* asks each speaker which model it is (read-only: nothing is changed) and lists every room with its model, model number and hardware version, and one of:
- **Already on S2**: it runs S2 software.
- **Can run S2**: its model isn't on Sonos's list of products that can't.
- **Can't run S2**: Zone Players, controllers and the Bridge (model numbers `ZP80`, `ZP100`, `CR100`, `CR200`, `BR100`).
- **Check**: the Connect (`ZP90`), Connect:Amp (`ZP120`) and Play:5 (`S5`) came in a first generation that can't run S2 and a second that can, **with the same model number**, so Roomtone doesn't guess; look your product and its generation up in Sonos's S2 compatibility list (the label under the speaker shows the model and a date).
A summary line says how many speakers can't run S2 (a system with such speakers would have to keep them on S1 as a separate system) or need checking. Roomtone's list is short on purpose, so a model it doesn't know shows as *Can run S2*: treat that as "not known to be a problem".

### Connection
| Control | What it does |
| --- | --- |
| *Search again* | Searches the network for speakers again. (*Reconnect*, in the sidebar under the systems list, retries your remembered speakers after a lost connection.) |
| *Forget* (beside a system marked *Not on this network*) | Takes a remembered system that isn't answering off the list, after a confirmation. Not offered for the only system or one that answers. Searching for speakers adds it back if it's on this network. |
| *Add a speaker by address* | Use when your network blocks discovery: type a speaker's IP address (find it in your router) and choose *Add*. Remembered in `speakers.json`. |
| *Open demo* | Shows a made-up system (5 S2 rooms and 3 S1 rooms) so every screen can be tried. Nothing real is touched. |

### Appearance
*Light or dark*: **Automatic** (follows your computer, live), **Light** or **Dark**. Changes at once.
File: `theme` (`"auto"`, `"light"`, `"dark"`). Default `auto`. For one run only: `--theme`.
*High contrast* (**off by default**): stronger text, outlines on every control, brighter accent and status colours and stronger slider tracks, in both light and dark. Text and the dim text reach at least 7:1 against the background (WCAG AAA); the faint text and accent colours at least 4.5:1. The usual palettes keep text at 4.5:1 or better but use fainter wording for secondary notes (about 3:1) and a deeper orange that is about 3:1 on light backgrounds, which is why this switch exists. Changes at once.
File: `high_contrast` (`true` / `false`), default `false`. For one run only: the environment variable `ROOMTONE_CONTRAST=1`.

*Show in the menu bar* on a Mac, or *Show in the notification area* on Windows (**off by default**; not yet on Linux; the Windows icon was tried on a Windows 11 virtual machine and works, 2026-11-23): a speaker icon at the top of the screen. Its menu shows what's playing in the chosen room, *Play*/*Pause*, *Next*, *Previous*, *Volume* (*Louder* and *Quieter* by the volume step, *Mute*), the rooms to choose from (the ticked one is the room it controls, the same room as in the window), *Pause all rooms*, *Open Roomtone* and *Quit Roomtone*. The menu is built each time it opens. *Keep running when the window is closed* (on by default, shown when the menu bar is on): closing the window hides Roomtone instead of quitting, so the speakers, alarm fades and the assistant link keep working; *Open Roomtone* brings it back, and ⌘Q or *Quit Roomtone* quits.
*Show the room name next to the icon* (Mac only, off by default, shown when the menu bar is on): the name of the playing room beside the icon, kept current every couple of seconds; the icon is alone when nothing plays.
File: `menu_bar` (`true` / `false`), default `false`; `menu_bar_keep` (`true` / `false`), default `true`; `menu_bar_title` (`true` / `false`), default `false`.

### Language
*App language*: **Automatic** (follows your computer's language), or English, German, French, Spanish, Italian or Dutch. Changes at once; anything not yet translated shows in English.
File: `language` (`"auto"` or `"en"`, `"de"`, `"fr"`, `"es"`, `"it"`, `"nl"`). Default `auto`. For one run only: `--lang`, or the `ROOMTONE_LANG` environment variable.

### Speaker updates (instant updates)
*Instant updates from the speakers*: when on, the speakers tell Roomtone when something changes (playing, volume, grouping), so changes made in other apps or on the speakers show up at once, and Roomtone asks them far less often. The status line says how many speakers are sending updates, or that some can't reach this computer.

- Needs your computer to accept connections from the speakers on your home network. If a firewall asks, allow Roomtone. If it can't get through, Roomtone quietly keeps asking the usual way, so nothing breaks.
- Uses a free port on this computer while it's on. Nothing leaves your home network.
- Works with real speakers only (not the demo system).

File: `speaker_events` (`true` / `false`). Default `false`.

### New versions
*Tell me when a new version of Roomtone is out*: when on, a few seconds after Roomtone starts and then about once a day it reads this project's latest release on GitHub (one plain request to `api.github.com`; the only thing that identifies the app is its normal `User-Agent` with the version number, and nothing about you or your speakers is sent). If a newer version is out, this section says so and *Open download page* takes you there; there's also a one-time pop-up. *Download* (not shown for a Homebrew install, and not when the release has no installer for this computer) fetches the installer for this computer from this project's release page into your Downloads folder (or the temporary folder if there is none), checks it against the release's `SHA256SUMS` and then offers *Open* and *Show in folder*; a file that doesn't match is deleted. Nothing is downloaded until you press it, and nothing is installed for you. *Check now* looks straight away, whether or not the switch is on. A look that fails (offline, rate limit) is tried again after 5 minutes, then 15, 45, about 2¼ hours and never less often than every 6 hours; *Check now* always looks at once. If Roomtone was installed with Homebrew (its `Caskroom/roomtone` folder exists), the section also shows `brew upgrade --cask roomtone` with a *Copy* button. The check runs while the app is open; a computer that was asleep looks within about 20 seconds of waking when a day has passed. A fresh start always looks once (when the switch is on), so a newer version found earlier isn't forgotten on restart.
File: `check_new_versions` (`true` / `false`), default `false`; `last_version_check` (seconds since 1970) is kept by the app so it only looks once a day while the app stays open (a start always looks once).
### Lyrics (experimental)
*Show lyrics for the song that's playing* (**off by default**, tagged *Experimental*): a *Lyrics* section on the Now Playing page. While the page is open, Roomtone sends the song's **title, artist, album and length** to [lrclib.net](https://lrclib.net), a free lyrics site run by volunteers (one or two plain requests per new song; no account, nothing else is sent). Lyrics with times highlight the current line and follow the song; others are shown as plain text. Nothing is saved: lyrics are kept in memory until you quit. Live radio isn't looked up. Lyrics are shared by the site's community, so some songs have none and some are wrong. This is a trial feature and may change or be removed.
File: `show_lyrics` (`true` / `false`), default `false`.
### Volume
*Volume keys move the volume by* (**5 by default**; choose 1, 2, 5 or 10): how far Ctrl (Cmd on a Mac) with the up and down arrow keys moves the volume of the selected room or group. A smaller step suits quiet listening. A slider that has the keyboard focus still moves by 1 with the arrow keys and by 10 with Shift.
File: `volume_step` (`1`, `2`, `5` or `10`), default `5`; any other number is read as `5`.
### Now-playing display (experimental)
*Show what's playing on a picture frame or tablet* (**off by default**, tagged *Experimental*; tried only with the demo system and a pretend speaker, not yet on a real frame): gives other screens on your home network a web address that shows the cover and song that's playing. It is **read-only** (nothing can be played or changed through it) and works while Roomtone is running, in the window or with `--headless`. The address looks like `http://<this computer>:8791/<secret>/`; the secret is made for you, and *New address* replaces it (screens using the old one stop working). Anyone on your network who has the address can see what you're playing. *Room to show* picks a room, or follows the room you're using. *Copy address* and *Open here* help you set it up.
Add `?text=0` to the address to hide the song's name, `?fit=cover` to fill the screen, `?size=1200` to ask for a particular cover size. The same address ending in `art.jpg` is just the cover (a JPEG) and ending in `now.json` is a description (`playing`, `title`, `artist`, `album`, `room`, `has_art`, `id`), for frames, Home Assistant and scripts. Details: SPEC §4.8a.
File: `frame` (see the table below). To use it with `--headless` and no window, set `"enabled": true` in the file.
### Volume limits
*Settings › Playback › Volume limits*: a highest volume for each room, and quiet hours. **Roomtone applies them to what it sends** (this window and the keyboard, the command line, assistants, scenes, timers and new or saved alarms). The speakers can't hold a limit themselves, so **another app, or the buttons on a speaker, can still go higher**; the *Turn a room back down if it goes over* switch (off by default) makes Roomtone lower such a room every few seconds, but only while Roomtone is open.
- *Loudest each room may be*: drag or type 0 to 99 for a room, or leave it at *No limit* (100). A group stops where its loudest room reaches its limit. A room already over its limit isn't raised further, but can be lowered.
- *Quiet hours*: from, to, and *no louder than*. Every room is held to the lower of its own limit and this. It uses the speakers' clock (the home's local time). Alarms and timers are held to the room's limit, not to quiet hours.
File: `volume_limits` with `rooms` (a list of `{ "room": <room id>, "max": 0-99 }`), `quiet` (`enabled`, `from` and `to` in minutes after midnight, `max`) and `keep_down` (true/false). Room ids are the speakers' own (see `speakers.json`); the window is the easy way to set them.
### Listening history
*Remember what I've played* (**on by default**): songs and stations that have played for about 20 seconds are kept on this computer in `history.json` (the newest 500, each with how often and when last), and **never sent anywhere**. They feed *Recently played* on Home (which now survives a restart and includes what was played from other apps) and the **History** page (Home › *History*, or *See history* here), where you can play any of them again. Skipping through songs counts none of them; demo-system songs are never kept. Switching it off stops adding to the history but keeps what's there; *Clear history* forgets everything (the file is emptied).
File: `remember_history` (`true` / `false`), default `true`.

### Assistant link (MCP)
Lets AI assistants that support MCP see and control your speakers. **Off until you switch it on.** Details and per-platform setup: [HEADLESS_AND_MCP.md](HEADLESS_AND_MCP.md).

| Setting in the app | File (`mcp.…`) | Default | What it does |
| --- | --- | --- | --- |
| *Let AI assistants use Roomtone* | `enabled` | off | The master switch. Off: assistants are refused. |
| *Assistants can change things* | `allow_control` | on | On: they can play, pause, change volume, edit queues and group rooms. Off: look only. |
| *Saved items* | `allow_saved` | on | Within *change things*: favorites, stations, podcasts, saved groups, scenes, saving the queue as a playlist. |
| *Alarms* | `allow_alarms` | on | Make, delete, turn on and off alarms. |
| *Room names and sound* | `allow_names` | on | Rename rooms; change bass, treble and other sound settings (including a room's sub, surround and lip-sync settings); blink a speaker's light to find it. |
| *Speaker updates* | `allow_updates` | **off** | Install new speaker software (they restart). Always possible in the app. |
| *Save files* and *Choose folder* | `allow_files`, `files_folder` | **off**, none | Assistants may save backups and playlist exports, only into the folder you choose. A folder is needed before this does anything. |
| *Allow other computers* | `allow_network` | off | Off: only this computer can connect (`127.0.0.1`). On: assistants elsewhere in your home can connect, with the access key. |
| *Port* | `port` | 8790 | The port of the web address (1024–65535). |
| *Access key* / *New key* | `token` | random | The secret assistants send over the web address. *New key* replaces it; assistants using the old one need the new one. |

The command line can change these too, each saving and quitting: `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-allow` / `--mcp-deny` `saved|alarms|names|updates`, `--mcp-files <folder>`, `--mcp-files-off`, `--mcp-port <port>`, and `--mcp-info` to show them with the web address and key. *Copy setup*, *Copy web address* and *Copy access key* are conveniences and store nothing.

### Backup and restore
Saves what you've built up to one file, and puts it back, on this system or a new one. Not stored in `config.json`.

- **Save backup** writes one `.json` file with: the system's **Sonos favorites**, **playlists** (with their songs) and **alarms**, plus Roomtone's own **lists and settings** (music folders, stations, podcasts, saved groups, favorites kept in the app, which sources are on, language, theme, speaker updates). It **never includes** streaming sign-ins or the assistant access key, so it's safe in a normal backup.
- **Choose a backup** opens a backup file and shows what's in it, with a tick box for each part. **Restore** then only **adds**: anything already on the system (same favorite, same playlist name, same alarm) is skipped, and nothing existing is changed or removed. A summary says what was added, skipped and couldn't be restored.
- It works on the system you're looking at (S1 and S2 are separate: choose the system in the sidebar first). Alarms are matched to rooms **by name**, so a room with a different name on the new system is reported and skipped. Saved groups move to the system you restore on.
- **Moving between S1 and S2, or to new speakers:** favorites and playlists that point at your own music library or at radio stations come across. Items from a streaming service use that service's account number on your system, so you may need to be signed in to the same service on the new system (and re-add an item if its account differs).
- **One playlist at a time:** on the Playlists page, a playlist's ⋯ (or right-click) menu has *Export playlist…*, which saves just that playlist and its songs to a file of the type you choose in the Save dialog (M3U playlist, CSV spreadsheet or Roomtone backup `.json`; type the ending yourself if your system's dialog has no type list, and without one you get `.m3u8`); *Import playlist* at the top of the page opens a file and takes you to the summary above, where *Restore* adds it (skipped if a playlist of that name already exists). A single-playlist file is an ordinary backup file with one playlist in it, so *Choose a backup* opens it too.
- **M3U playlists:** choosing M3U (`.m3u8` or `.m3u`) in the Export dialog writes a standard extended M3U8 file (`.m3u8`, UTF-8) that most players can open: songs from this computer's music folders are written as their real file paths, songs on a network share as `smb://` addresses, and anything else as its web address. *Import playlist* also opens `.m3u` and `.m3u8` files: web addresses and shares are taken as they are, and file paths (absolute, relative to the playlist, or `file://`) are matched to songs in **your music folders** (Music Library › Add folder), also when the list was made on another computer, by the last folder and file name. Anything that can't be used (a file outside your music folders, a path on another computer's drive that matches nothing) is skipped and listed under *Details* after you restore. Songs from streaming services only mean something to speakers and are skipped by other players. Streaming (HLS) `.m3u8` files aren't song lists and are refused.
- **CSV playlists:** choosing CSV in the Export dialog writes a spreadsheet-friendly list (UTF-8 with a byte-order mark so Excel shows accents correctly) with the columns *Title, Artist, Album, Duration (seconds), Address*; addresses are written the same way as in M3U. Cells that start with `=`, `+`, `-` or `@` get a leading apostrophe so a spreadsheet won't run them as formulas, and *Import* takes it off again. *Import playlist* reads `.csv` files with comma, semicolon or tab separators and the column names other services use (*Track Name*, *Artist Name(s)*, *Album Name*, *Duration (ms)*, *Track URI*…). A row's address is used if Roomtone can use it; otherwise (for example a Spotify export, whose addresses only work in Spotify) the song is looked up **by title and artist in your music folders**. Rows that match nothing are skipped and listed under *Details*, so a CSV from a streaming service helps you rebuild a playlist from music you have, but it can't bring the streaming songs themselves across.
- With no speakers connected, only the Roomtone part is saved or restored.
- The file format is described below; it's plain JSON, so it can also be read by other tools.

### About
Version, platform, your systems, the language in use, the address your computer serves music from, the assistant link status, the settings folder (*Show*), what's new in this version, help commands, credits, and buttons for the project page, *Report a problem*, *Latest version* and this *Settings guide*. Nothing here is a setting.

## Music settings (outside the Settings page)

| Setting | Where in the app | File | Default |
| --- | --- | --- | --- |
| Which free music sources are on | Settings › Services: switch each source on or off | `services`: any of `"local"` (folders on this computer), `"radio"`, `"podcasts"`, `"archive"` (Internet Archive) | all four on |
| Music folders on this computer | Music Library › *Add folder* (and *Remove folder*) | `folders`: list of paths | none |
| Radio stations you added | Radio › *Add station* (or *Save to My stations* on a stream) | `stations`: `{ "name", "url", "genre" }` | none |
| Podcast shows you follow | a show's *Subscribe* button | `podcasts`: `{ "title", "author", "feed", "art" }` | none |
| Streaming-service sign-ins | Settings › Services › *Connect*, *Disconnect*, and the account number if a service needs it | `accounts`: `{ "sid", "name", "uri", "manifest", "household", "device_id", "token", "key", "sn" }` | none |
| Favorites kept in the app (albums, artists, folders, podcast shows, archive recordings the speakers can't store) | the star on those items | `favorites` | none |
| Scenes ("Movie night") | *Scenes* in the sidebar (*New scene*, *Edit*, *Delete*; Home shows them as buttons) | `scenes`: `{ "name", "household", "rooms", "group", "play", "sleep_minutes", "layout" }` (a room may also hold sound settings, including `sub_level`, `surround_tv`, `delay`; `layout` is optional, `{ "pairs": [...], "sets": [...] }`, made only after a confirmation and only with `allow_layout` on) | none |
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
  "scenes": [],
  "language": "auto",
  "mcp": {
    "enabled": false,
    "allow_control": true,
    "allow_saved": true,
    "allow_alarms": true,
    "allow_names": true,
    "allow_updates": false,
    "allow_files": false,
    "files_folder": "",
    "allow_network": false,
    "port": 8790,
    "token": "0123456789abcdef0123456789abcdef"
  },
  "frame": {
    "enabled": false,
    "port": 8791,
    "token": "fedcba9876543210fedcba9876543210",
    "room": ""
  },
  "theme": "auto",
  "high_contrast": false,
  "menu_bar": false,
  "menu_bar_keep": true,
  "speaker_events": false,
  "room_order": [],
  "check_new_versions": false,
  "last_version_check": 0,
  "remember_history": true,
  "show_lyrics": false,
  "volume_step": 5,
  "allow_update_install": false,
  "allow_layout": false
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
| `scenes` | list | `[]` | Scenes. Each: `name`, `household`, `rooms` (a list of `{ "room": name, "volume": 0-100 or null, "bass": -10..10 or null, "treble": -10..10 or null, "night": true, false or null, "speech": true, false or null }`; a room that's already playing leads the group, otherwise the first), `group` (play together, default true), `play` (a favorite or playlist name, or empty), `sleep_minutes` (0 for none). |
| `language` | text | `"auto"` | `auto`, or `en`, `de`, `fr`, `es`, `it`, `nl`. |
| `theme` | text | `"auto"` | `auto`, `light` or `dark`. |
| `high_contrast` | true/false | `false` | Stronger text, outlines and colours. |
| `menu_bar` | true/false | `false` | Mac and Windows: show Roomtone in the menu bar or notification area. |
| `menu_bar_title` | true/false | `false` | Mac: show the playing room's name beside the menu-bar icon. |
| `menu_bar_keep` | true/false | `true` | With the menu bar on, closing the window keeps Roomtone running instead of quitting. |
| `speaker_events` | true/false | `false` | Instant updates from the speakers. |
| `room_order` | list of text | `[]` | The order you arranged your rooms in (room ids, first to last). Rooms not listed come after, in the speakers' own order. Managed by *Arrange* in the Rooms panel. |
| `check_new_versions` | true/false | `false` | Look for a newer Roomtone at start-up and about once a day. |
| `last_version_check` | number | `0` | When the last good look was (seconds since 1970). Kept by the app. |
| `media_keys` | true/false | `true` | **Not used any more.** The keyboard media keys feature was removed after 0.8.3; the field is still read and kept so older settings files load. |
| `volume_step` | 1, 2, 5, 10 | `5` | How far the volume keyboard shortcut (Ctrl or Cmd with the up and down arrows) moves the volume. |
| `remember_history` | true/false | `true` | Keep a listening history on this computer. |
| `frame` | object | off | Experimental: the now-playing display: `enabled` (false), `port` (8791), `token` (the secret in the address; made for you) and `room` (a room's name; empty follows the room being used). |
| `volume_limits` | object | none | Volume safeguards: `rooms` (room id and `max`), `quiet` (`enabled`, `from`, `to`, `max`) and `keep_down`. |
| `alarm_timers` | list | `[]` | The one-off timers Roomtone made (each: `household`, `id`, `due`), so it can tidy their alarms away after they ring. Kept by the app; leave it alone. |
| `alarm_fades` | list | `[]` | The alarms that fade in (each: `household`, `id`, `minutes`, `volume`): the speaker holds such an alarm at a quiet volume, and Roomtone raises it to `volume` over `minutes` when it rings. Kept by the app; leave it alone. |
| `show_lyrics` | true/false | `false` | Experimental: show lyrics on Now Playing, fetched from lrclib.net. |
| `show_alarms` | true/false | not set (off) | Show the Alarms page in the sidebar. |
| `show_scenes` | true/false | not set (on only if you already have scenes) | Show the Scenes page and its Home shortcuts. |
| `allow_layout` | true/false | `false` | Experimental: show the buttons that make or split stereo pairs and add or remove a soundbar's sub and rear speakers (*Settings › Speakers › Speaker setup*). Each change asks first. |
| `allow_update_install` | true/false | `false` | Experimental: offer *Update all speakers*. |
| `mcp.enabled` | true/false | `false` | Assistant link on. |
| `mcp.allow_control` | true/false | `true` | Assistants may change things. |
| `mcp.allow_saved`, `mcp.allow_alarms`, `mcp.allow_names` | true/false | `true` | Which kinds of change assistants may make (while `allow_control` is on). |
| `mcp.allow_updates` | true/false | `false` | Assistants may install speaker updates. |
| `mcp.allow_files`, `mcp.files_folder` | true/false, text | `false`, empty | Assistants may save backups, only into this folder. |
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
| `scene <name>`, `scenes`, `backup <file.json>`, `play`, `pause`, `toggle`, `next`, `previous`, `play <name>`, `volume <n\|+n\|-n>`, `mute`, `unmute`, `sleep <min>`, `group <room> <room>…`, `ungroup`, `status`, `now`, `queue`, `favorites`, `alarms` (with `--room <name>`, `--group`) | Control the Roomtone that's already running, through its assistant link (switched on; control allowed for anything that changes things). Talks only to this computer. Exit code 0 done, 1 refused, 2 not understood, 3 can't reach Roomtone |
| `--help`, `--help mcp` | Help, and how to connect an assistant on this computer. |

Environment: `ROOMTONE_RENDERER` (`glow` or `wgpu`: how the window is drawn on Windows; normally unset, Roomtone tries OpenGL first and DirectX if that fails), `ROOMTONE_LANG` and `ROOMTONE_THEME` (what `--lang` and `--theme` set); when the language is `auto`, `LC_ALL`, `LC_MESSAGES` and `LANG` decide. The settings folder follows your home folder (`HOME` on macOS and Linux, `APPDATA` on Windows).

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
| `app` | `folders`, `stations`, `podcasts`, `services`, `favorites` (kept in the app), `groups` (`name`, `rooms`), `scenes` (as in `config.json`), `language`, `theme`, `speaker_events`. |

Edit it by hand only if you know what you're doing; a file that can't be read is refused with a message and nothing is changed.

## Other files in the settings folder

| File | What | Safe to delete? |
| --- | --- | --- |
| `speakers.json` | Speakers found last time, for a fast start. Systems not seen for 30 days are dropped (the newest is always kept). | Yes: Roomtone searches again. |
| `config.json.broken-…` | A settings file Roomtone couldn't read, kept for you. | Yes, once you've recovered what you need. |
| `songs.json` | Details of songs played from this computer. | Yes: rebuilt as needed. |
| `crash.log` | Only exists if Roomtone has closed unexpectedly: what went wrong and where, with network addresses and your home folder blanked out. Stays on this computer; attach it to a bug report if you like. Settings › About shows when it was written. | Yes. |
| `window.json` | The window's size and position (and whether it was maximised), so Roomtone opens as you left it. Written a moment after you stop moving or resizing it. | Yes: it opens at the default size. |
| `history.json` | Your listening history (newest 500 songs and stations, with play counts). | Yes, or use *Clear history*. |
| `podcast-progress.json` | Where you stopped in podcast episodes, and which are played (newest 300). Behind *Continue listening* and the *Played* labels. | Yes, but you lose your place in episodes. |

## Back up, move and reset

- **Favorites, playlists, alarms and Roomtone's lists:** use *Backup and restore* in Settings (above).
- **Back up or move the settings file to another computer:** with Roomtone closed, copy `config.json` into the same folder on the other computer. Folders and stations come across; folder paths may need changing if the other computer's drives differ; streaming sign-ins are tied to your speakers' system and may need *Connect* again. Treat the copy like a password file. This hasn't been tried between different platforms.
- **Keep sign-ins out of a shared backup:** remove the `accounts` list and `mcp.token` first.
- **Reset one setting:** delete its field (or set the default from the table above) with Roomtone closed.
- **Reset everything:** close Roomtone and move `config.json` away (or let a broken file be set aside for you). The next start uses the defaults (and makes a new access key).
- **Uninstalling** doesn't remove the settings folder.
