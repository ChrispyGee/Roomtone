# Releasing

For maintainers.

## First upload to GitHub

1. The repository is <https://github.com/ChrispyGee/Roomtone>. If it's new and empty, don't add a README, licence or .gitignore on GitHub; they're already here.
2. Every commit records the author's name and email, and GitHub shows them publicly. To keep your email private, turn on *Keep my email addresses private* in GitHub › Settings › Emails, then use the `…@users.noreply.github.com` address it shows:
   ```sh
   git config user.name "Your Name or handle"
   git config user.email "12345678+you@users.noreply.github.com"
   ```
3. From this folder (the one with `Cargo.toml`):
   ```sh
   git init -b main
   git add .
   git status            # check: no shots/, target/ or personal files listed
   git commit -m "Roomtone 0.5.0"
   git remote add origin https://github.com/ChrispyGee/Roomtone.git
   git push -u origin main
   ```
4. On GitHub:
   - **Settings › General**: add a description and the topics `speakers`, `egui`, `rust`, `mcp`, `upnp`.
   - **Settings › Code security**: turn on *Private vulnerability reporting* (SECURITY.md points to it), Dependabot alerts and Dependabot security updates.
   - **Settings › Branches**: protect `main`; require the *CI* checks and one review.
   - **Settings › General › Features**: turn on *Discussions* (the issue page links to it for questions).
   - **Issues › Labels**: make sure `bug`, `enhancement` and `speaker-report` exist (the issue forms use them).
5. Check the README badges and links work once the first CI run has finished.

## Uploading later changes

From this folder (the one with `Cargo.toml`):

```sh
git status                       # see what changed; nothing from shots/ or target/ should be listed
git add -A
git commit -m "Short description of the change"
git push
```

**Keep the login out of the remote address.** If `git remote -v` shows something like `https://oauth2:gho_…@github.com/…`, a GitHub token is stored in plain text in `.git/config`. Replace it, let the GitHub CLI handle the login, and revoke the old token on GitHub (Settings › Applications › Authorized OAuth Apps, or Settings › Developer settings › Personal access tokens):

```sh
git remote set-url origin https://github.com/ChrispyGee/Roomtone.git
gh auth login        # once, if the GitHub CLI isn't signed in yet
gh auth setup-git    # git then uses the GitHub CLI's login
```

After pushing, Dependabot closes its own pull requests when the same update is already on `main`.

## Making a release

