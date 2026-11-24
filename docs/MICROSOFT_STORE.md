# Roomtone in the Microsoft Store

**Status (2026-11-24): certification passed and the maintainer published it in Partner Center; the listing is at https://apps.microsoft.com/detail/9MXNHZ6K9NJQ once it is live.** The package has been through the Store's review. This page is for the maintainer.

**Roomtone's Store identity** (public, from Partner Center): package name `ChrispyGee.Roomtone`, publisher `CN=7B70B259-0799-4538-B08C-67D102EE553E`, publisher display name `ChrispyGee`, Store ID `9MXNHZ6K9NJQ`. They are set in `.github/workflows/msix.yml`, so the *Microsoft Store package* workflow (Actions › Run workflow) makes the file to upload, as the artifact `msix-check`.

## Why it's worth doing

Microsoft signs the package, so people who install from the Store get no "unknown publisher" or SmartScreen warning, one-click install and automatic updates. It is also where many Windows users look first. It is a **separate channel** from the GitHub downloads (which stay), so it adds a release step rather than replacing one.

## What's ready

- `packaging/windows/msix/AppxManifest.xml`: the package definition, a normal desktop program ("full trust", not sandboxed) that can reach the internet, listen on the home network and serve files to the speakers. It also registers the `roomtone` command so assistants and scripts can run `roomtone --mcp` (the Store hides the install folder).
- `packaging/windows/msix/build-msix.ps1`: builds the `.msix` from a release build. Without the Partner Center values it makes a placeholder package for checking only.
- `.github/workflows/msix.yml`: builds it in CI when those files change, or by hand (Actions › Microsoft Store package › Run workflow), and keeps the file as an artifact.
- [PRIVACY.md](../PRIVACY.md): the privacy policy the Store asks for (a web address).

## What the maintainer does

1. **Register** at <https://storedeveloper.microsoft.com> with a personal Microsoft account. As an individual this needs an ID and a selfie check and no payment (check what the page asks today). The **name shown next to the app** is the maintainer's legal name unless an organisation account is made.
2. **Reserve the name "Roomtone"** in Partner Center (*Apps and games › New product › MSIX or PWA app*). If it's taken, pick another and tell me; the display name in the manifest and listing change.
3. Copy three values from *Product management › Product identity*: the **Package/Identity/Name**, the **Package/Identity/Publisher** (like `CN=…`) and the **publisher display name**.
4. Build the package with them (from a release build):
   ```powershell
   pwsh packaging/windows/msix/build-msix.ps1 -Version 0.9.4 -IdentityName "<Name>" -Publisher "<CN=…>" -PublisherDisplay "<display name>"
   ```
   or give them to me and I'll add them as repository variables and make the workflow produce the submission file. They are public (they appear in the package), so they're not secrets.
5. **Fill in the listing** with the text below, upload screenshots (use the ones in `docs/images/`), set the age rating and submit the `.msix`. The Store signs it, so it is uploaded unsigned.
6. Each release afterwards: upload a new `.msix` with the new version; it goes through review again.

## Listing text (starting point)

**Name:** Roomtone

**Short description:** A free, open-source controller for all your Sonos speakers.

**Description:**
> Control all your Sonos speakers from your PC, with no account of its own and no tracking.
>
> • Every room on one screen: play, pause, skip, volume and groups, with drag and drop.
> • Browse and search your favorites, playlists, radio, podcasts, your own music folders and the streaming services you already use.
> • Scenes, alarms, volume limits and room sound settings.
> • Works with both older (S1) and current (S2) speaker systems.
> • An optional icon next to the clock for quick control, a command-line mode, and a link for AI assistants (off until you switch it on).
> • Free and open source. Your data stays on your computer.
>
> Roomtone talks to your speakers directly on your home network. It is not affiliated with or endorsed by Sonos, Inc. Sonos is a trademark of Sonos, Inc.

**Search terms (US English; the Store takes up to 7, each up to 30 characters):** sonos controller, sonos speakers, speaker remote, multiroom audio, music controller, wireless speakers, sonos s1 s2. "Sonos" is a trademark: the description says Roomtone is not affiliated with it, but if certification objects to it in the search terms, replace the first, second and last with `smart speaker controller`, `home audio control` and `speaker groups`. The other languages' terms are in `packaging/windows/msix/listing/descriptions.md`.
**Category:** Music (or Utilities & tools)
**Privacy policy URL:** `https://github.com/ChrispyGee/Roomtone/blob/main/PRIVACY.md`
**Support:** `https://github.com/ChrispyGee/Roomtone/issues`
**Website:** `https://github.com/ChrispyGee/Roomtone`
**Languages:** English, German, Spanish, French, Italian, Dutch (the app is translated into these)
**Age rating:** likely suitable for everyone (the app has no content of its own); answer the questionnaire honestly about links to streaming services.

**Notes for certification** (the free-text box for reviewers):
> Roomtone is a desktop program packaged with the full trust capability because it must listen for notices from speakers on the user's home network and serve the user's own music files to those speakers over HTTP; it cannot run inside the app sandbox. It requests the network capabilities for this reason only. It sends no telemetry. The source code is public at https://github.com/ChrispyGee/Roomtone and its network connections are listed in SPEC.md section 9. No account is needed to test it: with no speakers on the network it offers a demo system with made-up rooms (Settings › Speakers › Demo system).

## Images for the listing

All made from the demo system (made-up rooms and music), 2640 by 1680 pixels, which the Store accepts (it wants at least 1366 by 768, and at most 3840 by 2160). Upload them in this order, with these captions:

| File | Caption |
| --- | --- |
| `docs/images/roomtone-home.png` | Every room and what's playing, on one screen |
| `docs/images/roomtone-now-playing.png` | The player: cover, queue and volume |
| `docs/images/roomtone-scenes.png` | Scenes: one click sets up your rooms |
| `docs/images/roomtone-home-light.png` | Light and dark, to match your PC |

The **store logo** (300 by 300, the Store's "1:1 box art") is `packaging/windows/msix/listing/store-logo-300.png`, the same icon as the app. The 2:3 poster and 16:9 hero images are optional; skip them for the first submission.

## What could go wrong

- **The name** may be taken, or the Sonos mention in the description may draw a trademark question. The wording above says it is not affiliated; keep it.
- **Certification** may question the full-trust capability or the local network listening. The notes above explain; be ready for a round or two of replies. A real review has not been tried.
- **Settings are kept per package.** In a Store install, Windows redirects the program's settings folder into the package's own storage, so a Store copy does not see the settings of a copy installed from GitHub, and they are removed when the Store copy is uninstalled. Say so in the listing notes if people ask. (Backups in Settings › Backups still work.)
- **The `roomtone` command** only exists for the Store copy under that name; assistant configurations that point at a file path will need `roomtone` instead.
- **Both copies on one PC** would fight over the same speaker ports. Say "use one" in the first-run notes if it comes up.
- **Updates** go through review each time, so the Store version can trail the GitHub one by days.
- **Not tried on a real PC**: installing the package (it needs the Store's signing or a test certificate), the tray icon inside a package, and the file serving.
