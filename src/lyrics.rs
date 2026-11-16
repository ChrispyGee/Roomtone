//! Lyrics for the song that's playing (experimental, off until the person switches it on).
//!
//! The words come from LRCLIB (lrclib.net), a free community lyrics service that needs no
//! account. Only the title, artist, album and length of the song are sent, only while the Now
//! Playing page is open, and the answer is kept in memory for this run (never written to disk).
//! Some songs have time-stamped ("synced") lyrics, some have plain text, many have none.

use serde::Deserialize;

/// Where lyrics are asked for (listed in SPEC §9).
pub const HOST: &str = "https://lrclib.net";

/// One line, and when it starts (seconds; 0 for plain lyrics).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub at: f32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    /// The lines carry times, so the current one can be followed.
    pub synced: bool,
    /// The service says the song has no words.
    pub instrumental: bool,
}

impl Lyrics {
    /// The line being sung at `position` seconds (synced lyrics only).
    pub fn current(&self, position: f32) -> Option<usize> {
        if !self.synced {
            return None;
        }
        // A little early, so the line is lit as it starts rather than just after.
        let p = position + 0.25;
        self.lines.iter().rposition(|l| l.at <= p)
    }
}

/// What is known about one song's lyrics.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Loading,
    Found(Lyrics),
    /// Asked, and the service doesn't have them.
    Missing,
    /// Couldn't ask (offline, service down…), with a short reason.
    Failed(String),
}

/// What identifies a song in the in-memory store.
pub fn key(artist: &str, title: &str) -> String {
    format!("{}\u{1}{}", artist.trim().to_lowercase(), title.trim().to_lowercase())
}

/// Read time-stamped lines (`[01:23.45] words`). Lines may carry several times; tags such as
/// `[ar:Someone]` are skipped.
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some(end) = r.find(']') else { break };
            match parse_time(&r[..end]) {
                Some(t) => times.push(t),
                None if times.is_empty() => {
                    // A tag like [ar:...]: nothing to show on this line.
                    rest = "";
                    break;
                }
                None => break,
            }
            rest = r[end + 1..].trim_start();
        }
        for t in times {
            out.push(Line { at: t, text: rest.trim().to_string() });
        }
    }
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    out
}

fn parse_time(s: &str) -> Option<f32> {
    let (m, sec) = s.split_once(':')?;
    let m: u32 = m.trim().parse().ok()?;
    let sec: f32 = sec.trim().parse().ok()?;
    if !(0.0..60.0).contains(&sec) {
        return None;
    }
    Some(m as f32 * 60.0 + sec)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Reply {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    instrumental: bool,
}

/// Turn one service answer into lyrics (`None` when it holds no words at all).
fn from_reply(r: Reply) -> Option<Lyrics> {
    if let Some(l) = r.synced_lyrics.as_deref().map(parse_lrc).filter(|l| l.iter().any(|x| !x.text.is_empty())) {
        return Some(Lyrics { lines: l, synced: true, instrumental: false });
    }
    if let Some(p) = r.plain_lyrics.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        let lines = p.lines().map(|t| Line { at: 0.0, text: t.trim().to_string() }).collect();
        return Some(Lyrics { lines, synced: false, instrumental: false });
    }
    if r.instrumental {
        return Some(Lyrics { lines: Vec::new(), synced: false, instrumental: true });
    }
    None
}

/// What asks the service: a path, its query, and the text that came back (`None` for "not found").
pub type Get<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> Result<Option<String>, String>;

/// Look a song up: an exact match first, then the best search hit.
pub fn find(get: Get, artist: &str, title: &str, album: &str, duration: f32) -> Result<Option<Lyrics>, String> {
    let secs = (duration.round() as u32).to_string();
    let mut q = vec![("track_name", title), ("artist_name", artist)];
    if !album.is_empty() {
        q.push(("album_name", album));
    }
    if duration > 0.0 {
        q.push(("duration", secs.as_str()));
    }
    if let Some(body) = get("/api/get", &q)? {
        if let Some(l) = serde_json::from_str::<Reply>(&body).ok().and_then(from_reply) {
            return Ok(Some(l));
        }
    }
    let Some(body) = get("/api/search", &[("track_name", title), ("artist_name", artist)])? else { return Ok(None) };
    let hits: Vec<Reply> = serde_json::from_str(&body).unwrap_or_default();
    Ok(hits.into_iter().find_map(from_reply))
}

