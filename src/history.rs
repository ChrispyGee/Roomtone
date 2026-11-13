//! Listening history: what's been played, kept on this computer (`history.json` in the settings
//! folder) and never sent anywhere. It feeds *Recently played* on Home and the History page,
//! and survives restarts, so "play that again" works tomorrow too.
//!
//! A song counts as played once it has been playing for a little while (not for every skip). The
//! same song is one entry with a count and a last-played time. Only the newest `KEEP` are kept.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::model::Item;

/// How long something must play before it goes in the history (seconds).
pub const MIN_PLAY: u64 = 20;
const KEEP: usize = 500;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub item: Item,
    /// Seconds since 1970.
    pub last_played: u64,
    pub plays: u32,
}

/// What identifies "the same song": its address, except that an address from this app's own
/// music server (which changes with the computer's network address) is reduced to the part that
/// stays the same. Stations are told apart by their address, songs without one by name.
pub fn key_of(item: &Item) -> String {
    let uri = item.uri.as_str();
    if let Some((_, rest)) = uri.split_once("/t/") {
        if uri.starts_with("http") {
            return format!("t/{}", rest.split(['.', '?']).next().unwrap_or(rest));
        }
    }
    if !uri.is_empty() {
        return uri.to_string();
    }
    format!("{}\u{1f}{}", item.title.to_lowercase(), item.artist.to_lowercase())
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    entries: Vec<Entry>,
}

impl Store {
    /// One more play of `item` at `now`. Returns true if the history changed (always, today).
    pub fn record(&mut self, item: &Item, now: u64) -> bool {
        let key = key_of(item);
        match self.entries.iter_mut().find(|e| key_of(&e.item) == key) {
            Some(e) => {
                e.plays += 1;
                e.last_played = now;
                // Keep the freshest details (a title or artwork may have been filled in).
                e.item = item.clone();
            }
            None => self.entries.push(Entry { item: item.clone(), last_played: now, plays: 1 }),
        }
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.last_played));
        self.entries.truncate(KEEP);
        true
    }

    /// Newest first.
    pub fn all(&self) -> Vec<Entry> {
        self.entries.clone()
    }

    pub fn recent(&self, n: usize) -> Vec<Item> {
        self.entries.iter().take(n).map(|e| e.item.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn load_from(path: &Path) -> Store {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// Decides when something has played long enough to count, group by group.
#[derive(Default)]
pub struct Tracker {
    /// For each group: what's playing, since when (seconds), and whether it's been counted.
    seen: HashMap<String, (String, u64, bool)>,
}

impl Tracker {
    /// Called every few seconds with what a group is playing (`None` if nothing, or paused).
    /// Returns the item the first time it has been playing for `MIN_PLAY` seconds.
    pub fn observe(&mut self, group: &str, playing: Option<&Item>, at: u64) -> Option<Item> {
        let Some(item) = playing else {
            // Paused or stopped: a song not yet counted starts its clock again when it resumes;
            // one that was counted stays counted (resuming it isn't another play).
            if self.seen.get(group).is_some_and(|e| !e.2) {
                self.seen.remove(group);
            }
            return None;
        };
        let key = key_of(item);
        let e = self.seen.entry(group.to_string()).or_insert_with(|| (key.clone(), at, false));
        if e.0 != key {
            *e = (key, at, false);
            return None;
        }
        if !e.2 && at.saturating_sub(e.1) >= MIN_PLAY {
            e.2 = true;
            return Some(item.clone());
        }
        None
    }
}

// ---- the one store for this run ----

struct Shared {
    store: Store,
    dirty: bool,
    tracker: Tracker,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared { store: path().map(|p| Store::load_from(&p)).unwrap_or_default(), dirty: false, tracker: Tracker::default() }))
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("history.json");
    Some(p)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn lock() -> std::sync::MutexGuard<'static, Shared> {
    shared().lock().unwrap_or_else(|e| e.into_inner())
}

/// A group is (or isn't) playing `item`: count it once it's been playing long enough.
pub fn observe(group: &str, playing: Option<&Item>) -> bool {
    let mut g = lock();
    let at = {
        // Seconds on a clock that only goes forward.
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs()
    };
    match g.tracker.observe(group, playing, at) {
        Some(item) => {
            g.store.record(&item, now_secs());
            g.dirty = true;
            true
        }
        None => false,
    }
}

pub fn recent(n: usize) -> Vec<Item> {
    lock().store.recent(n)
}

pub fn all() -> Vec<Entry> {
    lock().store.all()
}

pub fn count() -> usize {
    lock().store.len()
}

pub fn clear() {
    let mut g = lock();
    g.store.clear();
    g.dirty = true;
    drop(g);
    flush(true);
}

/// Write to disk if something changed.
pub fn flush(force: bool) {
    let Some(p) = path() else { return };
    let mut g = lock();
    if g.dirty || force {
        g.store.save_to(&p);
        g.dirty = false;
    }
}

/// How long ago, in words the page can translate: `(unit, n)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ago {
    JustNow,
    Minutes(u64),
    Hours(u64),
    Yesterday,
    Days(u64),
    /// Older than a month: show the date (`YYYY-MM-DD`).
    Date(String),
}

