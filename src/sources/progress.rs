//! Where you stopped in each podcast episode, and which ones you've finished.
//!
//! Kept in `podcast-progress.json` (not in `config.json`: it's written every few seconds
//! while an episode plays). Entries are keyed by the episode's audio address and hold the
//! whole episode item, so "Continue listening" can start it again without asking the show's
//! feed. Only the newest `KEEP` episodes are remembered.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::model::Item;

/// An episode that stopped this close to the end counts as finished.
pub const END_MARGIN: f32 = 30.0;
/// Resuming isn't worth it for the first moments of an episode.
pub const RESUME_AFTER: f32 = 20.0;
/// How far back to jump when resuming, so you hear the sentence you were in.
pub const RESUME_REWIND: f32 = 3.0;
const KEEP: usize = 300;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub item: Item,
    pub position: f32,
    pub duration: f32,
    pub played: bool,
    /// Seconds since 1970 when it was last listened to or marked.
    pub updated: u64,
}

/// How an episode shows in lists.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    New,
    InProgress { left: f32 },
    Played,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    episodes: HashMap<String, Entry>,
}

impl Store {
    /// An episode is about to play: remember it, and say where to resume (if it's worth it).
    pub fn start(&mut self, item: &Item, now: u64) -> Option<f32> {
        let e = self.episodes.entry(item.uri.clone()).or_insert_with(|| Entry { item: item.clone(), duration: item.duration, updated: now, ..Default::default() });
        e.item = item.clone();
        if e.duration <= 0.0 {
            e.duration = item.duration;
        }
        e.updated = now;
        let resume = !e.played && e.position >= RESUME_AFTER && (e.duration <= 0.0 || e.position < e.duration - END_MARGIN);
        let at = resume.then(|| (e.position - RESUME_REWIND).max(0.0));
        self.prune();
        at
    }

    /// The episode is at `position` of `duration` seconds. Returns true if anything changed.
    pub fn record(&mut self, key: &str, position: f32, duration: f32, now: u64) -> bool {
        let Some(e) = self.episodes.get_mut(key) else { return false };
        if duration > 0.0 {
            e.duration = duration;
        }
        let finished = e.duration > 0.0 && position >= e.duration - END_MARGIN;
        let (new_position, played) = if finished { (0.0, true) } else { (position.max(0.0), e.played) };
        if (e.position - new_position).abs() < 1.0 && e.played == played {
            return false;
        }
        e.position = new_position;
        e.played = played;
        e.updated = now;
        true
    }

    pub fn mark(&mut self, item: &Item, played: bool, now: u64) {
        let e = self.episodes.entry(item.uri.clone()).or_insert_with(|| Entry { item: item.clone(), duration: item.duration, ..Default::default() });
        e.item = item.clone();
        e.played = played;
        e.position = 0.0;
        e.updated = now;
        self.prune();
    }

    pub fn status(&self, key: &str) -> Status {
        match self.episodes.get(key) {
            Some(e) if e.played => Status::Played,
            Some(e) if e.position >= RESUME_AFTER => Status::InProgress { left: if e.duration > 0.0 { (e.duration - e.position).max(0.0) } else { 0.0 } },
            _ => Status::New,
        }
    }

    /// Episodes started but not finished, most recently listened first.
    pub fn continue_list(&self, n: usize) -> Vec<Entry> {
        let mut v: Vec<&Entry> = self.episodes.values().filter(|e| !e.played && e.position >= RESUME_AFTER).collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.updated));
        v.into_iter().take(n).cloned().collect()
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.episodes.get(key)
    }

    fn prune(&mut self) {
        if self.episodes.len() > KEEP {
            let mut by_age: Vec<(u64, String)> = self.episodes.iter().map(|(k, e)| (e.updated, k.clone())).collect();
            by_age.sort();
            for (_, k) in by_age.into_iter().take(self.episodes.len() - KEEP) {
                self.episodes.remove(&k);
            }
        }
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

// ---- the one store for this run ----

struct Shared {
    store: Store,
    /// Changed since it was last written to disk.
    dirty: bool,
    last_save: std::time::Instant,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared { store: path().map(|p| Store::load_from(&p)).unwrap_or_default(), dirty: false, last_save: std::time::Instant::now() }))
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("podcast-progress.json");
    Some(p)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn with<R>(f: impl FnOnce(&mut Store) -> (R, bool)) -> R {
    let mut g = shared().lock().unwrap_or_else(|e| e.into_inner());
    let (r, changed) = f(&mut g.store);
    if changed {
        g.dirty = true;
    }
    r
}

/// Is this item a podcast episode?
pub fn is_episode(item: &Item) -> bool {
    item.id.starts_with("POD:EP/") && !item.uri.is_empty()
}

/// An episode is about to play: where to resume from, if anywhere.
pub fn start(item: &Item) -> Option<f32> {
    with(|s| (s.start(item, now()), true))
}

pub fn record(key: &str, position: f32, duration: f32) {
    with(|s| ((), s.record(key, position, duration, now())));
}

pub fn mark(item: &Item, played: bool) {
    with(|s| (s.mark(item, played, now()), true));
    flush(true);
}

pub fn status(key: &str) -> Status {
    with(|s| (s.status(key), false))
}

