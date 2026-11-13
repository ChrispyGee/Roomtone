//! Remembering the window's size and position between runs (`window.json` in the settings
//! folder), so Roomtone opens where and how you left it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The smallest size the app is laid out for (matches the window's minimum).
pub const MIN: (f32, f32) = (1040.0, 660.0);
const MAX: f32 = 10_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub width: f32,
    pub height: f32,
    /// Where the window's top-left corner is, if the system says (Wayland doesn't).
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}

impl Saved {
    /// Pull anything unreasonable back to something safe: a size the app can be drawn in, and a
    /// position that isn't way off any screen (a monitor may have been unplugged since).
    pub fn sanitized(mut self) -> Saved {
        self.width = if self.width.is_finite() { self.width.clamp(MIN.0, MAX) } else { 1320.0 };
        self.height = if self.height.is_finite() { self.height.clamp(MIN.1, MAX) } else { 840.0 };
        let sane = |v: Option<f32>| v.filter(|v| v.is_finite() && (-200.0..=6000.0).contains(v));
        let (x, y) = (sane(self.x), sane(self.y));
        // Both or neither: half a position is no position.
        (self.x, self.y) = if x.is_some() && y.is_some() { (x, y) } else { (None, None) };
        self
    }
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("window.json");
    Some(p)
}

pub fn load() -> Option<Saved> {
    let text = std::fs::read_to_string(path()?).ok()?;
    serde_json::from_str::<Saved>(&text).ok().map(Saved::sanitized)
}

fn save(s: &Saved) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(s) {
        let _ = std::fs::write(p, text);
    }
}

/// Watches the window while the app runs and writes the file a moment after it stops changing
/// (not on every pixel of a drag).
pub struct Watcher {
    last_seen: Option<Saved>,
    changed_at: Instant,
    written: Option<Saved>,
}

impl Watcher {
    pub fn new(loaded: Option<Saved>) -> Watcher {
        Watcher { last_seen: loaded, changed_at: Instant::now(), written: loaded }
    }

    /// What the window looks like now. `None` while it isn't known yet.
    pub fn observe(&mut self, now: Option<Saved>) {
        let Some(now) = now.map(Saved::sanitized) else { return };
        if self.last_seen != Some(now) {
            self.last_seen = Some(now);
            self.changed_at = Instant::now();
        }
        if self.written != self.last_seen && self.changed_at.elapsed() > Duration::from_millis(1500) {
            if let Some(s) = &self.last_seen {
                save(s);
            }
            self.written = self.last_seen;
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn s(w: f32, h: f32, x: Option<f32>, y: Option<f32>) -> Saved {
        Saved { width: w, height: h, x, y, maximized: false }
    }

    #[test]
    fn sizes_are_pulled_into_range() {
        assert_eq!(s(2000.0, 1000.0, None, None).sanitized().width, 2000.0);
        let tiny = s(100.0, 50.0, None, None).sanitized();
        assert_eq!((tiny.width, tiny.height), MIN);
        assert_eq!(s(f32::NAN, f32::INFINITY, None, None).sanitized().width, 1320.0);
        assert_eq!(s(99_999.0, 99_999.0, None, None).sanitized().width, MAX);
    }

    #[test]
    fn positions_far_off_screen_are_dropped() {
        let ok = s(1320.0, 840.0, Some(120.0), Some(80.0)).sanitized();
        assert_eq!((ok.x, ok.y), (Some(120.0), Some(80.0)));
        let left_of_screen = s(1320.0, 840.0, Some(-50.0), Some(10.0)).sanitized();
        assert_eq!(left_of_screen.x, Some(-50.0), "a second monitor to the left is fine");
        let gone = s(1320.0, 840.0, Some(-9000.0), Some(10.0)).sanitized();
        assert_eq!((gone.x, gone.y), (None, None), "an unplugged far-away monitor: let the system choose");
        let half = s(1320.0, 840.0, Some(100.0), None).sanitized();
        assert_eq!((half.x, half.y), (None, None));
    }

    #[test]
    fn it_reads_back_what_was_written() {
        let saved = s(1500.0, 900.0, Some(40.0), Some(60.0));
        let text = serde_json::to_string(&saved).unwrap();
        assert_eq!(serde_json::from_str::<Saved>(&text).unwrap().sanitized(), saved);
        assert!(serde_json::from_str::<Saved>("{ nonsense").is_err());
    }

    #[test]
    fn the_watcher_waits_for_the_window_to_settle() {
        let mut w = Watcher::new(None);
        let a = s(1400.0, 900.0, Some(10.0), Some(10.0));
        w.observe(Some(a));
        assert_eq!(w.written, None, "not yet: it may still be moving");
        w.changed_at -= Duration::from_secs(5);
        // (Writing goes to the real settings folder, which tests keep separate.)
        crate::test_support::isolate();
        w.observe(Some(a));
        assert_eq!(w.written, Some(a));
        let b = s(1500.0, 900.0, Some(10.0), Some(10.0));
        w.observe(Some(b));
        assert_eq!(w.written, Some(a), "a new size starts the wait again");
        w.observe(None);
        assert_eq!(w.last_seen, Some(b), "not knowing isn't a change");
    }
}