pub fn ago(then: u64, now: u64) -> Ago {
    let d = now.saturating_sub(then);
    match d {
        0..=89 => Ago::JustNow,
        90..=3_599 => Ago::Minutes(d / 60),
        3_600..=86_399 => Ago::Hours(d / 3_600),
        86_400..=172_799 => Ago::Yesterday,
        172_800..=2_591_999 => Ago::Days(d / 86_400),
        _ => Ago::Date(crate::backup::date_stamp(then)),
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn song(title: &str, uri: &str) -> Item {
        Item { title: title.into(), artist: "Band".into(), uri: uri.into(), ..Default::default() }
    }

    #[test]
    fn the_same_song_is_one_entry_with_a_count() {
        let mut s = Store::default();
        s.record(&song("One", "x-file-cifs://nas/a.flac"), 100);
        s.record(&song("Two", "x-file-cifs://nas/b.flac"), 200);
        s.record(&song("One", "x-file-cifs://nas/a.flac"), 300);
        assert_eq!(s.len(), 2);
        let all = s.all();
        assert_eq!((all[0].item.title.as_str(), all[0].plays, all[0].last_played), ("One", 2, 300), "newest first");
        assert_eq!((all[1].item.title.as_str(), all[1].plays), ("Two", 1));
        assert_eq!(s.recent(1)[0].title, "One");
    }

    #[test]
    fn songs_from_this_computers_server_are_the_same_after_its_address_changes() {
        let mut s = Store::default();
        s.record(&song("Mine", "http://192.168.1.5:52880/t/abc123.mp3"), 1);
        s.record(&song("Mine", "http://10.0.0.8:61000/t/abc123.mp3"), 2);
        assert_eq!(s.len(), 1);
        assert_eq!(s.all()[0].plays, 2);
        s.record(&song("Other", "http://10.0.0.8:61000/t/zzz.mp3"), 3);
        assert_eq!(s.len(), 2);
        // Something without an address is told apart by name and artist.
        s.record(&Item { title: "Live".into(), artist: "Radio".into(), ..Default::default() }, 4);
        s.record(&Item { title: "LIVE".into(), artist: "radio".into(), ..Default::default() }, 5);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn only_the_newest_are_kept() {
        let mut s = Store::default();
        for n in 0..(KEEP + 25) {
            s.record(&song(&format!("S{n}"), &format!("x://{n}")), n as u64 + 1);
        }
        assert_eq!(s.len(), KEEP);
        assert_eq!(s.recent(1)[0].title, format!("S{}", KEEP + 24));
        assert!(!s.all().iter().any(|e| e.item.title == "S0"), "the oldest went");
    }

    #[test]
    fn clearing_and_saving() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("history-tests");
        let file = dir.join("h.json");
        let mut s = Store::default();
        s.record(&song("One", "x://1"), 5);
        s.save_to(&file);
        let back = Store::load_from(&file);
        assert_eq!(back.len(), 1);
        assert_eq!(back.all()[0].plays, 1);
        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(Store::load_from(&dir.join("missing.json")).len(), 0, "no file is an empty history");
    }

    #[test]
    fn a_song_counts_after_it_has_played_a_while_and_only_once() {
        let mut t = Tracker::default();
        let a = song("A", "x://a");
        let b = song("B", "x://b");
        assert!(t.observe("g", Some(&a), 100).is_none(), "just started");
        assert!(t.observe("g", Some(&a), 110).is_none(), "ten seconds in");
        assert_eq!(t.observe("g", Some(&a), 121).map(|i| i.title), Some("A".into()), "twenty seconds in");
        assert!(t.observe("g", Some(&a), 200).is_none(), "already counted");
        // Skipping through songs counts none of them.
        assert!(t.observe("g", Some(&b), 205).is_none());
        assert!(t.observe("g", Some(&song("C", "x://c")), 210).is_none());
        assert!(t.observe("g", Some(&b), 215).is_none(), "back to B: the clock starts again");
        assert!(t.observe("g", Some(&b), 236).is_some());
    }

    #[test]
    fn groups_are_counted_separately_and_pausing_stops_the_clock() {
        let mut t = Tracker::default();
        let a = song("A", "x://a");
        t.observe("kitchen", Some(&a), 100);
        t.observe("bedroom", Some(&a), 105);
        assert!(t.observe("kitchen", Some(&a), 120).is_some());
        assert!(t.observe("bedroom", Some(&a), 120).is_none(), "the bedroom started later");
        // Paused for a long time, then resumed: it doesn't count the pause.
        let c = song("C", "x://c");
        t.observe("kitchen", Some(&c), 200);
        t.observe("kitchen", None, 205);
        assert!(t.observe("kitchen", Some(&c), 400).is_none(), "the pause isn't listening time");
        assert!(t.observe("kitchen", Some(&c), 421).is_some());
    }

    #[test]
    fn ago_in_words() {
        let now = 10_000_000;
        assert_eq!(ago(now - 30, now), Ago::JustNow);
        assert_eq!(ago(now - 600, now), Ago::Minutes(10));
        assert_eq!(ago(now - 7_300, now), Ago::Hours(2));
        assert_eq!(ago(now - 100_000, now), Ago::Yesterday);
        assert_eq!(ago(now - 400_000, now), Ago::Days(4));
        assert!(matches!(ago(1_700_000_000, 1_794_570_436), Ago::Date(d) if d == "2023-11-14"));
        assert_eq!(ago(now + 5, now), Ago::JustNow, "the clock being off isn't a crash");
    }
}
