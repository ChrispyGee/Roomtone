# Contributing to Roomtone

Thanks for helping! Bug reports, speaker reports, translations, docs and code are all welcome. Please follow the [Code of Conduct](CODE_OF_CONDUCT.md) in every issue, discussion and pull request.

If you use an AI coding agent, point it at [AGENTS.md](AGENTS.md) first; it has the same rules in a checklist.

## Ways to help

- **Speaker reports**: tell us what works on your models and firmware (use the *Speaker report* issue form). This matters most: several features are only verified on the demo system (see [docs/ROADMAP.md](docs/ROADMAP.md)).
- **Bugs**: use the *Bug report* form. Include your platform, S1/S2, speaker models and what you expected.
- **Translations**: fix wording in `locales/*.tsv` or add a language (below).
- **Code**: pick an issue, or open one to discuss your idea first if it's big.

## Getting set up

Fork <https://github.com/ChrispyGee/Roomtone> on GitHub, then:

```sh
git clone https://github.com/<your-user-name>/Roomtone.git
cd Roomtone
git remote add upstream https://github.com/ChrispyGee/Roomtone.git
cargo build
cargo run -- --demo          # every screen, no speakers needed
cargo test
```

On Linux you need the desktop development packages first, for example on Debian or Ubuntu:

```sh
sudo apt install libxkbcommon-dev libgtk-3-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev
```

Read [SPEC.md](SPEC.md) (what the app does) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (how the code fits together) before larger changes.

## Before you open a pull request

1. `cargo fmt` (line width is set in `rustfmt.toml`).
2. `cargo clippy --all-targets -- -D warnings`: no warnings.
3. `cargo test`: this also fails if any wording isn't translated.
4. `cargo run -- --screenshot shots/check.png`, then look at the picture: the demo system should draw correctly.
5. If behaviour changed, update **[SPEC.md](SPEC.md)** in the same pull request, and add a line to **[CHANGELOG.md](CHANGELOG.md)** under *Unreleased*.
6. If you touched the speaker protocol, update **[docs/PROTOCOL.md](docs/PROTOCOL.md)**.
7. **Privacy check**: make sure nothing from your own home slipped in: room names, speaker IP addresses, household or speaker ids (`Sonos_…`, `RINCON_…`), sign-in tokens, the assistant access key, or paths with your user name. Test data and screenshots must use the demo system. A quick look:
   ```sh
   git diff --cached | grep -nEi '192\.168\.|10\.[0-9]+\.[0-9]+\.[0-9]+|172\.(1[6-9]|2[0-9]|3[01])\.|RINCON_[0-9A-F]{6}|Sonos_[A-Za-z0-9]{10}|/Users/|Bearer [0-9a-f]{8}'
   ```
   The only addresses that belong in the code are placeholders such as `192.168.1.20` and `127.0.0.1`.

Keep pull requests focused: one feature or fix each. Describe what you tested and on which speakers (or "demo only").

## Rules that keep the app working

These are explained in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#conventions); in short:

- Everything the person can do is a `Cmd`, handled in the backend and also in the demo system.
- Never block the window thread on the network.
- Every new piece of wording is wrapped in `t!(…)` and translated into every language in `locales/`.
- Stored settings stay compatible: new fields get `#[serde(default)]`.
- Anything that can't be undone (software updates; moving S1 to S2, which the app never does) needs explicit confirmation.
- No telemetry, and no new network destinations without saying so in SPEC.md §9.
- Don't use the speaker maker's logos or artwork, or other apps' code. Protocol identifiers in addresses (for example `x-sonos-http:`) are fine where the speakers need them.

## Testing with real speakers

- The read-only network tests: `ROOMTONE_SPEAKER=<speaker ip> cargo test -- --ignored --nocapture`.
- Point `HOME` (Windows: `APPDATA`) at a scratch folder when testing the assistant link, so your own settings aren't changed.
- **Don't play audio, regroup rooms or install updates on speakers other people are using** without asking them.
- Remove room names, IP addresses and account details from logs and screenshots before posting them.

## Translations

The tables are `locales/<code>.tsv`, one `English<TAB>Translation` per line. The English side must match the code exactly; keep placeholders such as `{room}` or `{n}` unchanged.

- **Improve a translation**: edit the right-hand side and run `cargo test`.
- **Find what's missing**: `cargo test` lists any untranslated phrase.
- **List every phrase**: `ROOMTONE_PHRASES=phrases.txt cargo test dump -- --ignored`.
- **Add a language**: add `(code, name)` to `LANGUAGES` and an `include_str!` line to `TABLES_TSV` in `src/i18n.rs`, create `locales/<code>.tsv` with every phrase, run `cargo test`, then try it with `cargo run -- --demo --lang <code>`. Check that long words still fit the buttons.

## Adding an assistant (MCP) tool

Add a row to `tools()` in `src/mcp.rs` (name, description written for an AI, JSON schema, whether it changes things) and an arm in `Mcp::call` that sends `Cmd`s and waits with `self.wait`. Replies are English. Document it in SPEC.md §6.3 and update the tool counts in SPEC.md §11.

## Licence

Roomtone is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Unless you say otherwise, any contribution you submit for inclusion is dual-licensed the same way, with no additional terms or conditions.
