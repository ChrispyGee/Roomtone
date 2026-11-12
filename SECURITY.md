# Security policy

Roomtone controls speakers on your home network and can run a local server for AI assistants, so security reports matter to us.

## Reporting a vulnerability

**Please don't open a public issue.** Use GitHub's private reporting instead: [report a vulnerability](https://github.com/ChrispyGee/Roomtone/security/advisories/new) (the repository's **Security** tab › **Report a vulnerability**). Include:

- what an attacker could do, and from where (this computer, the home network, a web page, the internet);
- steps to reproduce, with the version (`roomtone --help` shows it) and platform;
- any logs, with room names, addresses and keys removed.

We aim to acknowledge reports within a week and to fix serious issues in the next release. We'll credit you unless you'd rather we didn't.

## Supported versions

Only the latest release gets security fixes.

## What's in scope

- **The assistant link (MCP)**: authentication (the access key), the `Origin` and `Host` checks, look-only mode, the on/off switch, and anything that lets a web page or another computer reach it when it shouldn't.
- **The music server** on port 52880: serving files outside the folders the person added.
- **Stored secrets**: sign-in tokens and the access key in `config.json`.
- Anything that could install the wrong software on speakers, or move an S1 system to S2.

## Out of scope

- The speakers' own unauthenticated local control API (port 1400). Anyone on the same network can already use it; that's how the speakers work.
- Problems in the streaming services themselves.

## Design notes

The security model is in [SPEC.md §6 and §9](SPEC.md#9-network-security-and-privacy): the assistant link is off by default, listens on 127.0.0.1 unless other computers are allowed, always requires the key over HTTP, refuses browser origins, and can be limited to looking only.
