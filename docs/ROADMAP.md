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

## Known limits (by design, for now)

- S1 and S2 can't truly group; *play along* is the stand-in.
- Apple Music, YouTube Music and SoundCloud only play saved favorites (the providers block third-party sign-in).
- *Play along* links aren't saved between runs.
- "Automatic" language on Windows reads only environment variables.
- Translations need native-speaker review (de, fr, es, it, nl).

## Maintenance

- After an egui upgrade, compare `--screenshot` pictures of every page before and after (`--view home`, `search`, `favorites`, `library`, `radio`, `services`, `inputs`, `alarms`, `settings`, `nowplaying`, and `ROOMTONE_TEST_EDITOR=1` for the group editor). Pop-up menus (⋯ on a row, Sleep timer) and the file picker need a look by hand.

## Ideas

Roughly in order of value. Discuss in an issue before starting the big ones.

1. **Signed packaging**: the macOS `.dmg`, Windows setup and Linux `.deb`/`.rpm` already attach to each release but aren't signed or notarised (Apple Developer ID, Windows code-signing certificate), so macOS and SmartScreen warn on first run. Also an Intel Mac build, and Linux AppImage/Flatpak.
2. **Backup follow-ups**: back up and restore from the command line and for assistants, export playlists as PLS or XSPF for other apps (M3U and CSV are done), bring a streaming-service favorite across when the account number differs, and try restoring on real S1 and S2 speakers (it's only tested on a pretend system and the demo so far).
3. **Phone-friendly web remote**: serve a small responsive page from the headless HTTP server (protected by the access key), so any phone, tablet or computer can control Roomtone without an app. Drives the same commands as the window and the assistant link. Open questions: how to keep the access key safe on a phone, and whether to show it as a QR code in Settings.
4. **Scenes**: one click does several things at once: group these rooms, set each room's volume, choose the source and EQ or night sound, and optionally start a sleep timer ("Movie night", "Dinner", "Good night"). Builds on saved groups; also available from the command line and to assistants.
5. **"Can't find my speakers" helper and diagnostics**: a guided check (Local Network permission, same network, firewall, separate guest network, adding an address by hand), a speaker status page with connection type (wired or Wi-Fi), signal, software version and address, and a *Copy speaker report* button that redacts names and addresses and fills the `speaker-report` issue form.
6. **System media controls and a smaller footprint**: keyboard media keys, headphones and the lock screen control the selected group (macOS Now Playing, Windows media controls, Linux MPRIS); an optional menu-bar or tray mini player; and the window remembers its size and position.
7. **Saved play-along links**, and play along for more than two systems.
8. **More languages**: see "Adding a language" in [CONTRIBUTING.md](../CONTRIBUTING.md).
9. **Home-theatre settings**: surround and sub levels, where the speakers allow it locally.
10. **Accessibility pass**: keyboard navigation through every page, screen-reader labels (egui's AccessKit support).
11. **Recorded replies from real speakers** as test cases (with names and addresses removed), to complement the hand-written samples in the unit tests.
12. **MCP change notifications**: let assistants subscribe to the now-playing and queue resources.
