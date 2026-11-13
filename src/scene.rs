//! Scenes: a named set of things one click does ("Movie night": these rooms together, each at
//! its volume, playing this playlist, stopping in an hour). A scene is run as the same steps an
//! assistant would take, one after another, and stops at the first one that fails.

use crate::backend::Backend;
use crate::config::Scene;
use crate::mcp::Mcp;
use serde_json::json;

/// Do what the scene says. Returns one plain line per step taken.
pub fn apply(b: &Backend, scene: &Scene) -> Result<Vec<String>, String> {
    if scene.rooms.is_empty() {
        return Err("The scene has no rooms.".into());
    }
    let link = Mcp::new(b.clone());
    let mut done = vec![];
    let mut step = |what: &str, tool: &str, args: serde_json::Value| -> Result<(), String> {
        match link.run_tool(tool, &args) {
            Ok(text) => {
                done.push(text);
                Ok(())
            }
            Err(e) => Err(format!("{what}: {e}")),
        }
    };
    let names: Vec<String> = scene.rooms.iter().map(|r| r.room.clone()).collect();
    if scene.group && names.len() >= 2 {
        step("Grouping the rooms", "group_rooms", json!({"rooms": names}))?;
    }
    // Volumes before the music, so it starts at the right level.
    for r in &scene.rooms {
        if let Some(v) = r.volume {
            step(&format!("Setting the volume in {}", r.room), "set_volume", json!({"room": r.room, "volume": v.min(100)}))?;
        }
    }
    let lead = names[0].clone();
    if !scene.play.trim().is_empty() {
        step(&format!("Starting “{}”", scene.play), "play", json!({"room": lead, "name": scene.play}))?;
    }
    if scene.sleep_minutes > 0 {
        step("Setting the sleep timer", "sleep_timer", json!({"room": lead, "minutes": scene.sleep_minutes}))?;
    }
    Ok(done)
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::config::SceneRoom;
    use crate::test_support::demo_backend;

    fn scene(rooms: &[(&str, Option<u8>)], play: &str, sleep: u32) -> Scene {
        Scene {
            name: "Movie night".into(),
            household: String::new(),
            rooms: rooms.iter().map(|(r, v)| SceneRoom { room: r.to_string(), volume: *v }).collect(),
            group: true,
            play: play.into(),
            sleep_minutes: sleep,
        }
    }

    #[test]
    fn a_scene_groups_sets_volumes_plays_and_sets_a_timer() {
        let b = demo_backend();
        let steps = apply(&b, &scene(&[("Living Room", Some(22)), ("Kitchen", Some(35))], "Dinner Party", 45)).expect("the scene runs");
        assert!(steps.len() >= 4, "{steps:?}");
        let s = b.lock();
        let uuid = |name: &str| s.households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.name == name).map(|r| r.uuid.clone()).unwrap();
        let (lr, ki) = (uuid("Living Room"), uuid("Kitchen"));
        let g = s.households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&lr) && g.members.contains(&ki)).expect("grouped together");
        assert!(g.sleep_left.is_some(), "the sleep timer is set");
        let kitchen = s.households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.name == "Kitchen").unwrap();
        assert_eq!(kitchen.volume, 35);
    }

    #[test]
    fn a_scene_with_a_mistake_stops_and_says_which_step() {
        let b = demo_backend();
        let e = apply(&b, &scene(&[("Living Room", None), ("Nowhere", None)], "", 0)).unwrap_err();
        assert!(e.contains("Grouping the rooms") && e.contains("Nowhere"), "{e}");
        let e = apply(&b, &scene(&[("Living Room", None)], "No such favorite", 0)).unwrap_err();
        assert!(e.contains("Starting"), "{e}");
        assert!(apply(&b, &scene(&[], "", 0)).is_err());
    }

    #[test]
    fn scenes_are_saved_replaced_and_deleted_by_name() {
        use crate::backend::Cmd;
        let b = demo_backend();
        let wait = std::time::Duration::from_secs(10);
        b.send_and_wait(Cmd::DeleteScene("Test scene".into()), wait).unwrap();
        let mut sc = scene(&[("Living Room", Some(10))], "", 0);
        sc.name = "Test scene".into();
        b.send_and_wait(Cmd::SaveScene(None, sc.clone()), wait).unwrap();
        assert!(b.lock().config.scenes.iter().any(|x| x.name == "Test scene"));
        sc.name = "Renamed".into();
        b.send_and_wait(Cmd::SaveScene(Some("Test scene".into()), sc), wait).unwrap();
        let names: Vec<String> = b.lock().config.scenes.iter().map(|x| x.name.clone()).collect();
        assert!(names.contains(&"Renamed".to_string()) && !names.contains(&"Test scene".to_string()), "{names:?}");
        assert!(b.send_and_wait(Cmd::ApplyScene("Nope".into()), wait).is_err());
        b.send_and_wait(Cmd::DeleteScene("Renamed".into()), wait).unwrap();
        assert!(!b.lock().config.scenes.iter().any(|x| x.name == "Renamed"));
    }
}
