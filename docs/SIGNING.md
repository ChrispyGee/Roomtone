# Signing Roomtone for Windows

**Status (2026-11-23): prepared, not started with any service.** Nothing is signed yet. This page is for the maintainer: what is ready, what needs an account in the maintainer's own name, and what changes in the build once an account exists. The larger plan (including macOS and Linux) is the maintainer's own planning notes; [ROADMAP.md](ROADMAP.md) item 1 is the public summary.

## What signing does and doesn't do

A signature makes Windows show the publisher's name instead of "Unknown publisher" and is what lets a file build a reputation with SmartScreen. It does **not** remove the "Windows protected your PC" warning on day one: Microsoft's own pages say reputation comes from how many people have run a file, and an EV certificate no longer skips that. Keep the first-run steps in [FIRST_RUN.md](FIRST_RUN.md) until the warning is gone in practice.

## The two routes being started

| | SignPath Foundation | Microsoft Store |
| --- | --- | --- |
| What it gives | Free signing of `roomtone.exe` and the setup program, from GitHub Actions | Microsoft signs the package; installs and updates through the Store, with no SmartScreen warning |
| Who is named | **SignPath Foundation** is the publisher; the project must credit them | The maintainer (or an organisation), as registered with Microsoft |
| Cost | Free | Free to register (individuals since September 2025, companies since May 2026; Microsoft's fees page may still show older prices) |
| Needs | An application and approval; a code signing policy and privacy statement (done: [CODE_SIGNING_POLICY.md](../CODE_SIGNING_POLICY.md), [PRIVACY.md](../PRIVACY.md)); two-step login for everyone with access | A developer account with an ID and selfie check; the name "Roomtone" reserved; a Store package (done: `packaging/windows/msix/`) and a listing; certification review |
| Ongoing work | Two signing requests per release | A Store submission per release (an update goes through review each time) |

They don't exclude each other: the Store is a separate channel from the downloads on GitHub, which SignPath would sign.

## SignPath Foundation: what the maintainer does

1. **Read their current conditions and apply** at <https://signpath.org/> (the "apply" link). Their rules have changed over time; check the current page rather than this one. As far as the project can tell from other projects' write-ups, they ask for: a public repository under an OSI-approved licence with **no proprietary components**, builds done by automation from the source, a code signing policy page (done), a privacy statement (done), and **two-step login for everyone** with access to the repository or SignPath.
2. **Licence question to ask in the application:** Roomtone is licensed **MIT OR Apache-2.0** (the user picks one). Some write-ups say "no dual licensing"; the likely meaning is open-source plus commercial, not two open licences, but ask.
3. **Draft of the application text** (adapt it):
   > Roomtone is a free, open-source desktop app (Rust, MIT OR Apache-2.0) that controls Sonos speakers over the local network, with a headless mode and an MCP server for AI assistants. Repository: https://github.com/ChrispyGee/Roomtone. Releases are built only by GitHub Actions from a version tag (`.github/workflows/release.yml`) with locked dependencies; the Windows program is `roomtone.exe` and a setup program made with Inno Setup. There are no proprietary components and no telemetry (see PRIVACY.md). Our code signing policy is CODE_SIGNING_POLICY.md. The sole maintainer approves releases; two-step login is on for the repository. We would like to sign `roomtone.exe` and the setup program for each release.
4. **Turn on two-step login** for the GitHub account, and protect `v*` tags so only the maintainer can create them (Settings › Rules).

## SignPath: what changes in the build once accepted

Not written yet, because it needs the organisation and project names from SignPath and can't be tested without an account. The shape, to check against SignPath's own documentation then:

1. In `.github/workflows/release.yml`, the Windows job uploads the unsigned `roomtone.exe` as an artifact, calls SignPath's GitHub action to sign it (the job waits for the result), then builds the setup program from the **signed** program, uploads that and signs it the same way.
2. SignPath needs a small file describing what is inside each signed artifact (an *artifact configuration*), kept in the repository.
3. The organisation id and a token are stored as secrets on a protected `release` environment (only runs from `v*` tags can use them), never in the repository.
4. Add SignPath's credit line to `CODE_SIGNING_POLICY.md`, the README and the download page, and update [FIRST_RUN.md](FIRST_RUN.md) once the warning is gone.
5. Verify in CI with `signtool verify /pa` on both files, and check the result on a clean Windows PC.

Allow about a day of work after approval.

## Microsoft Store

Everything the maintainer needs to do, and the listing text, is in [MICROSOFT_STORE.md](MICROSOFT_STORE.md). The package itself (`packaging/windows/msix/`) is built by the *Microsoft Store package* workflow to check it is well formed. It has not been installed or submitted.

## Not covered here

macOS (Apple Developer ID and notarisation, US$99 a year) and Linux (checksums, attestations) are in the maintainer's plan and the roadmap. A commercial certificate, Azure's signing service and Certum are the fallbacks if SignPath says no; the trade-offs were discussed on 2026-11-23 and are summarised in the maintainer's notes.
