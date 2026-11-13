//! Translations. Every piece of wording in the app is written in English in the code and
//! looked up here in the chosen language; anything not translated yet shows in English.
//!
//! - `t!("Play")` translates a phrase.
//! - `t!("{n} rooms", n = count)` translates, then fills in `{n}`.
//! - `tn!(count, "1 room", "{n} rooms")` picks the singular or plural phrase.
//! - `n_!("Artists")` only marks a phrase for translation (for constants); show it with `tr`.
//!
//! The tables are in `locales/<code>.tsv`: one `English<TAB>Translation` pair per line.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Languages the app speaks: (code, name in that language).
pub const LANGUAGES: &[(&str, &str)] = &[("en", "English"), ("de", "Deutsch"), ("fr", "Français"), ("es", "Español"), ("it", "Italiano"), ("nl", "Nederlands")];

const TABLES_TSV: &[&str] = &[
    "",
    include_str!("../locales/de.tsv"),
    include_str!("../locales/fr.tsv"),
    include_str!("../locales/es.tsv"),
    include_str!("../locales/it.tsv"),
    include_str!("../locales/nl.tsv"),
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static TABLES: OnceLock<Vec<HashMap<&'static str, &'static str>>> = OnceLock::new();

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

fn parse(tsv: &'static str) -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    for line in tsv.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((en, other)) = line.split_once('\t') else { continue };
        if other.is_empty() {
            continue;
        }
        let key: &'static str = if en.contains('\\') { Box::leak(unescape(en).into_boxed_str()) } else { en };
        let val: &'static str = if other.contains('\\') { Box::leak(unescape(other).into_boxed_str()) } else { other };
        map.insert(key, val);
    }
    map
}

fn tables() -> &'static Vec<HashMap<&'static str, &'static str>> {
    TABLES.get_or_init(|| TABLES_TSV.iter().map(|t| parse(t)).collect())
}

/// The computer's preferred language, as a two-letter code.
pub fn system_language() -> String {
    for var in ["ROOMTONE_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let code: String = v.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_lowercase();
            if code.len() == 2 && code != "c" {
                return code;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Apps started from the Dock don't get LANG; ask the system's language list instead.
        if let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(first) = text.split('"').nth(1) {
                let code: String = first.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_lowercase();
                if code.len() == 2 {
                    return code;
                }
            }
        }
    }
    "en".into()
}

/// Switch language: a code from LANGUAGES, or "auto" to follow the computer.
pub fn set_language(code: &str) {
    let code = if code.is_empty() || code == "auto" { system_language() } else { code.to_string() };
    let idx = LANGUAGES.iter().position(|(c, _)| *c == code).unwrap_or(0);
    CURRENT.store(idx, Ordering::Relaxed);
}

/// The code of the language in use now.
pub fn current() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed).min(LANGUAGES.len() - 1)].0
}

/// Translate a phrase (English if there's no translation).
pub fn tr(en: &str) -> &str {
    let idx = CURRENT.load(Ordering::Relaxed);
    if idx == 0 {
        return en;
    }
    tables().get(idx).and_then(|m| m.get(en).copied()).unwrap_or(en)
}

/// Translate, then fill in `{name}` placeholders.
pub fn tf(en: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = tr(en).to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), &v.to_string());
    }
    out
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        tf(one, &[("n", &n)])
    } else {
        tf(many, &[("n", &n)])
    }
}

#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::i18n::tr($s)
    };
    ($s:literal, $($k:ident = $v:expr),+ $(,)?) => {
        $crate::i18n::tf($s, &[$((stringify!($k), &$v as &dyn std::fmt::Display)),+])
    };
}

#[macro_export]
macro_rules! tn {
    ($n:expr, $one:literal, $many:literal) => {
        $crate::i18n::plural($n as usize, $one, $many)
    };
}

