//! The speakers found last time, so the next start can show them at once and check
//! them directly instead of waiting for a network search.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{Group, Household, Room, System};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KnownRoom {
    uuid: String,
    name: String,
    ip: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    version: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KnownGroup {
    coordinator: String,
    members: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KnownHousehold {
    id: String,
    s1: bool,
    rooms: Vec<KnownRoom>,
    #[serde(default)]
    groups: Vec<KnownGroup>,
    /// When a room of this system last answered (seconds since 1970); 0 when not recorded yet.
    #[serde(default)]
    last_seen: u64,
}

/// A system that hasn't answered for this long is forgotten (30 days).
const FORGET_AFTER_SECS: u64 = 30 * 24 * 3600;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn read() -> Vec<KnownHousehold> {
    path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Drop systems not seen for 30 days, and put the most recently seen first. One is always kept (the most recently seen), so a long
/// trip away from home doesn't forget the only system there is. A system with no date yet
/// counts as seen now, so nothing disappears the first time this runs.
fn prune(mut list: Vec<KnownHousehold>, now: u64) -> Vec<KnownHousehold> {
    for h in list.iter_mut().filter(|h| h.last_seen == 0) {
        h.last_seen = now;
    }
    let newest = list.iter().map(|h| h.last_seen).max().unwrap_or(0);
    let mut kept_newest = false;
    list.retain(|h| {
        let fresh = now.saturating_sub(h.last_seen) <= FORGET_AFTER_SECS;
        let keep_anyway = !fresh && h.last_seen == newest && !kept_newest;
        if h.last_seen == newest {
            kept_newest |= fresh || keep_anyway;
        }
        fresh || keep_anyway
    });
    // The system that answered most recently comes first, and is the one shown at start.
    list.sort_by_key(|h| std::cmp::Reverse(h.last_seen));
    list
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("speakers.json");
    Some(p)
}

/// Remember these systems for next time.
pub fn save(households: &[Household]) {
    let before = read();
    let stamp = now();
    let list: Vec<KnownHousehold> = households
        .iter()
        .map(|h| KnownHousehold {
            last_seen: if h.rooms.iter().any(|r| r.online == Some(true)) {
                stamp
            } else {
                before.iter().find(|k| k.id == h.id).map(|k| k.last_seen).filter(|t| *t > 0).unwrap_or(stamp)
            },
            id: h.id.clone(),
            s1: h.system == System::S1,
            rooms: h
                .rooms
                .iter()
                .filter(|r| !r.ip.is_empty())
                .map(|r| KnownRoom { uuid: r.uuid.clone(), name: r.name.clone(), ip: r.ip.clone(), model: r.model.clone(), version: r.version.clone() })
                .collect(),
            groups: h.groups.iter().map(|g| KnownGroup { coordinator: g.coordinator.clone(), members: g.members.clone() }).collect(),
        })
        .filter(|h| !h.rooms.is_empty())
        .collect();
    if list.is_empty() {
        return;
    }
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(p, text);
    }
}

/// The systems from last time, not yet checked (every room starts as "not checked yet").
pub fn load() -> Vec<Household> {
    let list = prune(read(), now());
    list.into_iter()
        .map(|h| {
            let rooms: Vec<Room> = h
                .rooms
                .into_iter()
                .map(|r| Room {
                    uuid: r.uuid,
                    name: r.name,
                    ip: r.ip,
                    model: r.model,
                    version: r.version,
                    volume: 0,
                    muted: false,
                    settings: None,
                    online: None,
                    last_seen: None,
                    problem: String::new(),
                })
                .collect();
            let mut groups: Vec<Group> = h
                .groups
                .into_iter()
                .filter(|g| rooms.iter().any(|r| r.uuid == g.coordinator))
                .map(|g| Group { id: g.coordinator.clone(), coordinator: g.coordinator, members: g.members, ..Default::default() })
                .collect();
            // Any room not in a remembered group stands on its own.
            for r in &rooms {
                if !groups.iter().any(|g| g.members.contains(&r.uuid)) {
                    groups.push(Group { id: r.uuid.clone(), coordinator: r.uuid.clone(), members: vec![r.uuid.clone()], ..Default::default() });
                }
            }
            Household { id: h.id, system: if h.s1 { System::S1 } else { System::S2 }, rooms, groups }
        })
        .filter(|h| !h.rooms.is_empty())
        .collect()
}

#[cfg(test)]
mod unit {
    use super::*;

    fn known(id: &str, last_seen: u64) -> KnownHousehold {
        KnownHousehold { id: id.into(), s1: false, rooms: vec![], groups: vec![], last_seen }
    }

    #[test]
    fn systems_not_seen_for_thirty_days_are_forgotten_but_never_the_last_one() {
        let day = 24 * 3600;
        let now = 100 * day;
        let ids = |l: Vec<KnownHousehold>| l.into_iter().map(|h| h.id).collect::<Vec<_>>();
        let kept = prune(vec![known("home", now - day), known("office", now - 31 * day), known("new", 0)], now);
        assert_eq!(ids(kept), ["new", "home"], "old one goes; one with no date yet counts as seen now");
        let kept = prune(vec![known("older", now - 5 * day), known("newer", now - day), known("same", now - 5 * day)], now);
        assert_eq!(ids(kept), ["newer", "older", "same"], "most recent first; ties keep their order");
        let kept = prune(vec![known("a", now - 90 * day), known("b", now - 40 * day)], now);
        assert_eq!(ids(kept), ["b"], "only the most recently seen survives when all are old");
        let kept = prune(vec![known("a", now - 29 * day), known("b", now - 31 * day)], now);
        assert_eq!(ids(kept), ["a"]);
    }

    #[test]
    fn speakers_are_remembered() {
        crate::test_support::isolate();
        let mut s = crate::model::AppState::default();
        crate::demo::load(&mut s);
        // Demo rooms have no address, and rooms without one are never remembered.
        let path = path().unwrap();
        let _ = std::fs::remove_file(&path);
        save(&s.households);
        assert!(!path.exists(), "nothing to remember without addresses");
        for (i, r) in s.households.iter_mut().flat_map(|h| h.rooms.iter_mut()).enumerate() {
            r.ip = format!("192.168.1.{}", 20 + i);
        }
        save(&s.households);
        let back = load();
        assert_eq!(back.len(), s.households.len());
        for (a, b) in back.iter().zip(&s.households) {
            assert_eq!(a.system, b.system);
            assert_eq!(a.rooms.iter().map(|r| &r.name).collect::<Vec<_>>(), b.rooms.iter().map(|r| &r.name).collect::<Vec<_>>());
            assert_eq!(a.groups.len(), b.groups.len());
        }
    }
}
