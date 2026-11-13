# Roadmap and open questions

What's known to be unfinished or unverified, and ideas for what's next. If you pick one up, open an issue first so work isn't doubled. When something here is settled, move it into [SPEC.md](../SPEC.md) and the [CHANGELOG](../CHANGELOG.md).

## Needs checking on real speakers

These work on the demo system but haven't been confirmed on real hardware. Reports are very welcome; use the "Speaker report" issue form.

| Question | How to settle it | Matters |
| --- | --- | --- |
| Do streaming-service songs play, and does the account-number retry pick the right account? | Connect a service, play a song in one room, watch for "plays with account N; saved" or "didn't start" | High |
| How far apart do S1 and S2 rooms drift with *play along*? | Link an S1 and an S2 room near each other; try a radio station and a library song | Medium |
| Do *Check for updates* and *Update all speakers* behave as expected? Is one `BeginSoftwareUpdate` per system enough? | Check for updates (changes nothing). Install only on a system you can afford to restart | Medium |
| Line-in/TV detection from model names | Try every model you own; note which are wrong | Low |
| Windows and Linux by hand (discovery through the firewall, tray behaviour, file dialogs) | Run the app on each | Medium |
| *Checked so far (read-only, one S1 system of 5 Connect:Amp rooms, 2026-11-13)*: speaker updates subscribe, notify (about 0.5 s), renew and cancel on all 5 rooms; saving a backup and exporting both real playlists as M3U and CSV works and reads back; a Spotify song on S1 reports only the MIME type `audio/x-spotify` (no sample rate, bit depth or bitrate). Run them with `ROOMTONE_SPEAKER=<ip> cargo test <name> -- --ignored --nocapture` | | |
| **Restoring a backup** (0.5.4): does *Restore* create Sonos favorites, playlists (`CreateSavedQueue` / `AddURIToSavedQueue`) and alarms correctly, on S1 and on S2, and between them? | On a system you can spare: save a backup, delete a test playlist and favorite, restore, and check they're back. Saving was only checked read-only on one S1 system | High |
| **Podcast resume** (0.5.4): do speakers accept a seek about 1.5 s after an episode starts, so *Resume where you left off* works? | Play an episode for a minute, stop, play it again | Medium |
| **Sound quality** (0.5.4): what do S2 speakers send for sample rate, bit depth and bitrate? Only the format is confirmed (S1 library track: MIME type) | Play a song on an S2 speaker, open Now Playing › Technical details › Copy details, and attach it to a Speaker report | Medium |
| **Media keys and Now Playing** (0.6.0): do the keyboard keys, headphone buttons and the system's Now Playing show and control the selected room? Only starting up was checked, on macOS | Play something, press the keys, look at Control Center, the Windows media overlay or an MPRIS widget (`playerctl`) | Medium |
| **S2 check** (0.6.0): what do real Connect, Connect:Amp and Play:5 speakers report so the first and second generations can be told apart? One S1 Connect:Amp reported *Check* | Settings › Software updates › *Check my speakers*; attach the result to a Speaker report | Medium |
| **New-version check** (0.6.0): does it read GitHub's latest release correctly on every system, including behind a proxy? | Switch it on in Settings › New versions and press *Check now* | Low |
| **Connection type** (0.6.2): is *Wired* / *Wireless* right on every model, and on Wi-Fi-only speakers? Only one wired S1 Connect:Amp has been checked (reports Wired) | Open *Your speakers*; compare with how each speaker is actually connected. `ROOMTONE_SPEAKER=<ip> cargo test real_speaker_connection -- --ignored --nocapture` | Low |
| **Speaker updates** (events): S2 speakers, grouped rooms, bonded pairs, and a speaker restarting. Checked once on one S1 speaker (3 subscriptions, ~5 s to first notice, 1800 s lease) | Switch it on in Settings and watch the status line while changing things from another app | Medium |

## Known limits (by design, for now)