1. Make sure `main` is green in CI.
2. Pick the version ([Semantic Versioning](https://semver.org/)): bump `version` in `Cargo.toml` and run `cargo build`, so `Cargo.lock` updates.
3. In `CHANGELOG.md`, move *Unreleased* items under a new `## [x.y.z] — YYYY-MM-DD` heading.
4. Point the README's download links at the new version with `scripts/set-download-links.sh X.Y.Z` (a unit test fails if they and `Cargo.toml` disagree; the links work once the release exists, a few minutes after the pull request merges). Update the version in the title of `SPEC.md`, and the "What's new in this version" list in Settings › About (`src/ui/pages.rs`, `about_card`), translated.
   **Keep every document up to date in the same release, every time**, not only the version numbers: `SPEC.md` (behaviour, network table, privacy lines, what's verified in §10), `docs/ROADMAP.md` (move what shipped out of *Ideas*, update what's been checked on real speakers), `docs/SETTINGS.md`, `docs/ARCHITECTURE.md` when files or flows changed, `README.md` (features, privacy, download links) and `CHANGELOG.md`. Read each one against what the release actually contains before opening the pull request.
5. Commit the version change through a pull request, wait for CI, and merge it.
6. Tag the merge commit and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
7. The *Release* workflow (`.github/workflows/release.yml`) builds macOS (Apple silicon), Windows and Linux programs, and for each platform makes an installer: a `.dmg` (`packaging/macos/build-dmg.sh`), a Windows setup `.exe` (Inno Setup, `packaging/windows/roomtone.iss`) a `.deb` (cargo-deb) and an `.rpm` (cargo-generate-rpm), with their settings in `Cargo.toml`. It attaches them and the `.zip`/`.tar.gz` to a GitHub release, with that version's `CHANGELOG.md` section as notes. It fails if the changelog has no section for the tag. Every release's notes end with `.github/release-footer.md` (the first-run steps for the unsigned apps, linking to `docs/FIRST_RUN.md`); keep both in step with what macOS and Windows currently ask for.
8. To try the installers without releasing, run the workflow by hand (Actions › Release › Run workflow) on a branch: the files are kept as workflow artifacts and no release is made.
9. To build by hand instead: `cargo build --release`; the program is `target/release/roomtone` (`roomtone.exe` on Windows). On a Mac, `packaging/macos/build-dmg.sh <version> <out.dmg>` makes the `.dmg`.

**Mac signing and notarisation.** When these five repository secrets are set (*Settings › Secrets and variables › Actions*), the *Release* workflow signs the Mac program and `.dmg` with the Developer ID certificate (hardened runtime, secure timestamp), has Apple notarise the `.dmg` and staples the ticket, so it opens with no warning. Without them it signs ad hoc as before, so forks and test runs still work.

| Secret | What it is |
| --- | --- |
| `MAC_CERT_P12` | The *Developer ID Application* certificate and its private key, exported from Keychain Access as a `.p12`, then `base64 -i cert.p12 \| pbcopy` |
| `MAC_CERT_PASSWORD` | The password chosen when exporting the `.p12` |
| `MAC_NOTARY_APPLE_ID` | The Apple ID's email |
| `MAC_NOTARY_PASSWORD` | An app-specific password made at account.apple.com › Sign-In and Security |
| `MAC_TEAM_ID` | The 10-character Team ID (developer.apple.com › Membership details) |

The certificate lasts five years and the program membership renews yearly (note the dates here when set up). Windows installers aren't signed yet; see the [roadmap](ROADMAP.md) for signing, an Intel Mac build and AppImage/Flatpak. The preparation for Windows signing (SignPath Foundation) and the Microsoft Store is in [SIGNING.md](SIGNING.md) and [MICROSOFT_STORE.md](MICROSOFT_STORE.md); neither is part of the release yet.

## Package managers (Homebrew, winget, Arch)

Each release also attaches `SHA256SUMS` (a checksum for every file) and `roomtone-vX.Y.Z-package-manifests.tar.gz`, made by `packaging/package-managers/render.sh` from the templates in `packaging/package-managers/`:

| Channel | Files | Where they go |
| --- | --- | --- |
| Homebrew (macOS) | `homebrew/roomtone.rb`, a cask | The tap `ChrispyGee/homebrew-roomtone` (created for 0.6.1; `brew install --cask ChrispyGee/roomtone/roomtone`). For each release, replace `Casks/roomtone.rb` there with the new file and commit. The main `homebrew-cask` repository asks for a notarised app, so it has to wait for signing |
| winget (Windows) | `winget/*.yaml`, three files | A pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) under `manifests/c/ChrispyGee/Roomtone/<version>/` (`wingetcreate` or `winget validate` checks it first) |
| Arch User Repository | `aur/PKGBUILD` (`roomtone-bin`) | The AUR package `roomtone-bin`: `git clone ssh://aur@aur.archlinux.org/roomtone-bin.git`, copy `PKGBUILD`, run `makepkg --printsrcinfo > .SRCINFO`, commit and push. Fill in the maintainer line first |

Nothing is published automatically: each of these puts the project's name on someone else's service and needs an account, so a person does it, once per release, after the release is out and its files are downloadable. To make the files by hand for any release: `gh release download vX.Y.Z -p '*' -D dist && (cd dist && sha256sum -- * > ../SHA256SUMS) && packaging/package-managers/render.sh X.Y.Z SHA256SUMS out`.

Windows installers aren't signed, and the Mac one isn't until the secrets above are set, so they still ask for confirmation the first time whichever way Roomtone is installed.
