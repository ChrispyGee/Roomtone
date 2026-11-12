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
5. Commit, tag and push: `git tag vX.Y.Z && git push --tags`.
6. Build on each platform: `cargo build --release`; the program is `target/release/roomtone` (`roomtone.exe` on Windows). Attach the files to a GitHub release, with the changelog section as notes.

Packaged installers (a macOS `.app`/`.dmg`, a Windows installer, Linux AppImage/Flatpak) and release builds from CI are on the [roadmap](ROADMAP.md).
