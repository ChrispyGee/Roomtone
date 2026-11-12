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

- **Upgrade egui/eframe** from 0.31 to the current release (Dependabot #2: 0.37). It doesn't build as is: egui changed several APIs between these versions (panels, frames, rounding, modals, viewport options). Do it in one focused pull request, then compare `--screenshot` pictures of every page before and after.

## Ideas

Roughly in order of value. Discuss in an issue before starting the big ones.

1. **Packaging**: a signed macOS `.app`/`.dmg` (gives a proper bundle name and Local Network prompt text), a Windows installer, and Linux AppImage/Flatpak. Release builds from CI.
2. **Speaker-side events** (UPnP `SUBSCRIBE`) instead of polling, for instant updates and less network traffic.
3. **Saved play-along links**, and play along for more than two systems.
4. **More languages**: see "Adding a language" in [CONTRIBUTING.md](../CONTRIBUTING.md).
5. **Home-theatre settings**: surround and sub levels, where the speakers allow it locally.
6. **Accessibility pass**: keyboard navigation through every page, screen-reader labels (egui's AccessKit support).
7. **Unit tests** for the protocol parsers (`net::didl`, topology, `CheckForUpdate` replies) using recorded speaker replies, with any personal names and addresses removed.
8. **MCP resources**: expose now-playing and the queue as MCP resources with change notifications.
9. **Light theme**.
