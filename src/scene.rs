//! Scenes: a named set of things one click does ("Movie night": these rooms together, each at
//! its volume, playing this playlist, stopping in an hour). A scene is run as the same steps an
//! assistant would take, one after another, and stops at the first one that fails.
//!
//! A scene can also hold a speaker layout (stereo pairs, a soundbar's sub and rear speakers). Making
//! one changes how the speakers are bonded, which stops the music and can clear a measured tuning,
//! so it is done only when `with_layout` is true, which the window sets after the person has
//! confirmed it; an assistant or the command line leaves the layout alone and says so.

use crate::backend::{Backend, Cmd};
use crate::config::Scene;
use crate::mcp::Mcp;
use serde_json::json;

/// Do what the scene says. Returns one plain line per step taken.
pub fn apply(b: &Backend, scene: &Scene, with_layout: bool) -> Result<Vec<String>, String> {
    if scene.rooms.is_empty() {
        return Err("The scene has no rooms.".into());
    }
    let link = Mcp::new(b.clone());
    let mut done = vec![];
    // The speaker layout comes first: it can change which rooms there are.
    if let Some(want) = &scene.layout {
        let wait = std::time::Duration::from_secs(90);
        b.send_and_wait(Cmd::LoadLayout, wait).map_err(|e| format!("Reading the speaker layout: {e}"))?;
        let now = b.lock().layout.clone().unwrap_or_default();
        match crate::layout::plan(&now, want) {
            Ok(steps) if steps.is_empty() => {}
            Ok(_) if !with_layout => done.push("Left the speaker layout as it is: changing it needs a confirmation in the Roomtone window.".to_string()),
            Err(why) if !with_layout => done.push(format!("Left the speaker layout as it is: {why}")),
            Err(why) => return Err(format!("The speaker layout: {why}")),
            Ok(steps) => {
                if !b.lock().config.allow_layout {
                    return Err("Speaker setup is switched off (Settings › Speakers › Speaker setup), so the layout wasn't changed.".into());
                }
                for step in steps {
                    b.send_and_wait(step.cmd, wait).map_err(|e| format!("{}: {e}", step.text))?;
                    done.push(format!("{}.", step.text));
                }
            }
        }
    }
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
    // Sound settings (bass, treble, night sound, speech) before the music too.
    for r in scene.rooms.iter().filter(|r| r.has_sound()) {
        let mut args = serde_json::Map::new();
        args.insert("room".into(), json!(r.room));
        if let Some(v) = r.bass {
            args.insert("bass".into(), json!(v.clamp(-10, 10)));
        }
        if let Some(v) = r.treble {
            args.insert("treble".into(), json!(v.clamp(-10, 10)));
        }
        if let Some(v) = r.night {
            args.insert("night_sound".into(), json!(v));
        }
        if let Some(v) = r.speech {
            args.insert("speech_enhancement".into(), json!(v));
        }
        // Home-theatre settings, held to the speakers' own ranges by the tool.
        if let Some(v) = r.sub_on {
            args.insert("sub_enabled".into(), json!(v));
        }
        if let Some(v) = r.sub_level {
            args.insert("sub_level".into(), json!(v.clamp(-15, 15)));
        }
        if let Some(v) = r.surround_on {
            args.insert("surround_enabled".into(), json!(v));
        }
        if let Some(v) = r.surround_tv {
            args.insert("surround_tv_level".into(), json!(v.clamp(-15, 15)));
        }
        if let Some(v) = r.surround_music {
            args.insert("surround_music_level".into(), json!(v.clamp(-15, 15)));
        }
        if let Some(v) = r.height {
            args.insert("height_level".into(), json!(v.clamp(-10, 10)));
        }
        if let Some(v) = r.delay {
            args.insert("audio_delay".into(), json!(v.min(5)));
        }
        step(&format!("Setting the sound in {}", r.room), "set_sound", serde_json::Value::Object(args))?;
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
            rooms: rooms.iter().map(|(r, v)| SceneRoom { room: r.to_string(), volume: *v, ..Default::default() }).collect(),
            group: true,
            play: play.into(),
            sleep_minutes: sleep,
            layout: None,
        }
    }

    #[test]
    fn a_scene_groups_sets_volumes_plays_and_sets_a_timer() {
        let b = demo_backend();
        let steps = apply(&b, &scene(&[("Living Room", Some(22)), ("Kitchen", Some(35))], "Dinner Party", 45), false).expect("the scene runs");
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
    fn a_scene_can_set_the_sound_of_a_room() {
        use crate::backend::Cmd;
        let b = demo_backend();
        let wait = std::time::Duration::from_secs(10);
        let uuid = b.lock().households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.name == "Kitchen").map(|r| r.uuid.clone()).unwrap();
        b.send_and_wait(Cmd::LoadRoomSettings(uuid.clone()), wait).unwrap();
        let mut sc = scene(&[("Kitchen", None)], "", 0);
        sc.group = false;
        sc.rooms[0].bass = Some(4);
        sc.rooms[0].treble = Some(-3);
        let steps = apply(&b, &sc, false).expect("the scene runs");
        assert!(steps.iter().any(|s| s.contains("bass")), "{steps:?}");
        let st = b.lock().households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.uuid == uuid).and_then(|r| r.settings.clone()).expect("settings loaded");
        assert_eq!((st.bass, st.treble), (4, -3));
        // Out-of-range values are held to the speakers' limits rather than refused.
        sc.rooms[0].bass = Some(90);
        apply(&b, &sc, false).unwrap();
        let st = b.lock().households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.uuid == uuid).and_then(|r| r.settings.clone()).unwrap();
        assert_eq!(st.bass, 10);
    }

    #[test]
    fn a_scene_with_a_mistake_stops_and_says_which_step() {
        let b = demo_backend();
        let e = apply(&b, &scene(&[("Living Room", None), ("Nowhere", None)], "", 0), false).unwrap_err();
        assert!(e.contains("Grouping the rooms") && e.contains("Nowhere"), "{e}");
        let e = apply(&b, &scene(&[("Living Room", None)], "No such favorite", 0), false).unwrap_err();
        assert!(e.contains("Starting"), "{e}");
        assert!(apply(&b, &scene(&[], "", 0), false).is_err());
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

    #[test]
    fn a_scene_makes_its_speaker_layout_only_when_confirmed_and_switched_on() {
        use crate::config::{SceneLayout, ScenePair};
        let b = demo_backend();
        let rooms = |b: &crate::backend::Backend| b.lock().household().map(|h| h.rooms.iter().map(|r| r.uuid.clone()).collect::<Vec<_>>()).unwrap_or_default();
        let mut sc = scene(&[("Living Room", None)], "", 0);
        sc.group = false;
        sc.layout = Some(SceneLayout { pairs: vec![ScenePair { left: "D-KITCHEN".into(), right: "D-STUDY".into(), name: "Kitchen".into() }], sets: vec![] });
        // Not confirmed (an assistant, the command line): the layout is left alone, and it says so.
        let steps = apply(&b, &sc, false).expect("the rest of the scene still runs");
        assert!(steps.iter().any(|l| l.contains("confirmation")), "{steps:?}");
        assert!(rooms(&b).contains(&"D-STUDY".to_string()), "nothing was bonded");
        // Confirmed, but Speaker setup is off: refused.
        b.lock().config.allow_layout = false;
        let e = apply(&b, &sc, true).unwrap_err();
        assert!(e.contains("switched off"), "{e}");
        assert!(rooms(&b).contains(&"D-STUDY".to_string()));
        // Confirmed and on: the pair is made, then the rest of the scene runs.
        b.lock().config.allow_layout = true;
        let steps = apply(&b, &sc, true).expect("the scene runs");
        assert!(steps.iter().any(|l| l.contains("stereo pair")), "{steps:?}");
        assert!(!rooms(&b).contains(&"D-STUDY".to_string()), "the right speaker is part of the pair now");
        // Running it again changes nothing.
        let again = apply(&b, &sc, true).expect("the scene runs");
        assert!(!again.iter().any(|l| l.contains("stereo pair")), "{again:?}");
        b.lock().config.allow_layout = false;
    }

    #[test]
    fn a_scene_can_set_home_theatre_sound() {
        use crate::backend::Cmd;
        let b = demo_backend();
        let uuid = b.lock().households.iter().flat_map(|h| h.rooms.iter()).find(|r| r.name == "Living Room").map(|r| r.uuid.clone()).unwrap();
        b.send_and_wait(Cmd::LoadRoomSettings(uuid), std::time::Duration::from_secs(10)).unwrap();
        let mut sc = scene(&[("Living Room", None)], "", 0);
        sc.group = false;
        sc.rooms[0].sub_level = Some(5);
        sc.rooms[0].surround_tv = Some(-40);
        sc.rooms[0].delay = Some(9);
        assert!(sc.rooms[0].has_sound());
        let steps = apply(&b, &sc, false).expect("the scene runs");
        assert!(steps.iter().any(|l| l.contains("sub") || l.contains("sound")), "{steps:?}");
    }
}
