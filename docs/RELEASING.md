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
4. Update the version in the title of `SPEC.md`, and the "What's new in this version" list in Settings › About (`src/ui/pages.rs`, `about_card`), translated.
5. Commit the version change through a pull request, wait for CI, and merge it.
6. Tag the merge commit and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
7. The *Release* workflow (`.github/workflows/release.yml`) builds macOS (Apple silicon), Windows and Linux programs, and for each platform makes an installer: a `.dmg` (`packaging/macos/build-dmg.sh`), a Windows setup `.exe` (Inno Setup, `packaging/windows/roomtone.iss`) a `.deb` (cargo-deb) and an `.rpm` (cargo-generate-rpm), with their settings in `Cargo.toml`. It attaches them and the `.zip`/`.tar.gz` to a GitHub release, with that version's `CHANGELOG.md` section as notes. It fails if the changelog has no section for the tag.
8. To try the installers without releasing, run the workflow by hand (Actions › Release › Run workflow) on a branch: the files are kept as workflow artifacts and no release is made.
9. To build by hand instead: `cargo build --release`; the program is `target/release/roomtone` (`roomtone.exe` on Windows). On a Mac, `packaging/macos/build-dmg.sh <version> <out.dmg>` makes the `.dmg`.

The installers aren't signed or notarised; see the [roadmap](ROADMAP.md) for signing, an Intel Mac build and AppImage/Flatpak.

## Package managers (Homebrew, winget, Arch)

Each release also attaches `SHA256SUMS` (a checksum for every file) and `roomtone-vX.Y.Z-package-manifests.tar.gz`, made by `packaging/package-managers/render.sh` from the templates in `packaging/package-managers/`:

| Channel | Files | Where they go |
| --- | --- | --- |
| Homebrew (macOS) | `homebrew/roomtone.rb`, a cask | A personal tap, a GitHub repository named `homebrew-roomtone` (then `brew install --cask <owner>/roomtone/roomtone`). The main `homebrew-cask` repository asks for a notarised app, so it has to wait for signing |
| winget (Windows) | `winget/*.yaml`, three files | A pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) under `manifests/c/ChrispyGee/Roomtone/<version>/` (`wingetcreate` or `winget validate` checks it first) |
| Arch User Repository | `aur/PKGBUILD` (`roomtone-bin`) | The AUR package `roomtone-bin`: `git clone ssh://aur@aur.archlinux.org/roomtone-bin.git`, copy `PKGBUILD`, run `makepkg --printsrcinfo > .SRCINFO`, commit and push. Fill in the maintainer line first |

Nothing is published automatically: each of these puts the project's name on someone else's service and needs an account, so a person does it, once per release, after the release is out and its files are downloadable. To make the files by hand for any release: `gh release download vX.Y.Z -p '*' -D dist && (cd dist && sha256sum -- * > ../SHA256SUMS) && packaging/package-managers/render.sh X.Y.Z SHA256SUMS out`.

The installers aren't signed, so macOS and Windows still ask for confirmation the first time whichever way Roomtone is installed.
