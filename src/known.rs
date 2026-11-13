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
}

fn path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("speakers.json");
    Some(p)
}

/// Remember these systems for next time.
pub fn save(households: &[Household]) {
    let list: Vec<KnownHousehold> = households
        .iter()
        .map(|h| KnownHousehold {
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
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return vec![] };
    let Ok(list) = serde_json::from_str::<Vec<KnownHousehold>>(&text) else { return vec![] };
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
