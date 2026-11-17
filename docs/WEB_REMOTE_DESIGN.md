# Web remote: design

**Status: proposal, nothing built.** Written 2026-11-17 for discussion. When a decision here is agreed, record it in this file; when something ships, move its behaviour into [SPEC.md](../SPEC.md) and the [CHANGELOG](../CHANGELOG.md), as the roadmap says.

## 1. What we're building

A small web page, served by Roomtone itself, that lets a phone, tablet or other computer on the home network control the speakers from a browser. Nothing to install on the phone. It works with the Roomtone window open or with Roomtone running headless (`--headless`), where today there is no way for a person to control it at all.

It is a third way to drive the same commands, next to the window and the assistant link (MCP). Every action on the page becomes a `Cmd` in `backend.rs`, so the demo system, real speakers, the window and `--headless` behave the same.

### Goals
- Controlling the music from the sofa is as quick as with the window: rooms, Now Playing, volume, queue, favorites, scenes.
- Safe by default: off until switched on, and no one on the same Wi-Fi can take control just by finding the address.
- Works on any recent phone browser, in the five languages, with screen readers and large text.
- No accounts, no cloud, no relay, no telemetry, no outside scripts or fonts.

### Non-goals
- Reaching Roomtone from outside the home. There is no relay and no help with port forwarding; the docs say not to expose the port to the internet. (A person who wants this uses their own VPN.)
- Anything irreversible or administrative: speaker software updates, restoring backups, adding accounts, changing settings, deleting playlists. Those stay in the window.
- A native phone app.
- Replacing the assistant link. It stays as it is.

### Who uses it
Someone on the sofa; a household member who just wants a simple remote; a headless setup (Mac mini, NAS, Raspberry Pi) where it is the only screen; a wall-mounted tablet showing Now Playing.

## 2. What exists today (and what that means)

- **Assistant link** (`src/mcp.rs`, SPEC §6): a hand-written HTTP/1.1 server on port 8790, one thread per connection. It requires `Authorization: Bearer <key>`, refuses any `Origin` that isn't localhost, and with *Allow other computers* off accepts only a local `Host`. Its only route is `/mcp`.
- **Why it can't just be reused:** a browser page is served from this computer's own address, so its `Origin` is that address, which the assistant link refuses on purpose ("web pages mustn't reach the speakers through this link"). A browser can't send a custom `Authorization` header from a plain link or bookmark either, and the key is a long random string that nobody wants to type on a phone.
- **Gaps in the current server that the remote must not inherit:**
  - The key is compared with `==` (not constant-time).
  - Wrong keys aren't slowed down or counted.
  - Reading is done one request per thread with a 15 s timeout and no cap on simultaneous connections.
  Each is acceptable for a local assistant link and not for a page that's reachable from the whole Wi-Fi. Fixing them for the remote should also fix them for the assistant link.
- **Helpful pieces already there:** the `Cmd` model, `AppState` snapshots, change detection for subscriptions (`mcp::notify::Watcher`) that can drive live updates, the *Allow other computers* switch, the access key in `config.json` (mode 600), and `new_token()`.

## 3. Threat model

The remote is a control surface for devices in people's homes, reachable from the local network. What we protect against, and how:

