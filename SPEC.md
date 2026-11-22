# Roomtone — specification (v0.9.3)

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
- On start, the speakers remembered from last time (`speakers.json`) appear at once and are checked directly. A network search (SSDP) then runs in the background and adds anything new. The systems are listed with the one that answered most recently first, and the app starts on it. A remembered system that hasn't answered for 30 days is forgotten at the next start (the most recently seen system is always kept, and systems saved before the date was recorded count as seen on the first start after the update). Under Settings › Speakers a system whose rooms all failed to answer is marked *Not on this network* and, when it isn't the only one, has a *Forget* button (with a confirmation); a system that answers can't be forgotten, since searching would add it straight back. A speaker that reports no household (never set up in the Sonos app; **seen on a real Play:3 after a factory reset, 2026-11-19: it appeared as "Unnamed Room" with an empty household, then joined the system again at the same address once set up**) is marked *Not set up yet* and isn't asked for updates, since its update service refuses the check (UPnP error 801); its id is `unset-<address>`. In Software updates such a system says it can't be checked from here and offers no *Check for updates* button. Systems remembered from another network (say, the office) can't answer at home; when the one listed first doesn't answer and another does, the app starts on the one that does.
- If no speakers answer, the **demo system** is shown (one made-up S2 system with 5 rooms and one S1 system with 3), so every screen can be tried. A banner says so and offers "Search again".
- Lost contact (after the computer wakes from sleep, or after three failed polls in a row) triggers an automatic reconnect. Settings also offers a manual **Reconnect**.
- Group state is polled about every second for the selected group, every 4 s for others and every 20 s for grouping (topology). A full background search runs every 2 minutes.
- **Speaker updates** (Settings; off by default): the app subscribes to each speaker's UPnP events (playback on each group's coordinator, volume on every room, room layout on one room per system) and a small listener on this computer receives the `NOTIFY` messages. A notice only says which speaker and what kind of thing changed; the app then reads that group (or the layout) with the same calls polling uses. While a speaker is subscribed and has been heard from, polling for it slows to a safety net (selected group every 10 s, others every 30 s, layout every 60 s); the playback position is counted on screen between answers. If a speaker doesn't send notices within 20 s of subscribing (a firewall, say) the app keeps polling it as before and Settings says so. Subscriptions are renewed at half their lifetime and cancelled when the setting is switched off or the app quits. Only `NOTIFY` messages from a subscribed speaker's address carrying that subscription's id are accepted.

## 3. Window layout

| Area | Contents |
| --- | --- |
| Sidebar | Home, Search, Favorites, Playlists, Music Library, Radio, Line-In & TV, Alarms, Settings; then the **Systems** list (S1/S2, room count, how many are playing); connection status at the bottom (opens the speaker status list) |
| Page area | The selected page; opened items stack, with a back button |
| Right panel | **Rooms** tab: a card per group, in the order the person arranged them (*Arrange* shows up/down arrows on each card; the order is kept as `room_order` in `config.json`, a group standing where its earliest-listed room is, unlisted rooms after, in the speakers' order) (art, track, play/pause, volume, group button, "Plays along with …"), then Group all, Pause all, Ungroup, and the **Groups** section of saved groups. **Queue** tab: the selected group's queue |
| Bottom bar | Art, title, artist, room and system; shuffle, previous, play/pause, next, repeat; position slider with times (or "Live" for streams); favorite star, queue, group, volume, open player |

- The space bar toggles play/pause when no text field has focus.
- Minimum window size is 1040 × 660; the default is 1320 × 840.
- **Appearance**: dark and light palettes share one design. *Automatic* follows the computer's light/dark setting and switches the moment it changes. The brand orange is a little deeper in light mode for contrast; album art and gradients are the same in both.
- The app has its own icon (an orange orb on a dark rounded square), drawn in code and used for the window, the Dock or taskbar, and the About box.
- **macOS application menu**: *About Roomtone*, *Services*, *Hide Roomtone*, *Hide Others*, *Show All*, *Quit Roomtone*, all in the chosen language. *About Roomtone* opens the standard About box with the name, version, platform and architecture, icon, a short description, credits and copyright.

## 4. Features

### 4.1 Rooms and groups
- Group volume, and each room's own volume and mute. Group volume moves members proportionally, as the speakers do.
- **Drag a room card onto another** to join them: the dragged room's group (all its rooms) joins the card it is dropped on, which keeps leading and keeps playing. The card under the pointer gets an orange outline while dragging, a small label follows the pointer, and a toast says "Joining <room> to <other>". Same system only (the panel shows one system), not while *Arrange* is on, and mouse only; *Group rooms* and the group editor do the same from the keyboard. A press that doesn't move still just picks the room.
- **Group editor**: switch rooms of the same system in or out of a group; *Select all*, *Clear*, *Save as group*.
- **Saved groups** (for example "Downstairs"): one click groups exactly those rooms, leaving other rooms alone. They can be edited or deleted. A "Grouped" chip shows when the live group matches.
- **Group all**, **Pause all**, **Ungroup** for the active system.
- **Play along across systems.** S1 and S2 speakers can't share a group: grouping is done by the speakers themselves, the two firmware generations can't sync with each other, and they are separate households. So the group editor can link a group to a group in another system:
  - The app starts the same stream, or the same queue at the same track and position (about 1 s ahead, to allow for start-up), on the other group.
  - It then repeats play, pause, next, previous, seek, play mode, play-item, queue jumps, removing, moving and clearing songs in the queue, and the sleep timer on the other group. **This works from either side**: using the following group moves the leading one too, so the two keep the same queue and the same song. Volumes and other settings stay separate.
  - **Keeping in step.** Every five seconds the app compares the two (the song, and whether each is playing). If they have been on different songs, or one playing and the other not, for 12 seconds (songs end a moment apart on two systems), the song and play state of the group the person used last are copied onto the other. Queues are **not** copied again: copying a queue between an S1 and an S2 system loses the songs one of them can't take (a real test lost songs at each copy), so only linking does it. If the two queues differ in length they are left alone and the person is told to link them again. It waits 10 seconds after the person last used either group, and does this no more than once every 30 seconds. With neither group used, the leading group wins.
  - **At link time both groups get the leader's repeat setting and shuffle is turned off on both** (two systems can't shuffle the same way: each picks its own random next song, so they drift apart on the first skip). If the leader was on shuffle, a notice says so. Turning shuffle back on afterwards passes it on, and the two will drift.
  - A speaker with nothing to skip to (the last song for *next*) is left alone rather than reporting an error.
  - The two may drift up to a second or two apart. Heard on a real S1 and S2 pair: about **one second** apart, fine for rooms in different parts of the house, **not tight enough for two speakers in one room**.
  - One link per group, no chains. Rooms of the same system are refused with "group them instead".
  - Links last until the app quits; they aren't saved.