pub fn continue_list(n: usize) -> Vec<Entry> {
    with(|s| (s.continue_list(n), false))
}

pub fn title_of(key: &str) -> Option<String> {
    with(|s| (s.get(key).map(|e| e.item.title.clone()), false))
}

/// Write to disk if something changed and it's been a while (or `force`).
pub fn flush(force: bool) {
    let Some(p) = path() else { return };
    let mut g = shared().lock().unwrap_or_else(|e| e.into_inner());
    if g.dirty && (force || g.last_save.elapsed() > std::time::Duration::from_secs(15)) {
        g.store.save_to(&p);
        g.dirty = false;
        g.last_save = std::time::Instant::now();
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn ep(n: u32, duration: f32) -> Item {
        Item { id: format!("POD:EP/https://cdn/{n}.mp3"), uri: format!("https://cdn/{n}.mp3"), title: format!("Episode {n}"), duration, ..Default::default() }
    }

    #[test]
    fn a_new_episode_starts_from_the_beginning() {
        let mut s = Store::default();
        assert_eq!(s.start(&ep(1, 1800.0), 1), None);
        assert_eq!(s.status("https://cdn/1.mp3"), Status::New);
    }

    #[test]
    fn it_resumes_a_little_before_where_you_stopped() {
        let mut s = Store::default();
        s.start(&ep(1, 1800.0), 1);
        assert!(s.record("https://cdn/1.mp3", 600.0, 1800.0, 2));
        assert_eq!(s.status("https://cdn/1.mp3"), Status::InProgress { left: 1200.0 });
        assert_eq!(s.start(&ep(1, 1800.0), 3), Some(597.0));
    }

    #[test]
    fn the_first_moments_and_the_last_moments_dont_count() {
        let mut s = Store::default();
        s.start(&ep(1, 1800.0), 1);
        s.record("https://cdn/1.mp3", 10.0, 1800.0, 2);
        assert_eq!(s.start(&ep(1, 1800.0), 3), None, "under 20 s: start over");
        s.record("https://cdn/1.mp3", 1790.0, 1800.0, 4);
        assert_eq!(s.status("https://cdn/1.mp3"), Status::Played, "within 30 s of the end counts as finished");
        assert_eq!(s.start(&ep(1, 1800.0), 5), None, "a finished episode starts over");
    }

    #[test]
    fn marking_played_and_unplayed() {
        let mut s = Store::default();
        let e = ep(2, 600.0);
        s.mark(&e, true, 1);
        assert_eq!(s.status(&e.uri), Status::Played);
        s.mark(&e, false, 2);
        assert_eq!(s.status(&e.uri), Status::New);
        assert!(s.continue_list(5).is_empty());
    }

    #[test]
    fn continue_listening_is_newest_first_and_leaves_out_finished() {
        let mut s = Store::default();
        for (n, at) in [(1, 10), (2, 30), (3, 20)] {
            s.start(&ep(n, 3000.0), at);
            s.record(&format!("https://cdn/{n}.mp3"), 500.0, 3000.0, at);
        }
        s.mark(&ep(3, 3000.0), true, 40);
        let titles: Vec<String> = s.continue_list(5).into_iter().map(|e| e.item.title).collect();
        assert_eq!(titles, ["Episode 2", "Episode 1"]);
        assert_eq!(s.continue_list(1).len(), 1);
    }

    #[test]
    fn small_moves_are_not_saved_again() {
        let mut s = Store::default();
        s.start(&ep(1, 1800.0), 1);
        assert!(s.record("https://cdn/1.mp3", 100.0, 1800.0, 2));
        assert!(!s.record("https://cdn/1.mp3", 100.4, 1800.0, 3), "less than a second: nothing to write");
        assert!(!s.record("https://unknown", 50.0, 0.0, 3), "episodes that weren't started aren't tracked");
    }

    #[test]
    fn only_the_newest_episodes_are_kept() {
        let mut s = Store::default();
        for n in 0..(KEEP as u32 + 20) {
            s.mark(&ep(n, 100.0), true, n as u64 + 1);
        }
        assert_eq!(s.episodes.len(), KEEP);
        assert!(s.get("https://cdn/0.mp3").is_none() && s.get(&format!("https://cdn/{}.mp3", KEEP + 19)).is_some());
    }

    #[test]
    fn it_survives_a_restart() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("progress-tests");
        let file = dir.join("p.json");
        let mut s = Store::default();
        s.start(&ep(1, 1800.0), 1);
        s.record("https://cdn/1.mp3", 700.0, 1800.0, 2);
        s.save_to(&file);
        let back = Store::load_from(&file);
        assert_eq!(back.status("https://cdn/1.mp3"), Status::InProgress { left: 1100.0 });
        assert_eq!(Store::load_from(&dir.join("missing.json")).episodes.len(), 0, "no file is just empty");
    }

    #[test]
    fn only_feed_episodes_count_as_episodes() {
        assert!(is_episode(&ep(1, 10.0)));
        assert!(!is_episode(&Item { id: "FV:2/1".into(), uri: "x".into(), ..Default::default() }));
        assert!(!is_episode(&Item { id: "POD:EP/x".into(), ..Default::default() }), "no audio address");
    }
}