- S1 and S2 can't truly group; *play along* is the stand-in.
- Apple Music, YouTube Music and SoundCloud only play saved favorites (the providers block third-party sign-in).
- *Play along* links aren't saved between runs.
- "Automatic" language on Windows reads only environment variables.
- Translations need native-speaker review (de, fr, es, it, nl).
- The details under a restore summary (why something couldn't be restored) are in English only.
- Importing an M3U or CSV matches songs only to your music folders (Music Library › Add folder); songs from streaming services can't be brought across from other apps' lists.

## Maintenance

- After an egui upgrade, compare `--screenshot` pictures of every page before and after (`--view home`, `search`, `favorites`, `library`, `radio`, `services`, `inputs`, `alarms`, `settings`, `nowplaying`, and `ROOMTONE_TEST_EDITOR=1` for the group editor). Pop-up menus (⋯ on a row, Sleep timer) and the file picker need a look by hand.

## Ideas

Roughly in order of value. Discuss in an issue before starting the big ones.

1. **Signed packaging (parked)**: the macOS `.dmg`, Windows setup and Linux `.deb`/`.rpm` attach to each release but aren't signed or notarised (Apple Developer ID, Windows code signing), so macOS and SmartScreen warn on first run. **Parked until there's real user adoption**, because signing has running costs (the Apple Developer Program is about US$99 a year; Windows is free through SignPath Foundation if accepted, otherwise paid). The README explains how to get past the warnings meanwhile. Free steps that need no accounts (checksums and build attestations on each release), an Intel Mac build and Linux AppImage/Flatpak can be done at any time.
2. **Backup follow-ups**: bring a streaming-service favorite across when the account number differs, and try restoring on real S1 and S2 speakers (it's only tested on a pretend system and the demo so far). (Playlists as M3U, PLS, XSPF and CSV, and `roomtone backup` / the `save_backup` assistant tool, shipped in 0.6.1; command-line restore stays out until restore is proven.)
3. **Phone-friendly web remote**: serve a small responsive page from the headless HTTP server (protected by the access key), so any phone, tablet or computer can control Roomtone without an app. Drives the same commands as the window and the assistant link. Open questions: how to keep the access key safe on a phone, and whether to show it as a QR code in Settings.
4. **Scenes**: one click does several things at once: group these rooms, set each room's volume, choose the source and EQ or night sound, and optionally start a sleep timer ("Movie night", "Dinner", "Good night"). Builds on saved groups; also available from the command line and to assistants. Would also add a `scene` command to the command line.
5. **Menu-bar or tray mini player**: an optional small player in the menu bar or system tray. (Media keys, Now Playing and the remembered window size shipped in 0.6.0.)
6. **More streaming services, and more for the favorites-only ones**: add services beyond Spotify, Amazon Music, Deezer, TIDAL and Qobuz, and find legitimate ways to do more than play saved favorites with Apple Music, YouTube Music and SoundCloud (browsing and search where a service allows it). Open question: which services let a third-party controller sign in at all.
7. **Package managers**: a Homebrew cask, a winget manifest and Flathub/AUR entries, so Roomtone can be installed and updated with the usual tools (signing helps; see the first item).
8. **Gentler alarms**: a volume fade-in, snooze, alarms in several rooms at once, duplicating an alarm, and one-off timers per room.
9. **Lyrics (maybe)**: show lyrics for what's playing. Needs an outside lyrics source, so it would be opt-in and add a network destination; it may not be worth it.
10. **Saved play-along links**, and play along for more than two systems.
11. **More languages**: see "Adding a language" in [CONTRIBUTING.md](../CONTRIBUTING.md).
12. **Home-theatre settings**: surround and sub levels, where the speakers allow it locally.
13. **Accessibility pass**: keyboard navigation through every page, screen-reader labels (egui's AccessKit support).
14. **Recorded replies from real speakers** as test cases (with names and addresses removed), to complement the hand-written samples in the unit tests.

