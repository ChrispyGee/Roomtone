//! What the app remembers about songs it has sent to the speakers.
//!
//! Speakers keep only the web address of songs that come from this computer, podcasts
//! or the Internet Archive when a queue is saved as a playlist, so the playlist later
//! shows a code instead of a title. Remembering the details here (and between runs)
//! lets those playlists show proper titles, artists and artwork again.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::model::Item;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Known {
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    songs: HashMap<String, Known>,
    /// Relay codes (for S1 speakers) and the https address each one stands for.
    #[serde(default)]
    relays: HashMap<String, String>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| {
        let loaded = path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok());
        Mutex::new(loaded.unwrap_or_default())
    })
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("songs.json");
    Some(p)
}

fn save(s: &Store) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(s) {
        let _ = std::fs::write(p, text);
    }
}

/// The part of an address that stays the same if this computer's network address changes.
fn key(uri: &str) -> String {
    let u = uri.trim_start_matches("x-rincon-mp3radio://").trim_start_matches("http://").trim_start_matches("https://");
    match u.find("/t/").or_else(|| u.find("/p/")) {
        // Our own server: keep only "/t/<code>" or "/p/<code>".
        Some(i) if u[..i].contains(':') => u[i..].split(['.', '?']).next().unwrap_or("").to_string(),
        _ => u.to_string(),
    }
}

/// Remember the details of songs about to be sent to a speaker.
pub fn remember(items: &[Item]) {
    let Ok(mut s) = store().lock() else { return };
    let mut changed = false;
    for it in items.iter().filter(|i| !i.uri.is_empty() && !i.title.is_empty()) {
        let k = key(&it.uri);
        let known = Known {
            title: it.title.clone(),
            artist: it.artist.clone(),
            album: if it.album.starts_with("http") { String::new() } else { it.album.clone() },
            art: it.art.clone(),
            duration: it.duration,
            genre: it.genre.clone(),
            source: it.source.clone(),
        };
        let same = s.songs.get(&k).is_some_and(|o| o.title == known.title && o.artist == known.artist);
        if !same {
            s.songs.insert(k, known);
            changed = true;
        }
    }
    if changed {
        save(&s);
    }
}

pub fn remember_relay(code: &str, url: &str) {
    let Ok(mut s) = store().lock() else { return };
    if s.relays.get(code).map(String::as_str) != Some(url) {
        s.relays.insert(code.to_string(), url.to_string());
        save(&s);
    }
}

pub fn relay(code: &str) -> Option<String> {
    store().lock().ok()?.relays.get(code).cloned()
}

/// Does this title look like an address or a file code rather than a real name?
fn looks_unnamed(item: &Item) -> bool {
    let t = item.title.trim();
    if t.is_empty() || t.starts_with("http") || t.contains("://") {
        return true;
    }
    let stem = t.rsplit_once('.').map(|(a, _)| a).unwrap_or(t);
    stem.len() >= 8 && stem.chars().all(|c| c.is_ascii_hexdigit()) || item.uri.ends_with(t) || stem.chars().all(|c| c.is_ascii_digit())
}

/// Fill in titles, artists and artwork the speakers didn't keep.
pub fn enrich(items: &mut [Item]) {
    for it in items.iter_mut() {
        if it.uri.is_empty() || !looks_unnamed(it) && !it.artist.is_empty() {
            continue;
        }
        let k = key(&it.uri);
        let found = super::local::known_track(&k).or_else(|| store().lock().ok().and_then(|s| s.songs.get(&k).cloned()));
        if let Some(m) = found {
            if looks_unnamed(it) {
                it.title = m.title;
            }
            if it.artist.is_empty() {
                it.artist = m.artist;
            }
            if it.album.is_empty() {
                it.album = m.album;
            }
            if m.art.is_some() {
                it.art = m.art;
            }
            if it.duration <= 0.0 {
                it.duration = m.duration;
            }
            if it.genre.is_empty() {
                it.genre = m.genre;
            }
        } else if looks_unnamed(it) {
            // At least show something readable: the file name without its code-like parts.
            let last = it.uri.rsplit('/').next().unwrap_or(&it.uri).split('?').next().unwrap_or("");
            let decoded = last.replace("%20", " ");
            let stem = decoded.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(decoded);
            if !stem.chars().all(|c| c.is_ascii_hexdigit()) {
                it.title = stem;
            } else {
                it.title = "Unknown song".into();
                if it.description.is_empty() {
                    it.description = "Song from this computer (details not available)".into();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Read-only check against a real speaker: `ROOMTONE_SPEAKER=<ip> cargo test playlist_names -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn playlist_names() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else { return };
        let cfg = crate::config::Config::load();
        super::super::local::start_server();
        super::super::local::rescan(&cfg.folders, |_| {});
        let lists = crate::net::speaker::browse(&ip, "SQ:").unwrap();
        let Some(first) = lists.first() else { return };
        let mut items = crate::net::speaker::browse(&ip, &first.id).unwrap();
        println!("before: {}", items[0].title);
        super::enrich(&mut items);
        for i in items.iter().take(4) {
            println!("after: {} · {} · {}", i.title, i.artist, i.album);
        }
    }
}
