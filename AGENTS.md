# Notes for AI coding agents

Read this before changing anything. It's the short, checkable version of [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## What this is

Roomtone: a Rust + eframe/egui desktop controller for S1 and S2 wireless speaker systems, which also runs headless and serves MCP to AI assistants. Single binary crate; the entry point is `src/main.rs`.

**Source of truth for behaviour: [SPEC.md](SPEC.md).** If your change alters what the app does, update SPEC.md in the same change. If SPEC.md and the code disagree, ask which is right; don't silently pick one.

## Commands

```sh
cargo build                                        # must succeed
cargo test                                         # must pass (includes the translation check)
cargo fmt                                          # always run; CI checks it
cargo clippy --all-targets -- -D warnings          # must be clean
cargo run -- --demo                                # try it with made-up rooms
cargo run -- --screenshot shots/now.png            # picture of the demo window; look at it
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
              '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run -q -- --mcp --demo
```

For MCP tests, set `HOME` (Windows: `APPDATA`) to a scratch folder such as `target/testhome` and run `--mcp-on` there first, so the real user's settings aren't touched.

## Map

| Need to change | Look in |
| --- | --- |
| What a button or command does | `src/backend.rs` (`Cmd`, `run_common`, `run_live`) and `src/demo.rs` (`apply`) |
| A speaker call | `src/net/speaker.rs` (protocol notes: `docs/PROTOCOL.md`) |
| A page or control | `src/ui/pages.rs`, `src/ui/mod.rs`, `src/ui/widgets.rs` |
| Data the UI shows | `src/model.rs` (`AppState`) |
| Saved settings | `src/config.rs` |
| Assistant tools | `src/mcp.rs` (`tools()`, `Mcp::call`) |
| Wording and languages | `t!`/`tn!`/`n_!` from `src/i18n.rs`; tables in `locales/*.tsv` |
| Music sources | `src/sources/` |
| Command-line help | `src/help.rs`, `src/main.rs` |

## Rules (checklist)

- [ ] Every new user-visible string is wrapped in `t!`/`tn!` (or `n_!` for constants, shown with `i18n::tr`) **and** added to all five `locales/*.tsv` files with placeholders kept. `cargo test` tells you what's missing. MCP replies and CLI help stay English. A **new source file** with `t!` strings must also be added to `SOURCES` in `src/i18n.rs`, or the test won't see its phrases.
- [ ] Wording is plain and friendly: say what happened and what to do; no bare error codes or jargon.
- [ ] New behaviour is a `Cmd`, handled in the backend **and** working in the demo system. The window, `--headless` and MCP must behave the same.
- [ ] No network calls on the UI thread; don't hold the `AppState` lock across a network call.
- [ ] New `config.json` fields use `#[serde(default)]`; never rename or remove fields.
- [ ] Irreversible actions (speaker software updates) require confirmation; never send S2 software to an S1 system; re-check guards just before acting.
- [ ] MCP: changing tools are marked as changing (they're hidden in look-only mode); keep the off switch, access key, Origin and Host checks intact. Update SPEC.md §6.3 and the counts in §11.
- [ ] Colours come from the palette in `src/ui/theme.rs` (`TEXT()`, `SURFACE()`, `overlay(a)`…), never fixed values, so light and dark both work. Check with `--theme light` and `--theme dark`.
- [ ] New parsers and rules get unit tests; behaviour tests drive `test_support::demo_backend()`, never real speakers. `cargo test` must stay green on every platform.
- [ ] A hand-drawn control (anything made with `allocate_exact_size`/`interact` and the painter, not a standard egui widget) must call `describe`/`WidgetInfo` so screen readers get its name and role, and `focus_ring` so keyboard users can see it; icon-only buttons take their name from `widgets::icon_name`.
- [ ] Keep `--screenshot` working: it must save a non-blank picture of the demo window without taking focus.
- [ ] Added, renamed or changed a setting (a `config.json` field, a switch, a command-line option)? Update docs/SETTINGS.md too.
- [ ] Update SPEC.md, CHANGELOG.md (*Unreleased*) and, for protocol changes, docs/PROTOCOL.md.
- [ ] No telemetry; no new network destinations without listing them in SPEC.md §9.
- [ ] Don't copy other apps' code, logos or artwork. Protocol identifiers the speakers require are fine.

## Safety with real speakers

Real speakers are often shared with other people. Without the person's explicit go-ahead **don't**:
- play or change audio, volume or grouping;
- install software updates or rename rooms;
- post logs or screenshots with room names, IP addresses, household ids, tokens or access keys.

Read-only probes are fine: the ignored tests (`ROOMTONE_SPEAKER=<ip> cargo test -- --ignored --nocapture`), browsing, *Check*, *Check for updates*.

## Definition of done

Build, tests, fmt and clippy are clean. A screenshot of the affected screen looks right in English and in one longer language (`--lang de`), in light and dark (`--theme`). The demo works. SPEC.md and CHANGELOG.md are updated. What was verified on real speakers versus the demo only is stated in the pull request and, if it's still open, in docs/ROADMAP.md.