### 4.2 Playback
- Play, pause, next, previous, seek, shuffle, repeat (off/all/one), crossfade, sleep timer (15, 30, 45 min, 1 h, 2 h, or off).
- **Now Playing** page: large art, track details, the **sound** of what's playing as small labels (format such as FLAC or MP3, sample rate, bit depth, bitrate, and *Lossless*) wherever the speaker says (nothing is shown that isn't reported; it's read from details fetched with each poll, so no extra traffic), with **Technical details** (what the speaker sent, with addresses reduced to their kind) and *Copy details* for reports, artist link ("everything by this artist"), the group's room volumes, play mode, crossfade, sleep timer, upcoming queue. With **Lyrics** switched on (§4.8), a *Lyrics* section (tagged *Experimental*) sits above the volumes for a song that isn't a live stream: while asking it shows *Looking for lyrics…*; timed lyrics highlight the current line and scroll to follow the song (scrolling by hand still works), plain lyrics are just listed, and *No lyrics found*, *This song has no words* or the reason a look-up failed are said in words. Found lyrics are kept in memory for the run only. A **Volume** section has a slider per room and, for a group, an *All rooms* slider; when the rooms in the group aren't all at the group's volume, *Match all rooms to this volume* under *All rooms* sets every room to that volume (one room-volume command per room).
- **Queue**: play from any position, move up or down, remove, clear, save as a playlist. **Drag a row** to move it: a line shows where it will land, and letting go moves the song there (not while the list is filtered, and the list doesn't scroll by itself while dragging; the up and down buttons still work for far moves and from the keyboard).
- **Drop songs on the window**: audio files, or folders of them, dragged from the computer's file manager onto the window play in the room being used (*Play now*: they go in after the song that's playing and start, so the rest of the queue stays). While files are held over the window it dims and says where they will play; with no room picked it says to pick one. Up to 500 songs from one drop; other files are ignored. Dropped songs are served to the speakers like songs in the music folders (and listed with them until Roomtone quits), and nothing else on the computer is; they stay playable when the folders are scanned again.
- Play options on anything: *Play now*, *Play next*, *Add to end of queue*, *Replace queue*. Songs are sent to the speaker up to 16 at a time (one request each time, instead of one per song); a song whose address contains a space is sent on its own, and if the speaker refuses a batch the same songs are sent one at a time. In the queue, *Remove N played songs* (shown when songs above the one playing have played, and no search is typed) takes them all out in one request; if the speaker refuses the range they're removed one at a time, last first.

### 4.2a Keyboard and screen readers

- **Keyboard**: Tab and Shift+Tab move through every button, row, switch and slider, with an orange ring showing where the focus is; Enter or Space presses the focused control; arrow keys move a focused slider (Shift for bigger steps). Space anywhere else plays or pauses the selected room. Ctrl (Cmd on a Mac) with the Right or Left arrow skips to the next or previous song, and with Up or Down changes the volume by the *volume step* (1, 2, 5 or 10; `volume_step`, default 5; Settings › Playback). Ctrl (Cmd on a Mac) with 1 to 9 picks the first to ninth room card of the active system, in the order shown, and says "Controlling <room>" (a toast, which a screen reader reads out), so nobody has to Tab through the whole panel. None of these act while typing in a text box. Room cards in the Rooms panel can be reached with Tab and picked with Enter or Space. The ⋯ button on a list row opens its menu with Enter; on a tile or a row, the Menu key (or Shift+F10) opens the same menu while it has the focus.
- **Screen readers**: the window is exposed through AccessKit (VoiceOver on macOS, Narrator and NVDA on Windows, Orca on Linux). Every hand-drawn control has a name and a role: buttons (with a name for each icon), switches and filter chips as checkboxes (with their on/off state), the segmented control as radio buttons, sliders with their name, a number and their value **in words** ("40 of 100" for volume, "1:15 of 3:18" for the playing position, "+2 on a scale from -10 to +10" for bass and treble), and rows with the song or room they stand for. Controls that are only pictures take their name from `widgets::icon_name`. **Announcements**: a toast is read out when it appears (errors interrupt, others wait for a pause), and an invisible live region says "Now playing: title by artist in room" when the selected room's song changes (`widgets::announce`). Album artwork is treated as decoration: the tile or row it belongs to already carries the song, artist and album name. A test draws the controls and checks the names an accessibility tree reports. **High contrast** (Settings › Appearance, `high_contrast`, off by default) swaps in a stronger palette in both light and dark (text 7:1 or better against the background, accent, status and faint colours 4.5:1, outlines on every control, stronger slider tracks); unit tests compute the contrast ratios of all four palettes. **Tried with VoiceOver on macOS (works); not yet with Narrator, NVDA or Orca**, and the artwork and decorative icons aren't described.

### 4.2b Scenes

A **scene** is a named one-click set-up kept in `config.json` (`scenes`): a list of rooms (each with an optional volume and optional sound settings: bass, treble, and for soundbars night sound and speech enhancement), whether to play them together, a favorite or playlist to start (by name), and an optional sleep timer. The **Scenes** page (sidebar) lists the active system's scenes with *Do it*, *Edit* and *Delete* and a *New scene* window; Home shows them as buttons; `roomtone scene <name>` and the assistant tool `apply_scene` do the same. Doing a scene runs the same steps an assistant would, one after another, and stops at the first that fails, saying which: group the rooms (two or more, if *together* is on; a room that's already playing leads, otherwise the first room), set each room's volume, set each room's sound settings, start what it plays in the first room, set the sleep timer. A scene on the demo system works the same way. In the editor each room has a *Sound* section: bass and treble (a tick box, then −10 to 10) and, only for rooms that report them, night sound and speech enhancement (*Leave as it is*, *On* or *Off*). Scenes are part of backup files (`app.scenes`); restoring adds the ones whose name isn't taken and moves them to the system being restored onto.

### 4.3 Music sources
| Source | What it offers | Needs |
| --- | --- | --- |
| System library | Artists, album artists, albums, songs, genres, composers, imported playlists, shared folders | Music shares set up on the system |
| This computer | Folders on this computer or mounted drives, scanned for audio. A built-in web server serves them to the speakers, so S1 and S2 can both play them while the app runs | Folders added in Music Library |
| Favorites | The system's favorites, plus albums, artists, folders, podcast shows and archive recordings the speakers can't store, kept in the app instead | — |
| Playlists | Playlists saved on the system | — |
| Radio | *My stations* (any stream address; .pls/.m3u links followed), the free Radio Browser directory (popular, by tag, search) | Internet |
| Podcasts | Search the public Apple Podcasts directory, subscribe, play episodes from each show's feed. **Episodes remember where you stopped** (resumes 3 s earlier, if you'd passed 20 s and aren't within 30 s of the end), count as **played** when within 30 s of the end or when marked (*Mark as played* / *Not played* in the item menu), show *Played* or *N min left* in lists, appear under **Continue listening** on Home, and while one plays the Home player and Now Playing page offer **−15 s** and **+30 s**. Progress is kept in `podcast-progress.json` (newest 300 episodes) | Internet |
| Internet Archive | Featured collections, search, play whole recordings | Internet |
| Streaming services | Browse, search and play through the speakers' own music-service connection | Sign-in (§4.4) |

On S1 systems, `https` addresses are relayed through this computer as plain `http`, because S1 speakers can't play some secure streams. The relayed address (`http://<this computer>:<port>/p/<code>.<ending>`) carries the original's audio file ending, or `.mp3`, because speakers refuse a plain web address that has none.

When a page that reads the speakers' library (Favorites, Playlists, Music Library) can't load, it says so in words that name the speakers, not the internet, shows the technical reason in small print, and offers *Try again*. The request goes to the first room and, if it doesn't answer, to the next (up to four). A failed try is never kept for good: *Try again* asks again.

The Music Library filter chips run *Everything*, *Streaming services*, *Podcasts*, *On your system*, *On this computer*, *Internet Archive* (Podcasts and Internet Archive only while switched on). The Music Library page also holds the sources that used to be on a page of their own: **Podcasts** and **Internet Archive** (cards under *Everything*, and a filter chip each, shown while the source is switched on in Settings › Services) next to **Streaming services**, where each connected service opens its own catalogue. With no service connected the Streaming services section offers *Connect in Settings*, which opens Settings › Services. The sidebar no longer has a Music Services item, and `--view services` opens Settings.

