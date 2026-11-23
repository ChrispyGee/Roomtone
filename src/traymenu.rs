//! The menu behind the menu-bar icon (Mac) and the notification-area icon (Windows).
//!
//! What the menu shows is worked out here from the app's state (`entries`), and what choosing an
//! item does is `run`, so the two systems share them and they are tested on every platform. Only
//! the icon and the drawing of the menu are system code (`macos_menubar.rs`, `windows_tray.rs`).
//! The menu is built each time it is opened, so it is always current and nothing has to be kept
//! up to date while the window is closed or hidden.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

use crate::backend::{Backend, Cmd};
use crate::model::Transport;

/// What a menu item does. Items carry their place in the list as their tag.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    PlayPause(String),
    Next(String),
    Previous(String),
    Louder(String),
    Quieter(String),
    Mute(String),
    Select(String),
    PauseAll(Vec<String>),
    Open,
    Quit,
}

/// What the menu shows, worked out from the app's state (kept apart from the menu itself so it
/// can be tested): each entry is a title and what choosing it does (`None` is a plain label),
/// with `Sep` between sections.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Label(String),
    Item(String, Action, bool),
    Sub(String, Vec<Entry>),
    Sep,
}

/// The menu for the current state.
pub fn entries(s: &crate::model::AppState) -> Vec<Entry> {
    use Entry::*;
    let mut out = vec![];
    let Some(h) = s.household() else {
        out.push(Label(crate::t!("No speakers found yet").into()));
        out.push(Sep);
        out.push(Item(crate::t!("Open Roomtone").into(), Action::Open, false));
        out.push(Item(crate::t!("Quit Roomtone").into(), Action::Quit, false));
        return out;
    };
    let chosen = s.selected_group.as_ref().and_then(|g| h.groups.iter().find(|x| &x.id == g)).or_else(|| h.groups.first());
    match chosen {
        Some(g) => {
            let room = h.group_name(g);
            let playing = g.now.transport == Transport::Playing;
            if g.now.title.is_empty() {
                out.push(Label(crate::t!("Nothing playing").into()));
            } else {
                out.push(Label(g.now.title.clone()));
                if !g.now.artist.is_empty() {
                    out.push(Label(g.now.artist.clone()));
                }
            }
            out.push(Label(room));
            out.push(Sep);
            out.push(Item(if playing { crate::t!("Pause").into() } else { crate::t!("Play").into() }, Action::PlayPause(g.id.clone()), false));
            out.push(Item(crate::t!("Next").into(), Action::Next(g.id.clone()), false));
            out.push(Item(crate::t!("Previous").into(), Action::Previous(g.id.clone()), false));
            out.push(Sub(
                crate::t!("Volume {n}", n = if g.muted { 0 } else { g.volume }),
                vec![
                    Item(crate::t!("Louder").into(), Action::Louder(g.id.clone()), false),
                    Item(crate::t!("Quieter").into(), Action::Quieter(g.id.clone()), false),
                    Item(crate::t!("Mute").into(), Action::Mute(g.id.clone()), g.muted),
                ],
            ));
            out.push(Sep);
        }
        None => {
            out.push(Label(crate::t!("No rooms found yet").into()));
            out.push(Sep);
        }
    }
    if h.groups.len() > 1 {
        out.push(Label(crate::t!("Rooms").into()));
        for g in &h.groups {
            out.push(Item(h.group_name(g), Action::Select(g.id.clone()), chosen.is_some_and(|c| c.id == g.id)));
        }
        out.push(Sep);
    }
    let playing: Vec<String> = h.groups.iter().filter(|g| g.now.transport == Transport::Playing).map(|g| g.id.clone()).collect();
    if !playing.is_empty() {
        out.push(Item(crate::t!("Pause all rooms").into(), Action::PauseAll(playing), false));
    }
    out.push(Item(crate::t!("Open Roomtone").into(), Action::Open, false));
    out.push(Item(crate::t!("Quit Roomtone").into(), Action::Quit, false));
    out
}

/// Carry out an action.
pub fn run(b: &Backend, a: Action, open: &dyn Fn(), quit: &dyn Fn()) {
    match a {
        Action::PlayPause(g) => {
            let playing = b.lock().find_group(&g).is_some_and(|(_, x)| x.now.transport == Transport::Playing);
            let _ = b.send(if playing { Cmd::Pause(g) } else { Cmd::Play(g) });
        }
        Action::Next(g) => {
            let _ = b.send(Cmd::Next(g));
        }
        Action::Previous(g) => {
            let _ = b.send(Cmd::Previous(g));
        }
        Action::Louder(g) => nudge(b, g, true),
        Action::Quieter(g) => nudge(b, g, false),
        Action::Mute(g) => {
            let muted = b.lock().find_group(&g).is_some_and(|(_, x)| x.muted);
            let _ = b.send(Cmd::GroupMute(g, !muted));
        }
        Action::Select(g) => b.lock().selected_group = Some(g),
        Action::PauseAll(gs) => {
            for g in gs {
                let _ = b.send(Cmd::Pause(g));
            }
        }
        Action::Open => open(),
        Action::Quit => quit(),
    }
}

/// Turn a group up or down by the volume step setting.
fn nudge(b: &Backend, g: String, up: bool) {
    let (now, step) = {
        let s = b.lock();
        (s.find_group(&g).map(|(_, x)| x.volume).unwrap_or(0), s.config.volume_step.max(1))
    };
    let v = if up { now.saturating_add(step).min(100) } else { now.saturating_sub(step) };
    let _ = b.send(Cmd::GroupVolume(g, v));
}