| Threat | Example | Answer |
| --- | --- | --- |
| Someone on the same Wi-Fi finds the port | A guest, a compromised smart TV | Off by default; every request needs a paired-device session; pairing needs a one-time code shown only on the computer running Roomtone |
| Guessing the pairing code or session | A script trying values | Long random values, short code lifetime, single use, per-address attempt limits with growing delays, constant-time comparison |
| A malicious web page makes the browser send requests (CSRF) | A page open in the same browser as the remote | `SameSite=Strict` cookie, every changing request must be `POST` with a custom header and a matching `Origin`, and no state change on `GET` |
| A web page reads the remote through the person's browser (DNS rebinding) | `evil.example` re-resolves to the computer's address | Check `Host` against an allow-list (the computer's addresses, its `.local` name, `localhost`); check `Origin` on every request that has one |
| Eavesdropping on shared Wi-Fi | Someone sniffing plain HTTP | Not fully solvable with plain HTTP; limits and warnings below (§5.6) |
| A lost or borrowed phone | | Paired devices can be listed and signed out; *New key* style reset signs everyone out; sessions expire |
| A guest abusing control | Changing volume, skipping | Optional *look only* devices; no destructive actions are exposed at all |
| Resource exhaustion | Opening thousands of connections | Connection cap, short timeouts, body size limit, no long-lived connections except the update stream with its own cap |
| Leaking private details | Room names or addresses in logs or URLs | Nothing sensitive in URLs; no access logs with addresses by default; no telemetry |

What isn't in scope: someone with access to the computer running Roomtone (they can read `config.json` anyway), and someone who can intercept and modify traffic on the network (that needs HTTPS; see §5.6).

## 4. Overview of the design

1. A **separate switch**, *Web remote*, in Settings, off by default. Separate from the assistant link, so one can be on without the other.
2. A **separate port** (default 8791, configurable) and its own listener, so the assistant link's rules stay exactly as they are. Both are served by the same small HTTP code, hardened as in §6.
3. A **pairing step**: the person opens a one-time pairing link (shown as text and a QR code in Settings) on the phone. The phone gets a **session cookie** and is listed as a paired device.
4. The page itself is a **single embedded file** (HTML, CSS, JS, no build step), plus a small JSON API under `/api/` that wraps the same commands.
5. Pages update through **polling**, upgradable to an event stream later.
6. Devices have a **role**: *Control* or *Look only*. Administrative actions don't exist on the web at all.

## 5. Security design

### 5.1 Switch and network exposure
- *Web remote* in Settings is off by default and can't be switched on from the web page.
- While off, nothing listens on the web remote port.
- The remote is useful only when other devices can reach it, so switching it on also listens on all interfaces (unlike the assistant link, whose *Allow other computers* is separate). The Settings text says this plainly: "Devices on your home network can ask to connect; each has to be approved here first." On first switch-on, a one-time note says not to use it on guest or public Wi-Fi.
- A *This computer only* option (listen on `127.0.0.1`) is available for testing and for people who reverse-proxy it themselves.
- Setting the port to a busy one fails with a plain message.

### 5.2 Pairing
Nobody should type a 32-character key on a phone, and the key must not live in the address bar.

1. In Settings › Web remote, *Add a device* shows a **pairing link** and a **QR code** for it, and a short **spoken-style code** (for example four short words) as a fallback for devices that can't scan.
2. The link has the form `http://<address>:8791/pair#<code>`. The code is in the **fragment**, which browsers don't send to the server or put in the `Referer`, so it doesn't appear in logs.
3. The page reads the fragment, removes it from the address bar, and `POST`s it to `/api/pair` (with a device name taken from the browser and editable).
4. The server checks the code in constant time. Codes are **single use**, expire after **5 minutes**, and only the most recent one is valid (showing a new code cancels the old one).
5. On success the server sets a **session cookie** and records the device. The pairing link is then dead.
6. The Settings window is the only place a code can be created, so a person must be at the computer. (Headless setups: `roomtone web-remote --pair` prints the link and an ASCII QR code in the terminal; see §8.)

### 5.3 Sessions and devices
- A session is a random 256-bit value. The server stores only a **hash** of it, in `config.json` (mode 600), with the device's name, role, when it was paired and when it was last seen.
- Cookie: `HttpOnly`, `SameSite=Strict`, `Path=/`, a long expiry (for example 90 days, sliding), and `Secure` only when HTTPS is in use.
- Settings lists **paired devices** with *Rename*, *Look only / Can control* and *Sign out*. *Sign out everyone* is one button. Signing a device out takes effect immediately.
- Sessions survive restarts of Roomtone (they're stored), but not a *Sign out everyone*.
- The remote never uses the assistant link's access key and the assistant link never accepts a remote session. A leak of one doesn't open the other.

### 5.4 Requests
- **Host check:** the `Host` header must be an address the computer actually has, `localhost`, or the computer's own `.local` name; anything else gets 403. This is the DNS-rebinding defence.
- **Origin check:** any request with an `Origin` header must match the page's own origin exactly; otherwise 403.
- **Changing requests** are `POST` only, with `Content-Type: application/json` and a custom header (for example `X-Roomtone: 1`) that a cross-site form can't send. `GET` never changes anything.
- **Constant-time comparison** for codes and session hashes.
- **Limits:** request bodies up to 16 KB, headers up to 16 KB, a read timeout of a few seconds, at most about 32 simultaneous connections (excess get 503), and a separate cap for event streams.
- **Throttling:** failed pairing and unauthenticated requests are counted per source address; after a few failures the address waits, with the wait growing (up to a minute), and the failures are shown in Settings ("3 failed attempts from a device on your network").
- **Response headers:** `Content-Security-Policy: default-src 'self'` (the page has only its own script and style, no inline), `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store` on the API, `X-Frame-Options: DENY`.
- **Errors** say what happened in words and never include speaker addresses or internal paths.

### 5.5 Roles and what's exposed
- **Control**: everything on the list in §7.
- **Look only**: the page shows everything but its controls are disabled, and the server refuses changing requests (not just hidden buttons).
- If the assistant link's *look-only* switch is on, that is independent; the remote has its own roles.
- **Never exposed**: speaker software updates, restoring or saving backups, adding or removing music accounts or folders, editing alarms (first version), renaming rooms, changing settings, the assistant link's key, and anything that reveals sign-in tokens. The API is a small allow-list, not a pass-through of `Cmd`.

### 5.6 Transport: HTTP versus HTTPS
Browsers will load plain `http://` pages on a local address, and that's what version one uses. The honest consequences:
- Anyone able to watch traffic on the Wi-Fi can read what the phone sees and steal the session cookie, and could control the speakers.
- So the Settings text and docs say: use it on your own home network, with a Wi-Fi password, not on guest or shared networks.

Options for a later version:
1. **Self-signed certificate** generated by Roomtone: encrypts, but phones show a scary warning the first time and some browsers refuse to install the PWA. Probably worse than the plain page for a household.
2. **Pin a certificate during pairing** isn't possible in a normal browser.
3. **A real certificate** needs a real domain name and a public DNS challenge, which brings accounts and an outside service. Not wanted by default.
4. **Run it behind the person's own reverse proxy or VPN** (Tailscale and similar): documented, no code needed once *This computer only* exists, plus `X-Forwarded-*` handling to decide.

Decision for version one: plain HTTP on the home network, loudly documented, with a *Secure* cookie flag turned on automatically when a proxy says it's HTTPS.

## 6. Server design

- A new module `src/web.rs` (routes, pairing, sessions, rate limiting) with the HTML, CSS and JS in `web/` embedded using `include_str!` / `include_bytes!`. No new dependency is needed for the server; QR codes need a small crate (to be chosen: check licence, size and `no_std`-ness; or write a minimal QR encoder if nothing suitable is small enough).
- The listener is started and stopped from the same place as the assistant link, following the *Web remote* switch and port (the same polling loop approach: close within about 2 s of switching off).
- Hardening of connection handling (cap, timeouts, constant-time compare) goes in a shared helper, used by both servers. This also fixes the gaps listed in §2 for the assistant link.
- No work on the UI thread; no `AppState` lock held across a network call, as everywhere else.

## 7. Web API and page

### 7.1 Routes (all under one origin)
| Route | Method | Needs session | Does |
| --- | --- | --- | --- |
| `/` | GET | no | The page (it shows the pairing screen if there's no session) |
| `/assets/<name>` | GET | no | Embedded script, style, icons |
| `/api/pair` | POST | no (needs code) | Exchange a one-time code for a session |
| `/api/state` | GET | yes | One snapshot: systems, rooms, groups, what's playing, volumes, queue head, scenes, favorites, the device's role |
| `/api/art/<id>` | GET | yes | Album art, fetched and relayed by Roomtone (speaker art addresses may not be reachable from a phone and shouldn't be exposed) |
| `/api/do` | POST | yes (Control) | One action from a fixed list (below) |
| `/api/browse` | GET | yes | Favorites, radio, playlists, scenes lists (and later search) |
| `/api/events` | GET | yes | Optional event stream (later) |
| `/api/signout` | POST | yes | Remove this device's session |

### 7.2 Actions (`/api/do`)
`play`, `pause`, `next`, `previous`, `seek`, `set_volume`, `set_mute` (group or room), `shuffle`, `repeat`, `play_favorite`, `play_station`, `play_playlist`, `queue_jump`, `queue_remove`, `group_add`, `group_remove`, `pause_all`, `do_scene`, `sleep_timer`. Each maps to an existing `Cmd`; none is new work in the backend except small wrappers. Parameters are validated (room ids, ranges 0–100) before reaching the backend.

### 7.3 The page
- One column on a phone, two on a tablet; large tap targets; no hover-only controls; light and dark following the phone; respects the system text size and reduced motion.
- Screens: **Rooms** (cards with song, play/pause, volume), **Now Playing** (art, title, artist, progress, transport, group and room volumes), **Queue**, **Library shortcuts** (favorites, radio, playlists), **Scenes**.
- Standard controls (`button`, `input type=range`) with labels, so a phone's screen reader works without extra effort; focus order matches reading order.
- Text from the same translation tables as the app (see §10). The page is sent in the person's browser language when the app has it, otherwise the app's language setting.
- Updates: poll `/api/state` every 1–2 s while the page is visible and every 10 s while hidden; pause when the tab is hidden; show a clear banner when the connection drops and recover on its own. Moving a slider sends the value when released (and about every 250 ms while dragging), so the phone doesn't flood the speakers.
- When the page is open but Roomtone isn't reachable or has a different version, it says so and offers *Try again*.

## 8. Settings and command line

New `config.json` fields, all `#[serde(default)]`: `web_remote` (switch, default false), `web_remote_port` (default 8791), `web_remote_local_only` (default false), `web_remote_devices` (list of {id, name, role, session_hash, paired, last_seen}). Documented in [SETTINGS.md](SETTINGS.md).

Settings gets a *Web remote* section: the switch, port, a status line ("Listening at http://… on this network"), *Add a device* (QR, link and code, with a 5-minute countdown), the paired devices list with role and *Sign out*, *Sign out everyone*, and the failed-attempts note. Section title tagged *Experimental* until it has been tried on real devices.

Command line (each saves and quits, like the assistant link): `--web-remote-on`, `--web-remote-off`, `--web-remote-port <n>`, `--web-remote-pair` (prints the pairing link and QR in the terminal for headless setups), `--web-remote-devices`. Help text stays in English. SPEC.md's command-line table and §6 (a new section) are updated.

## 9. Testing

- **Unit tests:** code and session generation, expiry, single use, constant-time compare, the host and origin checks (including rebinding and `null` origins), CSRF rules, role enforcement, parameter validation, throttling, connection cap, request limits, stored sessions hashed rather than plain.
- **Behaviour tests** using `test_support::demo_backend()`: pair, read state, perform every action, look-only is refused server-side, sign out takes effect immediately, switching the remote off closes the port.
- **Hostile-input tests** on the HTTP parsing (long headers, slow clients, oversized bodies, malformed requests).
- **Demo and screenshots:** the page works against the demo system; phone-sized screenshots of each screen (English plus one longer language; light and dark) are taken with the built-in browser's mobile preset against a demo run, and posted in the pull requests.
- **Real devices (ask before anything audible):** iPhone Safari, Android Chrome, a desktop browser and a tablet; each with VoiceOver and TalkBack; reconnect after sleep; a phone leaving and re-joining Wi-Fi. Read-only first; any audible test is planned with the owner of the speakers beforehand and restored afterwards.
- A written **security review** before the first release that includes it: the threat table in §3, walked through against the actual code, with the results in the pull request.

## 10. i18n and accessibility

- Page strings are collected with the existing phrase tables: a build step (or a test) extracts the phrases the page uses so `locales/*.tsv` stay the single source; `every_phrase_is_translated` covers them. MCP replies and CLI help stay English.
- The page follows the same checklist as the window: names and roles on everything, visible focus, no information by colour alone, announcements for changes (for example an error banner as a live region).

## 11. Phases (one pull request each)

1. **Hardening and groundwork**: shared HTTP helper (cap, timeouts, constant-time compare, throttling) used by both servers; `web_remote` fields, switch, listener lifecycle; tests. No page yet. Also closes the gaps in §2 for the assistant link.
2. **Pairing and devices**: pairing codes, QR in Settings, sessions, paired devices list, roles, `--web-remote-pair`; tests; screenshots of the Settings section.
3. **Read-only page**: rooms, Now Playing, queue, art; polling; phone-size screenshots.
4. **Controls**: transport, volumes, grouping, pause all, queue actions; role enforcement; real-device checks.
5. **Browse and scenes**: favorites, radio, playlists, scenes (and search later).
6. **Polish**: tablet layout, reconnect behaviour, accessibility pass with real screen readers, translations review, docs, a "Web remote" guide.

Each phase updates SPEC.md, CHANGELOG.md, SETTINGS.md and (phase 1 onwards) the network table in SPEC §9 (a new inbound TCP row).

## 12. Open questions (with a recommendation)

| Question | Recommendation |
| --- | --- |
| Separate port and switch, or one more route on 8790? | Separate (8791), so the assistant link's rules and the key stay untouched. |
| Pairing with a QR code and session cookie, or a typed key? | QR plus cookie as above; the typed word-code is the fallback. |
| First-version scope | Phases 1–4 (state, transport, volume, grouping, queue). Favorites and scenes right after. |
| Roles | Yes: *Control* and *Look only* per device, enforced on the server. |
| HTTPS | Plain HTTP on the home network for version one, with the warnings; reverse proxy/VPN documented. |
| Frontend code | Plain hand-written JS and CSS, no framework or build step; revisit if the page grows too big. |
| QR encoder | Pick a small, permissively licensed crate if one exists; otherwise a minimal encoder (the codes are short). |
| Polling or event stream | Polling first; the event stream (we already have the change watcher) once the rest is solid. |
| Name | "Web remote" in Settings and docs. |
| Experimental tag | Yes, until tried on real phones and screen readers. |

## 13. Risks

- A weak remote is worse than none, because it controls devices in people's homes. Security comes before looks, and phase 1 comes before any page.
- Plain HTTP is a real limitation on shared networks; the documentation must say so without softening it.
- It widens the app's attack surface; the hardening in phase 1 and the tests are the price.
- Phone browsers differ (Safari pauses background tabs, mobile Wi-Fi sleep): reconnect handling needs real-device checks.
- Maintenance: a second user interface to keep in step with the window. Mitigated by keeping it deliberately small and by driving it from the same `Cmd` handlers.
- Support: "I can't reach it" will be the common question; the Settings status line and the existing network check should help (phase 6).