/// The real thing: one HTTPS `GET`, a few seconds' patience, no cookies.
pub fn get_over_https(path: &str, query: &[(&str, &str)]) -> Result<Option<String>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"), " (", env!("CARGO_PKG_REPOSITORY"), ")"))
        .build();
    let mut req = agent.get(&format!("{HOST}{path}"));
    for (k, v) in query {
        req = req.query(k, v);
    }
    match req.call() {
        Ok(r) => r.into_string().map(Some).map_err(|e| e.to_string()),
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(format!("the lyrics service answered {code}")),
        Err(e) => Err(e.to_string()),
    }
}

/// Made-up words for the demo system: nothing is fetched, and no real song is quoted.
pub fn demo(title: &str) -> Lyrics {
    let words = ["(demo lyrics)", "These are made-up words", "so the demo can show", "how lyrics follow the song", "one line at a time", "and keep going", "until the music ends"];
    let _ = title;
    Lyrics { lines: words.iter().enumerate().map(|(i, t)| Line { at: 10.0 + i as f32 * 25.0, text: t.to_string() }).collect(), synced: true, instrumental: false }
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn reads_timed_lines() {
        let l = parse_lrc("[ar:Nobody]\n[00:12.50] first line\n[01:02.00]second\n[00:05.00][00:30.00] twice\n\nplain words");
        let got: Vec<(f32, &str)> = l.iter().map(|x| (x.at, x.text.as_str())).collect();
        assert_eq!(got, vec![(5.0, "twice"), (12.5, "first line"), (30.0, "twice"), (62.0, "second")]);
    }

    #[test]
    fn rejects_bad_times() {
        assert!(parse_lrc("[00:99] nope\n[xx:10] nope").is_empty());
    }

    #[test]
    fn follows_the_song() {
        let l = Lyrics { lines: parse_lrc("[00:10] a\n[00:20] b\n[00:30] c"), synced: true, instrumental: false };
        assert_eq!(l.current(0.0), None);
        assert_eq!(l.current(9.9), Some(0));
        assert_eq!(l.current(25.0), Some(1));
        assert_eq!(l.current(999.0), Some(2));
        let plain = Lyrics { synced: false, ..l };
        assert_eq!(plain.current(25.0), None);
    }

    #[test]
    fn prefers_synced_then_plain_then_instrumental() {
        let r = |s: &str| serde_json::from_str::<Reply>(s).unwrap();
        assert!(from_reply(r(r#"{"syncedLyrics":"[00:01] hi","plainLyrics":"hi"}"#)).unwrap().synced);
        let p = from_reply(r(r#"{"syncedLyrics":null,"plainLyrics":"one\ntwo"}"#)).unwrap();
        assert!(!p.synced && p.lines.len() == 2);
        assert!(from_reply(r(r#"{"instrumental":true}"#)).unwrap().instrumental);
        assert!(from_reply(r(r#"{"plainLyrics":"  "}"#)).is_none());
    }

    #[test]
    fn asks_exactly_then_searches_and_sends_only_the_song() {
        let seen = RefCell::new(Vec::<String>::new());
        let get = |path: &str, q: &[(&str, &str)]| {
            seen.borrow_mut().push(format!("{path} {}", q.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")));
            Ok(if path == "/api/get" { None } else { Some(r#"[{"plainLyrics":null},{"plainLyrics":"found it"}]"#.to_string()) })
        };
        let l = find(&get, "An Artist", "A Song", "An Album", 201.4).unwrap().unwrap();
        assert_eq!(l.lines[0].text, "found it");
        let seen = seen.into_inner();
        assert_eq!(seen[0], "/api/get track_name=A Song&artist_name=An Artist&album_name=An Album&duration=201");
        assert_eq!(seen[1], "/api/search track_name=A Song&artist_name=An Artist");
    }

    #[test]
    fn nothing_found_and_errors_are_told_apart() {
        assert_eq!(find(&|_, _| Ok(None), "a", "b", "", 0.0), Ok(None));
        assert!(find(&|_, _| Err("offline".into()), "a", "b", "", 0.0).is_err());
    }

    /// Asks the real service about a traditional song (nothing about the person is sent). Prints
    /// only what was found, never the words. Run with `cargo test real_lookup -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_lookup() {
        let l = find(&get_over_https, "Traditional", "Amazing Grace", "", 0.0).expect("the service should answer");
        match l {
            Some(l) => println!("found: {} lines, synced: {}, instrumental: {}", l.lines.len(), l.synced, l.instrumental),
            None => println!("the service has no lyrics for that song"),
        }
        let none = find(&get_over_https, "Nobody At All", "Zzyzx Qwerty 12345 Not A Song", "", 0.0).expect("the service should answer");
        assert!(none.is_none(), "a made-up song has no lyrics");
    }

    #[test]
    fn key_ignores_case_and_spaces() {
        assert_eq!(key(" The Band ", "Song"), key("the band", " song"));
        assert_ne!(key("a", "b"), key("b", "a"));
    }
}