/// What the pop-up player shows (Mac): the chosen room's song, where it is up to and how loud it
/// is, with the rooms to choose from. Worked out here so it can be tested on every system.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Player {
    /// The chosen room's id, empty when there are no rooms.
    pub group: String,
    pub rooms: Vec<(String, String, bool)>,
    pub title: String,
    pub artist: String,
    pub art: Option<String>,
    pub playing: bool,
    pub position: f32,
    /// Zero when the song has no length (a radio station), so there is nothing to seek in.
    pub duration: f32,
    pub volume: u8,
    pub muted: bool,
}

/// The player for the current state.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn player(s: &crate::model::AppState) -> Player {
    let Some(h) = s.household() else { return Player::default() };
    let chosen = s.selected_group.as_ref().and_then(|g| h.groups.iter().find(|x| &x.id == g)).or_else(|| h.groups.first());
    let rooms = h.groups.iter().map(|g| (g.id.clone(), h.group_name(g), chosen.is_some_and(|c| c.id == g.id))).collect();
    let Some(g) = chosen else { return Player { rooms, ..Player::default() } };
    let n = &g.now;
    Player {
        group: g.id.clone(),
        rooms,
        title: n.title.clone(),
        artist: n.artist.clone(),
        art: n.art.clone(),
        playing: n.transport == Transport::Playing,
        position: n.live_position(),
        duration: if n.is_stream { 0.0 } else { n.duration },
        volume: g.volume,
        muted: g.muted,
    }
}

/// Move the chosen room to `seconds` into the song.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn seek(b: &Backend, group: &str, seconds: f32) {
    let _ = b.send(Cmd::Seek(group.to_string(), seconds.max(0.0)));
}

/// Set the chosen room's volume (the normal path, so limits apply).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn set_volume(b: &Backend, group: &str, volume: u8) {
    let _ = b.send(Cmd::GroupVolume(group.to_string(), volume.min(100)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(list: &[Entry]) -> Vec<String> {
        list.iter()
            .map(|e| match e {
                Entry::Label(t) => format!("· {t}"),
                Entry::Item(t, _, ticked) => format!("{}{t}", if *ticked { "✓ " } else { "" }),
                Entry::Sub(t, _) => format!("> {t}"),
                Entry::Sep => "---".into(),
            })
            .collect()
    }

    #[test]
    fn the_menu_shows_what_is_playing_the_rooms_and_the_ways_out() {
        let b = crate::test_support::demo_backend();
        let list = entries(&b.lock());
        let text = flat(&list);
        assert!(text.iter().any(|l| l == "Pause"), "something is playing in the chosen room: {text:?}");
        assert!(text.iter().any(|l| l.starts_with("> Volume ")), "{text:?}");
        assert!(text.contains(&"Open Roomtone".to_string()) && text.contains(&"Quit Roomtone".to_string()));
        assert!(text.contains(&"Pause all rooms".to_string()), "a room is playing, so there is something to pause");
        assert_eq!(list.iter().filter(|e| matches!(e, Entry::Item(_, Action::Select(_), true))).count(), 1, "exactly one room is ticked");
    }

    #[test]
    fn choosing_play_pause_volume_and_a_room_does_what_it_says() {
        let b = crate::test_support::demo_backend();
        let (gid, other, vol) = {
            let s = b.lock();
            let h = s.household().unwrap();
            let g = h.groups.iter().find(|g| g.now.transport == Transport::Playing).unwrap();
            (g.id.clone(), h.groups.iter().find(|x| x.id != g.id).unwrap().id.clone(), g.volume)
        };
        run(&b, Action::PlayPause(gid.clone()), &|| {}, &|| {});
        assert!(crate::test_support::wait_for(5, || b.lock().find_group(&gid).unwrap().1.now.transport != Transport::Playing), "it paused");
        run(&b, Action::Louder(gid.clone()), &|| {}, &|| {});
        assert!(crate::test_support::wait_for(5, || b.lock().find_group(&gid).unwrap().1.volume > vol), "it got louder");
        run(&b, Action::Select(other.clone()), &|| {}, &|| {});
        assert_eq!(b.lock().selected_group.as_deref(), Some(other.as_str()));
    }

    #[test]
    fn the_player_shows_the_chosen_rooms_song_and_every_room() {
        let b = crate::test_support::demo_backend();
        let p = player(&b.lock());
        assert!(!p.group.is_empty() && !p.title.is_empty(), "something is playing in the demo: {p:?}");
        assert!(p.rooms.len() > 1, "the demo has several rooms");
        assert_eq!(p.rooms.iter().filter(|r| r.2).count(), 1, "exactly one room is chosen");
        assert!(p.playing && p.duration > 0.0 && p.position <= p.duration);
        let gid = p.group.clone();
        seek(&b, &gid, 30.0);
        set_volume(&b, &gid, 17);
        assert!(crate::test_support::wait_for(5, || b.lock().find_group(&gid).unwrap().1.volume == 17), "the volume was set");
        {
            let mut s = b.lock();
            s.selected_group = Some("nowhere".into());
        }
        assert!(!player(&b.lock()).group.is_empty(), "an unknown room falls back to the first");
    }
}
