//! Looking up the year an album came out (experimental, off until switched on).
//!
//! Streaming services don't say when something came out, so with a year filter on, their
//! results would all be hidden. When the switch in Settings is on, the artist and album name of
//! a result with no year are sent to Apple's public iTunes Search (itunes.apple.com), which needs
//! no sign-in, and the year of the matching album is kept in `year-lookups.json` so the same
//! album is asked about only once. A match is by name, so a reissue can give a later year.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::model::Item;

/// Apple asks callers to stay under about 20 searches a minute.
const GAP: Duration = Duration::from_millis(3200);

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(10)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build()
}

#[derive(Default)]
struct Store {
    /// Normalised "artist|album" to a year (0: Apple has no such album).
    years: HashMap<String, u32>,
    /// Set when Apple said to slow down: no more asking until the app is restarted.
    stopped: bool,
    last_asked: Option<Instant>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| {
        let years = path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Mutex::new(Store { years, ..Default::default() })
    })
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("year-lookups.json");
    Some(p)
}

fn save(s: &Store) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(&s.years) {
        let _ = std::fs::write(p, text);
    }
}

/// Lower case, letters and digits only, and without a bracketed "(Deluxe Edition)" or
/// "(Remastered)" part or a trailing "- Remastered 2011".
pub fn normal(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let base = text.split(" - ").next().unwrap_or(text);
    for c in base.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            c if depth == 0 && c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// What to ask Apple about for one result: its artist and album. Only albums and songs; a
/// station, playlist or podcast has no album year.
pub fn subject(it: &Item) -> Option<(String, String)> {
    let is_album = it.class.contains("musicAlbum");
    let is_song = it.class.contains("musicTrack");
    if it.year != 0 || it.source.starts_with("radio") || !(is_album || is_song) || it.artist.is_empty() {
        return None;
    }
    let album = if !it.album.is_empty() && !it.album.starts_with("http") {
        it.album.clone()
    } else if is_album {
        it.title.clone()
    } else {
        return None;
    };
    Some((it.artist.clone(), album))
}

pub fn key(artist: &str, album: &str) -> String {
    format!("{}|{}", normal(artist), normal(album))
}

/// The year in Apple's reply that belongs to this album: results whose artist and album names
/// agree, and the earliest of those (later ones are usually reissues). 0 when none agree.
pub fn best_year(reply: &serde_json::Value, artist: &str, album: &str) -> u32 {
    let (want_artist, want_album) = (normal(artist), normal(album));
    reply
        .get("results")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter(|r| {
            let get = |k: &str| normal(r.get(k).and_then(|x| x.as_str()).unwrap_or(""));
            let (a, c) = (get("artistName"), get("collectionName"));
            !c.is_empty() && c == want_album && (a == want_artist || a.contains(&want_artist) || want_artist.contains(&a) && !a.is_empty())
        })
        .map(|r| crate::years::year_in(r.get("releaseDate").and_then(|x| x.as_str()).unwrap_or("")))
        .filter(|y| *y > 0)
        .min()
        .unwrap_or(0)
}

/// The year already known for this album (`Some(0)`: asked, Apple has none).
pub fn cached(artist: &str, album: &str) -> Option<u32> {
    store().lock().ok()?.years.get(&key(artist, album)).copied()
}

fn ask(artist: &str, album: &str) -> Result<u32, String> {
    let resp =
        agent().get("https://itunes.apple.com/search").query("media", "music").query("entity", "album").query("limit", "10").query("term", &format!("{artist} {album}")).call();
    match resp {
        Ok(r) => Ok(best_year(&r.into_json::<serde_json::Value>().map_err(|e| e.to_string())?, artist, album)),
        Err(ureq::Error::Status(code, _)) if code == 403 || code == 429 => {
            if let Ok(mut s) = store().lock() {
                s.stopped = true;
            }
            Err("Apple asked for fewer requests".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Find the years of these albums, one at a time and slowly enough for Apple, and call `found`
/// with each (key, year) as it comes in. Stops when `budget` runs out or Apple says to slow down.
/// Albums already known are reported at once and cost nothing.
pub fn resolve(pairs: &[(String, String)], budget: Duration, mut found: impl FnMut(&str, u32)) {
    let start = Instant::now();
    for (artist, album) in pairs {
        let k = key(artist, album);
        if let Some(y) = cached(artist, album) {
            found(&k, y);
            continue;
        }
        if start.elapsed() > budget {
            return;
        }
        let wait = {
            let s = store().lock().unwrap_or_else(|e| e.into_inner());
            if s.stopped {
                return;
            }
            s.last_asked.map(|t| GAP.saturating_sub(t.elapsed())).unwrap_or_default()
        };
        std::thread::sleep(wait);
        if let Ok(mut s) = store().lock() {
            s.last_asked = Some(Instant::now());
        }
        // A network failure isn't remembered, so the album is tried again another time.
        if let Ok(y) = ask(artist, album) {
            if let Ok(mut s) = store().lock() {
                s.years.insert(k.clone(), y);
                save(&s);
            }
            found(&k, y);
        }
    }
}

/// Give results with no year the one already looked up for their album (nothing is asked).
pub fn apply(items: &mut [Item]) {
    for it in items.iter_mut() {
        if let Some((a, b)) = subject(it) {
            if let Some(y) = cached(&a, &b) {
                it.year = y;
            }
        }
    }
}

/// The distinct albums among results that still have no year, at most `max`.
pub fn wanted(items: &[Item], max: usize) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    items.iter().filter_map(subject).filter(|(a, b)| cached(a, b).is_none() && seen.insert(key(a, b))).take(max).collect()
}

#[cfg(test)]
pub(crate) fn seed(artist: &str, album: &str, year: u32) {
    store().lock().unwrap().years.insert(key(artist, album), year);
}

#[cfg(test)]
mod unit {
    use super::*;
    use serde_json::json;

    fn item(artist: &str, album: &str, class: &str) -> Item {
        Item { artist: artist.into(), album: album.into(), title: album.into(), class: class.into(), source: "svc:9".into(), ..Default::default() }
    }

    #[test]
    fn names_are_compared_without_the_extras() {
        assert_eq!(normal("Abbey Road (Remastered 2009)"), normal("abbey road"));
        assert_eq!(normal("Kind of Blue - Legacy Edition"), "kindofblue");
        assert_eq!(normal("Sgt. Pepper's [Deluxe]"), "sgtpeppers");
        assert_eq!(key("The Beatles", "Abbey Road"), "thebeatles|abbeyroad");
    }

    #[test]
    fn the_matching_albums_earliest_year_wins() {
        let reply = json!({"results": [
            {"artistName": "The Beatles", "collectionName": "Abbey Road (Remastered)", "releaseDate": "2019-09-27T07:00:00Z"},
            {"artistName": "The Beatles", "collectionName": "Abbey Road", "releaseDate": "1969-09-26T07:00:00Z"},
            {"artistName": "Beatles Tribute Band", "collectionName": "Abbey Road", "releaseDate": "1999-01-01T00:00:00Z"},
            {"artistName": "The Beatles", "collectionName": "Let It Be", "releaseDate": "1970-05-08T07:00:00Z"},
        ]});
        assert_eq!(best_year(&reply, "The Beatles", "Abbey Road"), 1969);
        assert_eq!(best_year(&reply, "The Beatles", "Revolver"), 0, "an album Apple didn't list gets no year");
        assert_eq!(best_year(&json!({}), "A", "B"), 0);
    }

    #[test]
    fn only_albums_and_songs_without_a_year_are_looked_up() {
        let album = item("Nova Reyes", "Midnight Atlas", "object.container.album.musicAlbum");
        let song = item("Nova Reyes", "Midnight Atlas", "object.item.audioItem.musicTrack");
        assert_eq!(subject(&album), Some(("Nova Reyes".into(), "Midnight Atlas".into())));
        assert!(subject(&song).is_some());
        assert!(subject(&Item { year: 2023, ..album.clone() }).is_none(), "it already has one");
        assert!(subject(&item("", "Midnight Atlas", "object.container.album.musicAlbum")).is_none(), "no artist to go on");
        assert!(subject(&item("Jazz FM", "", "object.item.audioItem.audioBroadcast")).is_none(), "a station has no album year");
        let both = vec![album.clone(), song, item("Other", "Thing", "object.container.album.musicAlbum")];
        assert_eq!(wanted(&both, 10).len(), 2, "one album and its song are asked about once");
        assert_eq!(wanted(&both, 1).len(), 1);
    }

    #[test]
    fn a_remembered_year_is_applied_without_asking() {
        seed("Lookup Test Artist", "Lookup Test Album", 1977);
        seed("Lookup Test Artist", "Missing Album", 0);
        let mut items = vec![
            item("Lookup Test Artist", "Lookup Test Album", "object.container.album.musicAlbum"),
            item("Lookup Test Artist", "Missing Album", "object.container.album.musicAlbum"),
        ];
        apply(&mut items);
        assert_eq!((items[0].year, items[1].year), (1977, 0));
        let mut got = vec![];
        resolve(&[("lookup test artist".into(), "LOOKUP TEST ALBUM".into())], Duration::ZERO, |k, y| got.push((k.to_string(), y)));
        assert_eq!(got, [("lookuptestartist|lookuptestalbum".to_string(), 1977)], "a known album costs no request, even with no time");
    }
}