### 4.4 Streaming services
Sign-in happens on the service's own web page, with a code when the service uses one. Status as tested on 2026-11-11 against a real S2 system:

| Service | In this app |
| --- | --- |
| Spotify, Amazon Music, Deezer, TIDAL, Qobuz | Sign in, then browse, search and play. Spotify browsing and search confirmed |
| Apple Music, YouTube Music, SoundCloud | **Favorites only.** The services refuse third-party sign-in (Apple sends back an empty encrypted link, YouTube a 403 for unregistered callers, SoundCloud `Client.NOT_AUTHORIZED`). Favorites saved from them in the speaker maker's app show under "Your favorites" on the service card and as a card in Music Library. They're labelled with their service, can be found in Search (also by the service's name) and play here |

- Music plays with the account already added to the system in the speaker maker's app. The account number (`sn`) comes from what's playing, then from favorites, the queue and playlists. If the speakers refuse a song, accounts 1–6 are tried and the one that works is saved. If playback stops within 5 seconds, a note suggests another account number. The number can also be set by hand on the service card. Roomtone's own sign-in to a service belongs to one system (`household` in the saved account); playing on another system, where the service hasn't been added in the maker's app, is refused by its speakers with code 800 for every account number, and the message then says the service isn't added to *this* system yet.
- **Check** (each connected service): tries browsing, a search and finding the account, without playing anything, and shows the result in green or red.
- Settings › Services shows each service as *Connected*, *Ready to connect*, *Favorites only* or *Not offered on this system*.

### 4.5a Long lists
- Lists of songs, albums, playlists, favorites and stations have a filter box, a sort (*Default order*, *Title A–Z*, *Title Z–A*, *Artist A–Z*, *Album A–Z*, *Longest first*, *Shortest first*; songs with no known length go last) and, where tagged, a genre filter.
- A list of 40 or more entries that is alphabetical (by the chosen sort, or in its own order if that is by title or artist) shows an **A–Z strip** of the letters that occur (`#` for numbers and symbols, leading punctuation ignored); clicking a letter scrolls to the first entry under it. Not shown for orders that aren't alphabetical (longest first and so on).
- The queue has a search box (title, artist, album) when it holds more than 8 songs; the rows keep their real positions, so playing, moving and removing act on the right song.
- *Rescan* (Music Library, when music folders are added) reads the folders again.

### 4.5 Search
One box searches every source in parallel: library, favorites (by title, artist, album, description or service name), playlists, this computer, My stations, the radio directory, podcasts, the Internet Archive and each connected service. Results are grouped by type (artists, albums, songs, playlists, stations, podcasts, episodes, folders). There are source and type filters, a genre filter and sort orders. A source that fails says so; the others still show.

### 4.6 Line-In & TV
Plays a speaker's line-in or TV input in the selected room. Each speaker with an input has one row, with a button for each of its inputs. Which speakers have them is guessed from the model name. A speaker without the socket refuses.

### 4.7 Alarms
The **Alarms page** is hidden until *Settings › Appearance › Pages you use › Show Alarms* is switched on (`show_alarms`, off by default; the speakers' alarms keep working either way, and assistants have their own switch, §6.1). The **Scenes page** and its Home shortcuts follow *Show Scenes* (`show_scenes`; off by default, except that someone who already has scenes keeps seeing them until they choose). A hidden page isn't in the sidebar and can't be opened (`--view` is the exception only for the test hook `ROOMTONE_TEST_PAGES=1`, which shows both for screenshots). The page, its switch and the assistant's alarm tools (`list_alarms`, `create_alarm`, `set_alarm_enabled`, `delete_alarm`) are tagged *Experimental*: the page says that alarms have had little testing on real speakers and to check that one rings before relying on it. **When the system has no time zone set** (the speaker's `AlarmClock` `GetTimeNow` returns a `CurrentTimeZone` of all zeros, as on a new S2 speaker not yet fully set up), its alarms never ring, so the page shows a warning line saying so (set the time zone in the Sonos app), and `list_alarms` and `create_alarm` say the same. Read when alarms load or change; nothing is written to the speaker.

**Several rooms, copies and timers** (added in 0.9.3; Experimental like the rest). A *new* alarm has a *Rooms* checklist instead of one room: each room ticked gets its own alarm (the same time, music, volume and length), saved one after another, so they ring on the speakers even if Roomtone is closed; editing an existing alarm still has the single *Room* choice. **Duplicate** (in the editor of an existing alarm) turns the same form into a new alarm with that alarm's settings, ready to change and save. **Timer** (a card at the top of the Alarms page): a room, 5, 10, 15, 30 or 60 minutes or any number from 1 to 720, a volume, and *Start timer*. It is made as a **one-time chime alarm** (recurrence `ONCE`, 1 minute long) on the speaker, so it rings with Roomtone closed but only to the **nearest whole minute** (never less than 20 seconds away). The time is counted from the **speaker's own clock** (`AlarmClock` `GetTimeNow` › `CurrentLocalTime`), because alarms ring by it, and passes midnight correctly. Running timers are listed on the card (*room: rings at hh:mm*) with *Cancel timer*, and are left out of the alarm list. Roomtone remembers each timer (`alarm_timers`: system, the alarm's id and about when it is due), and when the alarms are read again it **tidies away** a timer whose alarm has gone (forgets it) and one that was due more than 5 minutes ago (deletes the alarm, only if it is still a one-time alarm with that id; one that was changed into a normal alarm is left alone and forgotten). Timers made on the demo system are never saved.

**A ringing alarm can be snoozed or stopped from anywhere** (every poll of a playing room asks the speaker whether an alarm is ringing, so this works for a room that isn't selected, with *Instant updates* off too) (not only on the Alarms page, which may be hidden): while a group plays, the app asks whether an alarm is ringing there (`GetRunningAlarmProperties`, at the slower full poll), and if one is, a strip at the top of the window says *Alarm ringing in {room}* with *Snooze 5 min*, *Snooze 10 min* and *Stop the alarm* (pause). Snoozing sends `SnoozeAlarm` (refused with a friendly message if nothing is ringing). The assistant has `snooze_alarm` (room, minutes, default 10; under *Playback*), and `now_playing` shows `alarm_ringing`.

Create, edit, enable/disable and delete alarms. Each has a time, how often it repeats (every day, weekdays, weekends, once and so on), a room, what to play (the chime or any favorite), a volume and a duration.

### 4.8 Settings
The Settings page is a row of seven tabs, one shown at a time, in this order: **About**, **Playback**, **Services**, **Speakers**, **Appearance**, **Assistants** and **Backups**. **About** is selected when the app starts (every time; the chosen tab isn't saved). On a narrow window the tab pills wrap onto a second line; Left and Right move between them. **About** holds the about card and *New versions* (the switch that looks for a new Roomtone and *Check now*); **Playback** holds listening history and lyrics; **Services** holds the switches for the free sources (this computer, radio directory, podcasts, Internet Archive) and connecting streaming services (*Connect*, *Check*, *Disconnect*, the account number); **Speakers** holds the rooms of each system, *Connection* (find, add by address, can't-find check, demo), *Software updates* and *Speaker updates* (instant updates); **Appearance** holds light or dark, *Pages you use* (which optional pages show in the sidebar) and the language; **Assistants** holds the assistant link; **Backups** holds saving and restoring backups. Inside a tab with several cards the card titles are smaller; a tab with one card shows no second title. For screenshots, `ROOMTONE_TEST_TAB=speakers` (or `music`, `services`, `appearance`, `assistants`, `backups`) opens that tab.

The groups hold these things:
- **Systems and rooms** (model, version, address). A room opens its **room settings**: rename, volume, bass and treble (−10…+10), loudness, night sound and speech enhancement (TV speakers), status light, and details.
- **Software updates** (tagged *Experimental*; a switch *Allow installing updates*, `allow_update_install`, off by default, decides whether *Update all speakers* is offered at all; *Check for updates* and the S2 check are always available), for each system:
  - For an S1 system, *Check my speakers* (under *Can this system move to S2?*) reads each speaker's own description (`/xml/device_description.xml`: model name and number, hardware version, software generation; read-only) and shows per room *Already on S2* (software generation 2), *Can't run S2* (model numbers ZP80, ZP100, CR100, CR200, BR100), *Check* (ZP90, ZP120, S5: a first generation that can't and a second that can share the number) or *Can run S2* (anything else), with a summary. It never decides for a *Check* model.
  - *Check for updates* asks the speakers' own update service.
  - When an update is offered, *Update all speakers* asks for confirmation (speakers restart and music stops for a few minutes), then tells every speaker of the system to install it, one after another (*Sending the update to speaker N of M…*). The speakers offer no progress figure of their own, so the app then looks at each room's own software build every 5 seconds and shows *Updating to X: N of M speakers done, K restarting* (a room that isn't answering counts as restarting), then *Updated to X* and a notice when every room reports the new build; the rooms' versions are refreshed at that point. After 45 minutes without all rooms on the new build it says so (*The update hasn't finished…*) instead of waiting forever. The progress is kept in memory only: closing the app during an update loses it, and the next *Check for updates* shows where the system stands. `check_for_updates` reports the same progress lines.
  - Updates are only offered and installed for the same generation: an S1 system is never moved to S2. The generation is checked again just before installing.
  - An offer matching the speaker's current build counts as up to date.
- **Connection**: search again, add a speaker by IP address, open the demo.
- **Appearance**: *Automatic* (follows the computer's light/dark setting), *Light* or *Dark*. It changes at once.
- **Language**: *Automatic* (follows the computer) or a fixed language. It changes at once.
- **New versions**: a switch (off by default) for *Tell me when a new version of Roomtone is out*, a *Check now* button and a status line. When on, a few seconds after start-up, then about once a day (and on *Check now*, switch or not) the app reads the project's latest release record from GitHub, compares its version number with its own, and if it's newer shows *Version X is available* with an *Open download page* button and one pop-up. Pre-releases and drafts are ignored. A *Download* button (only when the release has an installer for this computer: `-macos-arm64.dmg`, `-windows-x64-setup.exe`, `-linux-x64.deb`/`.rpm` (by whether `dpkg` or `rpm` is installed) or `.tar.gz`) fetches that file, from this project's own `releases/download/` addresses only, into the Downloads folder, checks its SHA-256 against the release's `SHA256SUMS` (a release without it, or a file that doesn't match, gives an error and no file) and offers *Open* and *Show in folder*; nothing is downloaded until the button is pressed and nothing is installed, and a failed look is tried again after 5 minutes, then 15, 45, 135 and at most every 360 minutes (kept in memory; *Check now* ignores the wait). The time of the last good look is kept as `last_version_check`. When Roomtone was installed with Homebrew (a `Caskroom/roomtone` folder under `/opt/homebrew`, `/usr/local` or `HOMEBREW_PREFIX`), the line also shows `brew upgrade --cask roomtone` with a *Copy* button, and *Open download page* is no longer the main button.
- **Lyrics (experimental)**: a switch *Show lyrics for the song that's playing* (off by default, tagged *Experimental*; `show_lyrics`). While on and the Now Playing page is open, one HTTPS `GET` per new song goes to lrclib.net (§9) with the song's title, artist, album and length: first an exact match, then the best search hit. Timed lyrics are preferred, then plain text. The demo system shows made-up words and never goes online. If the service says it is busy (`429`), the app waits as long as it asks (up to 10 seconds, off the screen's thread) and tries once more; a song whose look-up failed is tried again the next time it plays. Lyrics are never written to disk. It may be removed again if it doesn't work out.
- **Window**: the size, position and maximised state are saved to `window.json` shortly after they stop changing, and restored at the next start (a size below the minimum is raised to it, and a position far off any screen is ignored so the system chooses). Not used for `--screenshot`.
- **Speaker status** (the *Your speakers* window): besides model, version, address and whether it answers, each room shows whether it's connected by cable (**Wired**) or **Wireless**. When the window opens, Roomtone asks each speaker for its network counters (`GET /status/ifconfig`, read-only; `Cmd::CheckConnections`) and decides from which interface (`eth0` or the wireless one, which is `apcli0` on a Sonos One SL) has carried clearly more traffic; with too little traffic to tell, nothing is shown. No signal strength is shown, because the speakers' status pages don't give one that can be relied on. The demo system shows made-up values.
- **Can't find your speakers?** A button in the *Your speakers* window and in Settings › Speakers opens a guided, read-only check (`Cmd::Diagnose`; `ROOMTONE_TEST_FINDER=1` runs it for screenshots). It reads this computer's network address (is it a home-network address), repeats the search for speakers, and asks every speaker known (found, already in the app or added by address) for its device description on port 1400, noting whether each is on the same network (first three numbers of the addresses). The advice shown follows what answered: not on a home network, nothing answered, the search blocked but speakers answer when asked directly (add by address), a speaker on a different network, some known speakers silent, or all fine; plus a short list of things to check for this platform (macOS Local Network permission, Windows firewall, Linux firewall, VPN, guest network) and an *Add a speaker by address* field. *Copy report* puts an English summary on the clipboard with no room names and no addresses (models and versions, how many answered, same network yes/no) for a bug report. Nothing is changed on the speakers and nothing is sent anywhere. The demo system shows made-up results.
- **Listening history**: a switch *Remember what I've played* (on by default), the number remembered, *See history* and *Clear history*. While on, anything a speaker group has been playing for 20 seconds (songs, stations; not the demo system) is added to `history.json` (newest 500; one entry per song with a play count and last-played time; songs from this computer's own server are matched by their code, since its address changes). Home › *Recently played* is that list's newest 12 and survives restarts (and is what the assistant tool `list_recent` returns); the **History** page shows all with how long ago and how often. Nothing leaves the computer.
- **Speaker updates**: a switch for *Instant updates from the speakers* (§2) with a status line (how many speakers are sending updates, or a hint if they can't reach this computer).
- **Backup and restore**: *Save backup* writes one JSON file (`roomtone_backup` format 1) with the active system's favorites, playlists (with songs) and alarms, and Roomtone's own folders, stations, podcasts, saved groups, kept favorites, sources, language, theme and speaker-updates setting. Never sign-ins or the access key. *Choose a backup* shows what's inside with a tick box per part; *Restore* only adds (duplicates skipped by favorite title+address, playlist name, alarm room+time+repeat+sound, station address, podcast feed, folder path, group name), maps alarms to rooms by name, moves saved groups to the active system, and reports added / skipped / failed. Works on the demo system (favorites, playlists and alarms there). **One playlist**: *Export playlist…* on the playlist's own page (next to Play, Shuffle and Favorite) and in its menu on the Playlists page saves just it, with its songs, to a file whose type is chosen in the Save dialog (M3U `.m3u8`/`.m3u`, PLS `.pls`, XSPF `.xspf`, CSV `.csv` or a one-playlist backup `.json`; a name with no known ending gets `.m3u8`); The M3U type writes an extended M3U8 (real paths for this computer's songs, `smb://` for shares, addresses otherwise); *Import playlist* on that page opens an M3U/M3U8 or backup file and shows the restore summary in Settings (M3U file paths are matched to songs in the music folders, by exact file or by last folder and name; entries that can't be matched are skipped and reported). The PLS and XSPF types write the usual `[playlist]` list and XSPF XML (file paths as `file://` addresses), and import reads both the same way (an XSPF with a DTD is refused). The CSV type writes `Title, Artist, Album, Duration (seconds), Address` (UTF-8 with BOM; spreadsheet-formula guard); import reads CSV with comma/semicolon/tab separators and other services' column names, uses a row's address if usable, otherwise matches title and artist against the music folders, and reports rows it can't place. With no speakers, only Roomtone's own part is saved or restored. Backup files from a newer format are refused.
- **Assistant link (MCP)**: §6.
- Every setting, its place in the app, its `config.json` name, defaults, options and back-up advice is written up in [docs/SETTINGS.md](docs/SETTINGS.md); the About section and `--help` link to it (*Settings guide*). Keep that file in step with this section.
- **About**:
  - version and platform; each system with its room count and speaker software versions;
  - the language in use, the address this computer serves music from, and the assistant link status;
  - the settings folder, with *Show* to open it in the file browser;
  - what's new in this version, help commands to copy, and buttons for the project page, *Settings guide*, *Report a problem* and *Latest version* (on GitHub);
  - credits (Radio Browser, the Apple Podcasts directory search, the Internet Archive, the open-source libraries) and a note that the app is independent of the speaker maker and the music services.

### 4.8a Now-playing display (experimental)
An optional, read-only web address that shows what is playing, for a picture frame, tablet, small screen or script on the home network. **Off until switched on** in *Settings › Playback › Now-playing display* (`frame.enabled`). While on, Roomtone listens on TCP port 8791 (`frame.port`) on all network connections, in the window and in `--headless`. It is tagged *Experimental* (the card says it is experimental and may change; it has not been tried on a real picture frame). It can't play, pause or change anything: only `GET` and `HEAD` are answered.
- **Every address starts with a secret** (`frame.token`, 32 hex digits, made by Roomtone; *New address* replaces it): `/<secret>/`. A picture frame can't send a password, so the secret is the access control. A missing or wrong secret, or the display being off, gets a plain `404`. The secret is compared without stopping at the first difference. Anyone on the network who has the address can see what is playing, which the settings text says.
- `/<secret>/` is a full-screen page (plain HTML with a small script, no outside scripts, fonts or images, and a content security policy): the cover fills the screen on black, with the song and artist under it; paused fades the cover, nothing playing goes black. It asks `now.json` every 2 s and loads a new cover only when the song or its cover changes. `?text=0` hides the words, `?fit=cover` fills the screen instead of fitting it, `?size=` sets the cover's size. `/<secret>` redirects to `/<secret>/`.
- `/<secret>/art.jpg` is the cover of what is playing now, always a JPEG, scaled down to fit `?size=` (64 to 2000, default 800; never enlarged). Roomtone fetches it from the speaker and keeps the last one, so a frame asking often doesn't make the speaker send it again. With no cover, or if it can't be fetched, a small dark square is sent so a frame always gets a picture.
- `/<secret>/now.json` is `{"playing", "title", "artist", "album", "room", "has_art", "id"}`; `id` changes whenever the song or its cover does. With nothing to show everything is empty and `playing` is false.
- **Which room**: *Room to show* (`frame.room`, a room's name) follows that room's group; empty follows the room being used in the window, or, with no window or none chosen, the first group that is playing.
- At most 16 connections are served at once; a 17th gets `503`.
### 4.8b Volume limits
Safeguards against a room being made too loud, in *Settings › Playback › Volume limits* (`volume_limits`; none until set). **The speakers have no limit a controller can set** (their `RenderingControl` service has none, checked on a real S1 room and a real S2 speaker), so Roomtone applies the limits to **what it sends**: every command goes through `limits::limit` in `Backend::send`, before the screen is updated, so a slider shows the volume it really got. That covers the window (sliders, Ctrl/Cmd+up/down, the *Match* button), the command line, assistants (`set_volume` says what it ended at, and that the limits lowered it), scenes, timers and *new or saved alarms* (an alarm's volume is capped at the room's limit when it is saved; an alarm already on a speaker is not changed). **Another app, or the buttons on a speaker, can still go higher.**
- **A highest volume per room** (0 to 99; 100 is no limit, shown as *No limit*), kept by the room's id so renaming doesn't lose it. A group volume stops where its loudest room reaches its limit (rooms keep their proportions, as the speakers do). A room that is *already* over its limit is never raised further, but can still be lowered, even if the new volume is still over.
- **Quiet hours** (a switch, from, to, and *no louder than*; 22:00 to 07:00 and 20 by default; runs past midnight if *to* is earlier than *from*): every room is held to the lower of its own limit and the quiet-hours volume. The time of day is **the speakers' clock** (the home's local time, read from the speakers every ten minutes; the demo system uses this computer's, in UTC), the same clock alarms ring by. Until it has been read, quiet hours aren't applied. Alarms and timers are held to the room's limit but not to quiet hours (they ring at their own time).
- **Turn a room back down if it goes over** (off by default): every three seconds Roomtone looks for rooms louder than their limit and sets them to it, at most once per room every 15 seconds. **Only while Roomtone is running**, and only on a real system.
- A lowered volume shows a short toast ("Kitchen is limited to 40"), at most one every four seconds.

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
| Assistants can change things | on | Off: only the look-only tools are listed; the others are refused. Look-only beats every switch below |
| Saved items | on | Within *change things*: `add_favorite`, `remove_favorite`, `save_queue_as_playlist`, `add_station`, `remove_station`, `subscribe_podcast`, `unsubscribe_podcast`, `save_group`, `delete_saved_group`, `save_scene`, `delete_scene` |
| Alarms | on | `create_alarm`, `delete_alarm`, `set_alarm_enabled` |
| Room names and sound | on | `rename_room`, `set_sound` |
| Speaker updates | **off** | `install_update` (still refused unless `confirm: true`) |
| Save files | **off** | `save_backup`, `export_playlist`. Needs a **folder** (`files_folder`) chosen in Settings: files are written only there, a bare file name goes in it, a longer path must already point into it, `..` is refused, the name must end in `.json` and nothing is overwritten |
| Allow other computers | off | Off: listen on 127.0.0.1 only and accept only local `Host` names. On: listen on all interfaces (Settings only, not on the command line) |
| Port | 8790 | HTTP port |
| Access key | random 32 hex characters | Required as `Authorization: Bearer <key>` on HTTP. *New key* replaces it |

A tool in a switched-off group isn't listed and is refused with a plain sentence naming the setting. Command line, each one saves and quits: `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-allow` / `--mcp-deny` `saved|alarms|names|updates`, `--mcp-files <folder>`, `--mcp-files-off`, `--mcp-port <port>`. `--mcp-info` prints the address and key. Settings also offers *Copy setup* (a ready-made `mcpServers` block), *Copy web address*, *Copy access key*, and a collapsible **How to connect on <platform>** with copyable commands.

### 6.2 Protocol
- JSON-RPC 2.0 with MCP protocol versions `2025-11-25`, `2025-06-18`, `2025-03-26` and `2024-11-05` (the client's choice if supported, otherwise the newest).
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, `resources/read`, `resources/subscribe`, `resources/unsubscribe`, `prompts/list`, `prompts/get` and `completion/complete`. Notifications get no reply. Batches are accepted. Capabilities: tools, resources, prompts and completions (none of them send change notifications).
- HTTP (Streamable HTTP, JSON responses only):
  - `POST /mcp` for requests. `initialize` is answered with an `Mcp-Session-Id` header; an assistant that sends it back on later requests has its own change subscriptions (§6.4). `GET /mcp` with `Accept: text/event-stream` and that header opens an event stream carrying `notifications/resources/updated`; `DELETE` with the header ends the session. Other `GET`s return 405.
  - Requires `Content-Type: application/json` (otherwise 415) and a valid key (otherwise 401).
  - Refuses an `Origin` other than localhost (403), and with *Allow other computers* off, a non-local `Host` (403).
  - Bodies up to 1 MB. A notification-only request returns 202.

### 6.3 Tools
Rooms are given by name (exact, or a unique part of a name) or id. Leaving the room out means the room selected in the app. Look-only tools are marked *(read)*; the others are hidden and refused in look-only mode.

| Tool | Does |
| --- | --- |
| `get_status` *(read)* | Every system, room (model, software version and, when known, wired or wireless), group, what's playing (with the sound details the speaker reports: format, sample rate, bit depth, bitrate, lossless), volumes, play mode, play-along links and saved groups (JSON) |
| `now_playing` *(read)* | One room's group: track, position, state, volume, play mode, sleep timer |
| `playback` | `scene <name>`, `scenes`, `backup <file.json>`, `play`, `pause`, `toggle`, `next`, `previous` |
| `set_volume` | Absolute `volume` 0–100 or relative `change`; `whole_group` for the group |
| `set_mute` | Mute or unmute a room or its group |
| `set_play_mode` | `shuffle`, `repeat` (off/all/one), `crossfade` |
| `seek` | Jump to `seconds` |
| `snooze_alarm` | Silence an alarm that is ringing in a room and have it ring again after some minutes (default 10); `now_playing` shows `alarm_ringing` |
| `snooze_alarm` | Silence an alarm that is ringing in a room and have it ring again after `minutes` (default 10); `now_playing` shows `alarm_ringing` |
| `sleep_timer` | Stop after `minutes` (0 = off) |
| `list_favorites` *(read)* | Favorites with refs (`fav1`…) |
| `list_playlists` *(read)* | Playlists with refs (`pl1`…) |
| `search` *(read)* | Everything search covers, with refs (`r1`…), up to `limit` |
| `play` | A `ref`, or a favorite/playlist `name`; `how`: now, next, end, replace |
| `get_queue` *(read)* | The queue numbered from 1, the current song marked ▶ |
| `play_from_queue`, `remove_from_queue`, `clear_queue` | Queue control by position; `remove_from_queue` takes an optional `count` to remove that many songs in a row in one request |
| `group_rooms`, `ungroup_room`, `group_all`, `use_saved_group` | Grouping within one system |
| `play_along`, `stop_play_along` | Play along across systems (§4.1) |
| `switch_input` | Line-in or TV input, optionally from another room |
| `get_sound` *(read)*, `set_sound` | Bass, treble, loudness, night sound, speech enhancement, status light |
| `list_alarms` *(read)*, `set_alarm_enabled` | Alarms |
| `check_for_updates` *(read)* | Whether each system has newer software (same generation only) |
| `install_update` | Installs on every speaker of a room's system; refused unless `confirm: true` |
| `browse` *(read)* | Step through the sources: without a ref the top level (favorites, playlists, library, this computer, radio, podcasts, Internet Archive, services); with a ref, what's inside (refs `b1`…) |
| `save_backup` | Writes a backup `.json` of the system's favorites, playlists and alarms and Roomtone's own lists and settings to a new file on this computer (never overwrites; `.json` only). Reads the speakers; restoring stays in the app |
| `save_scene`, `delete_scene` | Make or replace a scene (rooms with optional volumes, group or not, a favorite or playlist by name, a sleep timer; rooms in one system) and delete one. In the *Saved items* switch |
| `check_s2` *(read)*, `diagnose_network` *(read)* | Which speakers could move to S2 (the app's *Check my speakers*); the *Can't find your speakers?* check as a report with no names or addresses |
| `export_playlist` | Save a playlist (a ref from `list_playlists`) as `.m3u`, `.m3u8`, `.pls`, `.xspf`, `.csv` or `.json` in the folder chosen for assistants; same folder rules and switch as `save_backup` (*Save files*) |
| `list_scenes` *(read)*, `apply_scene` | The saved scenes; do one by name (groups its rooms, sets volumes, starts what it plays, sets its timer) |
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

**Change notifications.** `resources/subscribe` (capability `resources.subscribe: true`) follows `roomtone://status`, `roomtone://now-playing`, `roomtone://alarms` or `roomtone://queue/{room}` (favorites can't be followed). When one changes, the assistant gets `notifications/resources/updated` with its URI, on a line of stdout (`--mcp`) or on the event stream (HTTP). Roomtone looks once a second at what it already knows and leaves out values that tick on their own (the song position, sleep-timer minutes), so a playing song sends nothing; queues and alarms are asked for again every 10 and 30 seconds while followed. At most 20 resources per assistant and 64 assistants at once.

Prompts (ready-made instructions an assistant can offer the person): `play_for_mood` (mood, room), `bedtime` (room, minutes), `whole_home` (what), `morning_alarm` (room, time). `completion/complete` suggests room names, saved-group names, alarm days and moods while an argument is typed.

Each changing tool waits until the speakers have answered (up to 20 s). It then returns a one-line summary, or the speakers' error with `isError: true`. Refs last until the next list of the same kind.

## 7. Command-line options

| Option | Effect |
| --- | --- |
| *(none)* | Open the window |
| `--demo` | Demo system |
| `--lang <code>` | Language for this run |
| `--theme <mode>` | Appearance for this run: `auto`, `light` or `dark` |
| `--view <page>` | Open a page first: home, search, favorites, playlists, library, radio, inputs, alarms, settings, nowplaying, services, history; or open a window: speakers, finder |
| `--screenshot <file.png>` | Draw about 120 frames, save the window as PNG and quit. Uses the demo system, opens without taking focus (and behind other windows on Windows) and doesn't open the assistant port |
| `--headless`, `--mcp` | §5 |
| `--mcp-on`, `--mcp-off`, `--mcp-read-only`, `--mcp-allow-control`, `--mcp-port <n>`, `--mcp-info` | §6.1 |
| `play`, `pause`, `toggle`, `next`, `previous`, `play <name>`, `volume <n\|+n\|-n>`, `mute`, `unmute`, `sleep <min>`, `group <room> <room>…`, `ungroup`, `status`, `now`, `queue`, `favorites`, `alarms` (with `--room <name>`, `--group`) | Control the Roomtone that's already running, through its assistant link (switched on; control allowed for anything that changes things). Talks only to this computer. Exit code 0 done, 1 refused, 2 not understood, 3 can't reach Roomtone |
| `--help`, `--help mcp` | Options; the assistant guide for this platform |

Test-only environment variables: `ROOMTONE_LIVE` (screenshots with real speakers), `ROOMTONE_SHOT_FRAME`, `ROOMTONE_SCROLL`, `ROOMTONE_TEST_GROUP`, `ROOMTONE_TEST_RECONNECT`, `ROOMTONE_TEST_EDITOR`, `ROOMTONE_TEST_PANIC` (panics at start, to see the crash log), `ROOMTONE_TEST_ABOUT` (macOS: also saves a picture of the About box), and `ROOMTONE_SPEAKER` / `ROOMTONE_TERM` for the ignored network tests.

## 8. Stored files

All in the app's data folder: `~/Library/Application Support/Roomtone/` (macOS), `%APPDATA%\Roomtone\` (Windows) or `~/.config/roomtone/` (Linux). Unknown fields are ignored and missing fields take defaults, so older files keep working. **Add new fields with `#[serde(default)]`.**

| File | Contents |
| --- | --- |
| `config.json` (mode 600 on Unix) | `folders` (paths), `stations` [{`name`, `url`, `genre`}], `podcasts` [{`title`, `author`, `feed`, `art`}], `services` (switched-on free sources: `local`, `radio`, `podcasts`, `archive`), `accounts` [{`sid`, `name`, `uri`, `manifest`, `household`, `device_id`, `token`, `key`, `sn`}], `favorites` (items kept in the app), `groups` [{`name`, `household`, `rooms`}], `language` (`auto` or a code), `theme` (`auto`, `light` or `dark`), `speaker_events` (`true` or `false`), `scenes` [{`name`, `household`, `rooms` [{`room`, `volume`}], `group`, `play`, `sleep_minutes`}], `room_order` (room ids, first to last), `remember_history` (`true` or `false`), `check_new_versions` (`true` or `false`), `last_version_check` (seconds since 1970), `mcp` {`enabled`, `allow_control`, `allow_network`, `port`, `token`} |
| `speakers.json` | Systems from last time: [{`id`, `s1`, `rooms` [{`uuid`, `name`, `ip`, `model`, `version`}], `groups` [{`coordinator`, `members`}], `last_seen` (seconds since 1970)}] |
| `songs.json` | `songs`: details (title, artist, album, art) by address, for songs the speakers know only by address; `relays`: relay codes for S1 and the https address each stands for |
| `window.json` | The window's size, position and maximised state (§4.8) |
| `crash.log` | Written only if Roomtone panics: version, system, time (UTC), thread, message, place and a backtrace, with IPv4 addresses and the home folder blanked out (`crash.rs`). Kept to about 64 KiB, newest last. Never sent anywhere; Settings › About shows a *Crash log* row, with the date, only while the file exists |
| `history.json` | The listening history: up to 500 entries (song or station, address, play count, last played) (§4.8) |
| `podcast-progress.json` | Where you stopped in each podcast episode and which are played (newest 300) |

## 9. Network, security and privacy

| Port | Direction | Use |
| --- | --- | --- |
| UDP 1900 (multicast 239.255.255.250) | out/in | Finding speakers (SSDP): the search goes out of every network connection, and the port is shared and listened on for speakers announcing themselves (`NOTIFY`). An announcement from a speaker we aren't already talking to starts one search, after 3 s and not more than every 10 s. The announcement is used only as a hint that a speaker may be new; its address is never contacted because of it |
| TCP 1400 on each speaker | out | Control (UPnP/SOAP), artwork, device details; a room that is playing is also asked whether an alarm is ringing on each check, and the system's time zone is read when alarms load |
| TCP 52880 (or a free port) on this computer | in | Speakers fetch music from this computer and S1 https relays. Only addresses on this computer or a local network (loopback, private, link-local, unique-local) are answered; a request that isn't sent within 10 s, a request line over 8 KiB or more than 64 header lines is dropped, a speaker that stops reading for 2 minutes is dropped, and at most 64 connections are served at once |
| A free TCP port on this computer (only while *Speaker updates* is on) | in | Speakers send change notices (UPnP events); anything not from a subscribed speaker with its subscription id is refused |
| TCP 8790 (configurable) | in | Assistant link over HTTP, 127.0.0.1 unless other computers are allowed |
| TCP 8791 (configurable, only while *Now-playing display* is on) | in | Read-only now-playing page, cover and description (§4.8a); every address starts with a secret, other requests get 404 |
| HTTPS to github.com release downloads (only when *Download* is pressed in *New versions*) | out | One `GET` of `SHA256SUMS` and one of the installer, from this project's own release; the same `User-Agent`; saved to the Downloads folder |
| HTTPS to api.github.com (only if *New versions* is on, or *Check now* is pressed) | out | One `GET` of this project's latest release; the only thing sent that identifies the app is the usual `User-Agent: Roomtone/<version>`; no account, no identifier |
| HTTPS to lrclib.net (only if *Lyrics* is on and Now Playing is open) | out | One or two `GET`s per new song with its title, artist, album and length (a `User-Agent` with the app version and project address); no account, no identifier; the answer is kept in memory only |
| HTTPS out | out | Radio Browser, Apple Podcasts search and feeds, Internet Archive, streaming services' own APIs |

- No telemetry. Nothing is sent anywhere except the speakers, the music sources listed above, the services the person signs in to and, only if *Lyrics* is switched on, lrclib.net.
- Sign-in tokens and the access key live only in `config.json`, readable only by the user.
- A settings file that can't be read is never overwritten: at start-up it is renamed `config.json.broken-<n>`, the defaults are used, and Settings shows a notice with the new name. If it can't be renamed, saving is switched off for that run. Read-only callers (`--help`, the headless copy's change watcher, streaming-service lookups) never rename or write it.
- Actions that can't be undone are never done without the person's confirmation: software updates; never moving S1 to S2.

## 10. Verification status

| Area | Verified how |
| --- | --- |
| Discovery, live status, browsing, favorites on real **S2** speakers | Real speakers (2026-11-11); **a real Play:3 on S2 software (2026-11-18)**: playback of an internet radio station (AAC) started, advanced and paused, volume was set and restored. Streaming-service playback on S2 not yet heard |
| Now-playing display (§4.8a) | A pretend speaker serving a cover and the demo system in tests (secret, read-only, scaling, HEAD, changing the secret); the page in a real browser on the demo system. **Not yet on a real picture frame or tablet, nor with a real speaker's cover** |
| Discovery and control on a real **S1** speaker | Reported working by the maintainer in an earlier version |
| Streaming sign-in pages (8 services), Spotify browse and search | Real services through an S2 system |
| Streaming **playback** and the account-number retry | **Spotify plays on the maintainer's real S1 speakers (2026-11-18) and, once Spotify was added to the new system in the maker's app, on a real S2 Play:3 (2026-11-18)**; the account-number retry, other services and S2 systems not yet heard |
| Play along between real S1 and S2 speakers (queue edits and keeping in step, both directions) | Demo system and unit tests only |
| Software updates (check and install) | Check: real S1 and S2 (read-only). Install: demo system only; tagged *Experimental* and off until switched on |
| Forgetting old systems, and starting on a system that answers (0.9.0) | **Checked on a real setup by the maintainer (2026-11-18)**: one S1 system answering and two S2 systems remembered from another network; the app started on the S1 system, the others showed *Not on this network*, *Forget* removed one and it stayed gone after a restart. The 30-day rule and the ordering are unit-tested |
| Settings tabs, Settings › Services, Music Library chips and cards, Line-In & TV rows (0.9.0) | Demo system and screenshots (English and German, light and dark). **Check on a connected Spotify account worked on real speakers (2026-11-19: browsing 5 sections, search 122 results, plays with account 9 found on the system) and the services list showed each service's state correctly. Connect and Disconnect not yet tried on real speakers** (the same calls as before, moved) |
| Following a software update (0.9.1) | **Watched read-only on a real Play:3 (2026-11-18)** while the maker's app updated it: builds 65.1-21040, 73.0-42060 and 86.11-82170 (display version 27.3), a restart between each, and the update service kept offering the same release as `98.0-82170` (so builds are compared by their id). Roomtone's own install of a real update is **untried**: told to install, that speaker did nothing in the first minutes |
| Faster queue changes: batch adds and range removal (0.9.1) | **A real S1 Connect:Amp room with an empty queue (2026-11-18)**: a batch of 16 and a range removal with update id 0 were accepted, order and count came back right (ignored test `real_queue_batches`). **The same test passed on a real S2 Play:3 (86.11-82170, 2026-11-18).** **The *Remove N played songs* button itself was clicked in the real window on a Play:3 (2026-11-19) and removed the three played songs, which the speaker confirmed.** Untried: a long queue of real songs. The operation behind it passed on a real Play:3 (2026-11-19, silent) |
| Listening for speaker announcements (0.9.1) | Real announcements were seen on the maintainer's network (five S1 speakers sent 25 to 28 notices in 90 s; the Play:3 sent six); the search they trigger and the "is this news" rule are unit-tested. **A real Play:3 switched on while the app was running appeared by itself about 45 s later, before the 120 s regular search was due (2026-11-19); unplugged, it showed as not answering and came back by itself.** **A real Play:3 moved to a new address by the maintainer (2026-11-19) showed as not answering about 27 s after the move and was found again at the new address by itself.** Untried: Windows and Linux |
| Searching out of every network connection (0.9.1) | A Mac with one connection finds the same two systems as before. **Untried on a computer with several connections, and on Windows and Linux** |
| Music server limits, crash log, first-run instructions (0.9.1) | Crash log: a deliberate crash on a Mac (addresses and home folder blanked). Limits: unit tests; a local file played to a real speaker afterwards is **not yet tried**. First-run steps: **not yet tried on a clean macOS or Windows computer** |
| Windows: no console window (0.8.3) | **Confirmed on a real Windows PC by the maintainer (2026-11-18)**: the app starts with no console window. Printing to a terminal (`--help`, the command-line commands) not yet checked there |
| S2 compatibility check | Read-only on a real S1 system of five Connect:Amps (ZP120, hardware 1.16.3.1, software generation 1 → *Check*); S2 speakers and the Connect/Play:5 generations not seen |
| Alarms made by Roomtone and *Snooze* (0.9.2, experimental) | Demo system and unit tests. **Snooze worked on a real ringing alarm** (made in the Sonos app, playing music, on a real S1 Connect:Amp room, 2026-11-19: it paused at once and rang again 61 s later; alarms fade in from volume 0 over about 15 s). **A one-time radio-station alarm made from Roomtone rang on that room, twice** (volume 25, 5 min). **One-time chime alarms rang at volume 25 for 5 min and at volume 15 for 4 min.** An earlier chime alarm on that room did not ring and couldn't be reproduced; a new S2 Play:3 rang none while it had no time zone set (which Roomtone now warns about), **and a one-time radio alarm made from Roomtone rang on it once a time zone was set** (BBC 6 Music, volume 20, 2026-11-19) |
| Backup: saving favorites, playlists and alarms | One real S1 system, read only (counts and round trip); demo system for everything else |
| Backup: **restoring** favorites, playlists and alarms | Pretend system in unit tests and the demo system; **not yet tried on real speakers** (the playlist calls `CreateSavedQueue`/`AddURIToSavedQueue` are unverified) |
| Sound quality labels | Unit tests on made-up and real-S1-library descriptions (format from the MIME type is what a real S1 system sends for library tracks); **sample rate, bit depth and bitrate from real speakers not yet seen** (nothing was playing when checked) |
| Speaker updates (UPnP events) | Unit tests against a pretend speaker (subscribe, notice, renew, cancel, refusals); tried once with a real S1 speaker (Connect:Amp, firmware 11.16): all three subscriptions accepted, a first notice for each within about 5 s, a 1800 s lease, renewing and cancelling work; no S2 or multi-room tests yet |
| Assistant link: every tool, on/off, look-only, the per-group switches, the backup folder, key, origin check | Demo system, stdio and HTTP; unit tests for each group and for the folder rules |
| Assistant change notifications (`resources/subscribe`) | Unit tests and a stdio run on the demo system; **not yet tried with a real assistant client** |
| Command line (`roomtone play`, `volume`, `scene`, `backup`…) | Unit tests and a headless demo instance; **not yet run against a real system** |
| Scenes | Unit tests and the demo system; once on two rooms of a real S1 system (grouping, volumes, a station, a sleep timer); **not yet on S2, with an album or playlist, or with an offline room** |
| Speaker status (wired or wireless) | One real S1 Connect:Amp (wired); two real S2 Sonos One SL speakers on Wi-Fi (2026-11-17: first showed no connection type because their Wi-Fi interface `apcli0` was not known; fixed in 0.8.2, and both now read Wireless; the maintainer confirmed both are on Wi-Fi). A real S2 Play:3 moved between Wi-Fi and cable (2026-11-19): wired by its counters on the cable; on Wi-Fi its counters were silent but the system reported wireless. An S1 room on wireless not yet seen |
| *Can't find your speakers?* | Unit tests and the demo system; **not yet on a genuinely failing network** |
| PLS and XSPF playlist files | Unit tests; **not yet opened in other players** |
| Keyboard focus and screen-reader names | Unit test of the accessibility tree and screenshots; **VoiceOver on macOS tried by the maintainer (2026-11-16): works**; Narrator, NVDA and Orca not yet tried |
| Lyrics (experimental) | Unit tests (parser, look-up order, nothing sent unless on) and the demo system's made-up lyrics; one real look-up of a traditional song on lrclib.net returned timed lyrics (2026-11-16, ignored test `real_lookup`); **tried by the maintainer with real songs playing on real speakers (2026-11-16): works well**; accuracy and coverage depend on the community site |
| Homebrew install and upgrade | Real Mac (0.6.2 installed and upgraded through the tap) |
| Translations | Automated completeness test; not reviewed by native speakers |
| Logic without speakers (parsers, rules, settings, demo system, assistant link incl. HTTP security) | About 235 unit tests (`cargo test`), run on every platform in CI |
| Light mode | Screenshots of every page in the demo system |
| Windows and Linux builds | Continuous integration (`.github/workflows/ci.yml`); not run by hand |

## 11. Acceptance checks

Quick checks that a change hasn't broken behaviour. Each needs no real speakers unless marked.

1. `cargo test` passes: 103 unit tests, including that every phrase is translated and placeholders are kept.
2. `roomtone --screenshot shots/x.png` saves a non-blank picture of the demo system within a few seconds.
3. `roomtone --mcp --demo` with an `initialize` line, then `tools/list`, lists 54 tools when control is allowed (with speaker updates and saving files left off, as they are by default; 57 with every switch on) and 17 when look-only.
4. With the link off, `tools/call` answers error −32001.
5. Over HTTP, a request without the key gets 401, one with `Origin: https://example.com` gets 403, and `--mcp-off` closes the port within ~2 s.
6. In the demo: `play_along` from "Living Room" to "Office", then `playback next` on Living Room, changes Office's track too.
7. In the demo: `check_for_updates` offers an S2 update and none for S1. `install_update` without `confirm` is refused; with it, the update is installed.
8. *(real speakers)* Settings › Services › *Check* on a connected service shows browsing and search working.
9. `roomtone --theme light --screenshot shots/light.png` draws the light palette (and `--theme dark` matches the earlier dark pictures).
