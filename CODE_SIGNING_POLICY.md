# Code signing policy

**Status (2026-11-24): the Mac download is signed with an Apple Developer ID and notarised by Apple (from 0.9.6); Windows users get a Microsoft-signed build from the [Microsoft Store](https://apps.microsoft.com/detail/9MXNHZ6K9NJQ); the Windows installer and the Linux packages on GitHub are not code-signed.** This page is the policy we follow for signed builds, and the one we will follow if the GitHub installer is signed later (see [docs/SIGNING.md](docs/SIGNING.md)). For the unsigned files, the first-run steps in [docs/FIRST_RUN.md](docs/FIRST_RUN.md) apply, and every release carries a `SHA256SUMS` file.

## What gets signed

Windows program files built by this project's own automated release workflow from the source in this repository: `roomtone.exe` and the Windows setup program. Nothing else is signed, and nothing built anywhere else is signed.

## How it is built

- Releases are built **only by GitHub Actions** ([`.github/workflows/release.yml`](.github/workflows/release.yml)) from a version tag in this public repository, so a signed file can always be traced back to the exact source it was built from.
- The build uses locked dependencies (`cargo build --release --locked`). No binary is added to the build by hand.
- Signing requests are made by the workflow only for tags created by the maintainer, and release approval is a manual step.

## Roles

| Role | Who |
| --- | --- |
| Committers and reviewers | The project maintainer ([ChrispyGee](https://github.com/ChrispyGee)) and contributors whose pull requests the maintainer reviews and merges |
| Approvers (may approve a signing request) | The project maintainer |

Everyone with write access to the repository or any signing service uses **multi-factor authentication**. If more maintainers join, they are listed here.

## Privacy

See [PRIVACY.md](PRIVACY.md). In short: this program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

## Attribution

Once signing is provided by a service that asks for credit, this page and the download page will carry it, for example: "Free code signing provided by SignPath.io, certificate by SignPath Foundation."

## Reporting a problem with a signed file

If you think a signed Roomtone file is not what it should be, follow [SECURITY.md](SECURITY.md).