/// Marks a phrase for translation without translating it (for constants).
#[macro_export]
macro_rules! n_ {
    ($s:literal) => {
        $s
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCES: &[&str] = &[
        include_str!("main.rs"),
        include_str!("backend.rs"),
        include_str!("model.rs"),
        include_str!("demo.rs"),
        include_str!("sources/mod.rs"),
        include_str!("ui/mod.rs"),
        include_str!("ui/pages.rs"),
        include_str!("ui/widgets.rs"),
        include_str!("macos_about.rs"),
    ];

    /// Read one Rust string literal starting at the opening quote; returns its text and the rest.
    fn literal(s: &str) -> Option<(String, &str)> {
        let s = s.strip_prefix('"')?;
        let mut out = String::new();
        let mut chars = s.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => return Some((out, &s[i + 1..])),
                '\\' => match chars.next()?.1 {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '\n' => {
                        // Line continuation: skip the next line's leading spaces.
                        let rest = &s[i + 2..];
                        let skip = rest.len() - rest.trim_start().len();
                        for _ in 0..skip {
                            chars.next();
                        }
                    }
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
        None
    }

    /// Every phrase marked for translation in the code.
    fn phrases() -> Vec<String> {
        let mut out = vec![];
        for src in SOURCES {
            for (marker, count) in [("t!(", 1), ("n_!(", 1), ("tn!(", 2)] {
                let mut rest = *src;
                while let Some(at) = rest.find(marker) {
                    // `format!(` and `print!(` end in `t!(` too; only count the macros themselves.
                    let glued = rest[..at].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_');
                    rest = &rest[at + marker.len()..];
                    if glued {
                        continue;
                    }
                    // Skip the macro's own definition and doc examples.
                    if rest.starts_with('$') {
                        continue;
                    }
                    let mut r = rest;
                    for _ in 0..count {
                        let Some(q) = r.find('"') else { break };
                        // Only literals before the call closes (or the first comma for tn!'s count).
                        if r[..q].contains(';') {
                            break;
                        }
                        match literal(&r[q..]) {
                            Some((text, after)) => {
                                out.push(text);
                                r = after;
                            }
                            None => break,
                        }
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Write every phrase to a file, for translators: `ROOMTONE_PHRASES=out.txt cargo test dump -- --ignored`.
    #[test]
    #[ignore]
    fn dump() {
        let path = std::env::var("ROOMTONE_PHRASES").unwrap_or_else(|_| "phrases.txt".into());
        let text: Vec<String> = phrases().iter().map(|p| p.replace('\n', "\\n")).collect();
        std::fs::write(path, text.join("\n")).unwrap();
    }

    #[test]
    fn every_phrase_is_translated() {
        let all = phrases();
        assert!(all.len() > 100, "only found {} phrases", all.len());
        let mut missing = String::new();
        for (i, (code, _)) in LANGUAGES.iter().enumerate().skip(1) {
            let table = &tables()[i];
            for p in &all {
                if !table.contains_key(p.as_str()) {
                    missing.push_str(&format!("{code}: {}\n", p.replace('\n', "\\n")));
                }
            }
        }
        assert!(missing.is_empty(), "untranslated:\n{missing}");
    }

    #[test]
    fn placeholders_survive() {
        let all = phrases();
        for (i, (code, _)) in LANGUAGES.iter().enumerate().skip(1) {
            for p in &all {
                let Some(t) = tables()[i].get(p.as_str()) else { continue };
                for part in p.split('{').skip(1) {
                    let name = part.split('}').next().unwrap_or("");
                    assert!(t.contains(&format!("{{{name}}}")), "{code}: “{t}” lost {{{name}}}");
                }
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn placeholders_and_plurals() {
        crate::test_support::isolate();
        assert_eq!(tf("Playing in {room}", &[("room", &"Kitchen")]), "Playing in Kitchen");
        assert_eq!(plural(1, "1 room", "{n} rooms"), "1 room");
        assert_eq!(plural(3, "1 room", "{n} rooms"), "3 rooms");
        assert_eq!(tr("A phrase nobody translated"), "A phrase nobody translated");
    }

    #[test]
    fn every_language_has_a_table() {
        assert_eq!(LANGUAGES.len(), TABLES_TSV.len());
        assert_eq!(LANGUAGES[0].0, "en");
        for (i, t) in TABLES_TSV.iter().enumerate().skip(1) {
            assert!(t.lines().filter(|l| l.contains('\t')).count() > 400, "{} looks incomplete", LANGUAGES[i].0);
        }
    }

    #[test]
    fn tables_have_no_broken_lines() {
        for (i, t) in TABLES_TSV.iter().enumerate().skip(1) {
            for line in t.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
                let parts: Vec<&str> = line.split('\t').collect();
                assert_eq!(parts.len(), 2, "{}: “{line}” should be English<TAB>translation", LANGUAGES[i].0);
                assert!(!parts[1].trim().is_empty(), "{}: empty translation for “{}”", LANGUAGES[i].0, parts[0]);
            }
        }
    }
}
