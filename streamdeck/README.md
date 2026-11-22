# Roomtone for Stream Deck (experimental)

> **Experimental.** This plugin is new. It has been tested against a Roomtone demo system and with automated tests, but **not yet on a real Stream Deck or with real speakers**. It may change, and it may not work for you. Reports are welcome.

Buttons and a dial for [Roomtone](../README.md) on an Elgato Stream Deck: play and pause, next and previous, volume (hold to keep going), mute, scenes, pause everything, and a key that shows the song that's playing. On a Stream Deck + the dial turns the volume, pressing it mutes, and tapping the screen plays or pauses.

Roomtone and this plugin are independent projects, not made or endorsed by Sonos or Elgato.

## What you get

| Action | Does |
| --- | --- |
| **Play / Pause** | Toggles; shows a pause symbol while music plays |
| **Next song**, **Previous song** | Skips |
| **Now Playing** | Shows the song's title on the key; press to play or pause |
| **Volume Up**, **Volume Down** | Changes the volume by the *Volume step* (default 5); hold to keep going; shows the volume |
| **Mute** | Mutes or unmutes; shows a crossed-out speaker while muted |
| **Scene** | Applies one of Roomtone's scenes (pick the name in the button's settings) |
| **Pause All** | Pauses every room |
| **Volume Dial** (Stream Deck +) | Turn: volume. Press: mute. Tap the screen: play or pause. Shows the room and the volume |

Every button acts on the room you pick from a list in its settings (the list comes from Roomtone), or on whichever room is selected in Roomtone (*Follow the room selected in Roomtone*, the default). You can type your own title on any key; it replaces the one the plugin would show (such as the volume number or the song title). Volume changes move the whole group, as the sliders in Roomtone do, and Roomtone's [volume limits](../docs/SETTINGS.md#volume-limits) apply to them.

## Setting up

1. **In Roomtone** (Settings › Assistants): switch on **Let AI assistants use Roomtone**, keep **Assistants can change things** on, and copy the **access key**. The plugin uses the same local *assistant link* that AI assistants use; there is no separate switch for it.
2. **Install the plugin.** There is no download yet; build it (needs Node.js 24 or later):
   ```sh
   cd streamdeck
   npm ci
   npm run pack
   ```
   then double-click `io.github.chrispygee.roomtone.streamDeckPlugin`. Stream Deck 7.1 or later, on macOS 13+ or Windows 10+. (The file isn't signed or from the Marketplace, so Stream Deck may ask you to confirm.)
3. **Add a button** from the *Roomtone (experimental)* group, and in its settings paste the **access key** (the host and port can stay as they are when Roomtone runs on the same computer). *Check the connection* says whether it can reach Roomtone, and fills the room list. The connection settings are shared by all buttons.

Roomtone must be running (with its window, or `roomtone --headless`) for the buttons to work. If it isn't, or the key is wrong, a button says so on its face (*Roomtone not running*, *Wrong key*, *Link is off*) and shows the alert mark when pressed.

## How it works, and what it can see

The plugin runs inside the Stream Deck app (as a Node.js program) and talks to Roomtone's assistant link over HTTP, by default at `http://127.0.0.1:8790/mcp`, with your access key. It calls only these Roomtone tools: `now_playing` (every two seconds while a button that shows something is on screen), `playback`, `set_volume`, `set_mute`, `apply_scene`, `pause_all`, `list_scenes` and `get_status`. It sends nothing anywhere else, and has no analytics.

- **The access key is stored by Stream Deck in plain text** in the plugin's settings (Stream Deck says plugin settings are not secret). It lets anyone who can read it control your speakers through the assistant link, and it is the same key AI assistants use. *New key* in Roomtone's settings replaces it. Keep **Allow other computers** off in Roomtone unless the Stream Deck is on another computer.
- Scene names are read from Roomtone's `list_scenes` reply, one per line before the first colon; a scene name that contains a colon won't be listed in the settings suggestions (type it by hand).

## Developing

```sh
cd streamdeck
npm ci
npm run build        # bundles src/ into the plugin's bin/plugin.js
npm run typecheck
npm test             # unit tests (a pretend Roomtone); needs Node 24+
npm run validate     # Elgato's manifest checker
npm run pack         # builds the .streamDeckPlugin file
npm run watch        # rebuild on change; `npx streamdeck link io.github.chrispygee.roomtone.sdPlugin` installs it for development
```

A test that talks to a real (demo) Roomtone is skipped unless you start one and tell it the key:

```sh
roomtone --headless --demo       # in another terminal (note its assistant link port)
ROOMTONE_LIVE_KEY=<access key> ROOMTONE_LIVE_PORT=8790 npm test
```

Layout: `src/roomtone.ts` (the link's client), `src/monitor.ts` (asks what's playing for the buttons that show it), `src/format.ts` (text for small screens), `src/actions/` (one class per button), `io.github.chrispygee.roomtone.sdPlugin/` (the manifest, icons and the settings page).

`npm audit` reports a high-severity advisory in `zod`, which the Stream Deck SDK (`@elgato/streamdeck`) depends on; the suggested fix is to go back to an old SDK major version, so it is left as is. It concerns memory use when checking very large inputs; the plugin only handles small messages from the Stream Deck app and from Roomtone on your own computer.

## Not done yet

- Not tried on real Stream Deck hardware, including the Stream Deck + dial and touch strip.
- No cover art on keys (Roomtone's *Now-playing display* address could supply it).
- Not on the Elgato Marketplace, and not attached to Roomtone's releases.
- Only the listed actions; no room picker, no queue or favorites buttons.
