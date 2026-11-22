//! Volume safeguards: a highest volume for each room, and quiet hours that lower it further.
//!
//! The speakers have no limit of their own that a controller can set (checked on a real S1 and a
//! real S2 speaker: their `RenderingControl` has no such setting), so Roomtone applies the limits
//! to what it sends: the window, the keyboard, the command line, assistants, scenes, timers and
//! new alarms all go through `limit`. Another app, or the buttons on a speaker, can still go higher.
//! Optionally (`keep_down`) Roomtone also turns a room back down when it sees it over its limit.

use crate::backend::{scale_volume, Cmd};
use crate::config::VolumeLimits;
use crate::model::AppState;

/// 22:00 to 07:00 as minutes of the day, for example.
pub fn in_quiet_hours(from: u16, to: u16, now_minute: u32) -> bool {
    let (from, to, now) = (from as u32 % 1440, to as u32 % 1440, now_minute % 1440);
    if from == to {
        false
    } else if from < to {
        now >= from && now < to
    } else {
        now >= from || now < to
    }
}

/// The loudest `room` may be right now (100: no limit). `now_minute` is the minute of the day by
/// the speakers' clock, if known; without it quiet hours can't be applied.
pub fn room_cap(l: &VolumeLimits, room: &str, now_minute: Option<u32>) -> u8 {
    let mut cap = l.rooms.iter().find(|r| r.room == room).map(|r| r.max.min(100)).unwrap_or(100);
    if l.quiet.enabled && now_minute.is_some_and(|m| in_quiet_hours(l.quiet.from, l.quiet.to, m)) {
        cap = cap.min(l.quiet.max.min(100));
    }
    cap
}

/// Is any limit set at all?
pub fn any(l: &VolumeLimits) -> bool {
    !l.rooms.is_empty() || l.quiet.enabled
}

/// Lower `cmd` if it would take a room above its limit. Returns the command to run and, when it
/// was lowered, a sentence saying why. A command that asks for a volume above the limit never
/// *raises* a room that is already louder than the limit; it just leaves it where it is.
pub fn limit(s: &AppState, cmd: Cmd) -> (Cmd, Option<String>) {
    let l = &s.config.volume_limits;
    if !any(l) {
        return (cmd, None);
    }
    let now = s.local_minute();
    match cmd {
        Cmd::RoomVolume(room, v) => {
            let cap = room_cap(l, &room, now);
            if v <= cap {
                return (Cmd::RoomVolume(room, v), None);
            }
            let current = s.find_room(&room).map(|(_, r)| r.volume).unwrap_or(0);
            let allowed = cap.max(current.min(v));
            let note = s.find_room(&room).map(|(_, r)| crate::t!("{room} is limited to {n}", room = r.name, n = cap).to_string());
            (Cmd::RoomVolume(room, allowed), note)
        }
        Cmd::GroupVolume(group, v) => {
            let Some((h, g)) = s.find_group(&group) else { return (Cmd::GroupVolume(group, v), None) };
            let members: Vec<(u8, u8, String)> = g.members.iter().filter_map(|m| h.room(m).map(|r| (r.volume, room_cap(l, &r.uuid, now), r.name.clone()))).collect();
            let fits = |nv: u8| members.iter().all(|(vol, cap, _)| scale_volume(*vol, g.volume, nv) <= *cap);
            if fits(v) {
                return (Cmd::GroupVolume(group, v), None);
            }
            // The loudest the whole group may go; raising never goes below where it already is.
            let mut allowed = (0..=v).rev().find(|nv| fits(*nv)).unwrap_or(0);
            if v > g.volume {
                allowed = allowed.max(g.volume.min(v));
            }
            let tightest = members.iter().filter(|(_, cap, _)| *cap < 100).min_by_key(|(_, cap, _)| *cap).map(|(_, cap, name)| (name.clone(), *cap));
            let note = tightest.map(|(name, cap)| crate::t!("{room} is limited to {n}", room = name, n = cap).to_string());
            (Cmd::GroupVolume(group, allowed), note)
        }
        Cmd::SaveAlarm(mut a) if !a.room_uuid.is_empty() => {
            // Not the quiet hours: an alarm rings at its own time, not now.
            let cap = room_cap(l, &a.room_uuid, None);
            if a.volume > cap {
                let name = s.find_room(&a.room_uuid).map(|(_, r)| r.name.clone()).unwrap_or_default();
                a.volume = cap;
                return (Cmd::SaveAlarm(a), Some(crate::t!("{room} is limited to {n}", room = name, n = cap).to_string()));
            }
            (Cmd::SaveAlarm(a), None)
        }
        Cmd::StartTimer(room, minutes, volume) => {
            let cap = room_cap(l, &room, None);
            if volume > cap {
                let name = s.find_room(&room).map(|(_, r)| r.name.clone()).unwrap_or_default();
                return (Cmd::StartTimer(room, minutes, cap), Some(crate::t!("{room} is limited to {n}", room = name, n = cap).to_string()));
            }
            (Cmd::StartTimer(room, minutes, volume), None)
        }
        other => (other, None),
    }
}

/// Rooms that are louder than their limit right now: (room, limit).
pub fn rooms_over(s: &AppState) -> Vec<(String, u8)> {
    let l = &s.config.volume_limits;
    if !any(l) {
        return vec![];
    }
    let now = s.local_minute();
    s.households
        .iter()
        .flat_map(|h| h.rooms.iter())
        .filter(|r| r.online != Some(false))
        .filter_map(|r| {
            let cap = room_cap(l, &r.uuid, now);
            (r.volume > cap).then(|| (r.uuid.clone(), cap))
        })
        .collect()
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::config::{QuietHours, RoomLimit};
    use crate::model::{Group, Household, Room, System};

    fn room(uuid: &str, name: &str, volume: u8) -> Room {
        Room {
            uuid: uuid.into(),
            name: name.into(),
            ip: String::new(),
            model: String::new(),
            version: String::new(),
            volume,
            muted: false,
            settings: None,
            online: None,
            last_seen: None,
            problem: String::new(),
        }
    }

    fn state(limits: VolumeLimits) -> AppState {
        let mut s = AppState::default();
        let kitchen = Group { id: "K".into(), coordinator: "K".into(), members: vec!["K".into(), "D".into()], volume: 30, ..Default::default() };
        s.households.push(Household { id: "h".into(), system: System::S2, rooms: vec![room("K", "Kitchen", 40), room("D", "Den", 20)], groups: vec![kitchen] });
        s.config.volume_limits = limits;
        s
    }

    fn cap(room: &str, max: u8) -> RoomLimit {
        RoomLimit { room: room.into(), max }
    }

    #[test]
    fn quiet_hours_wrap_past_midnight() {
        assert!(in_quiet_hours(22 * 60, 7 * 60, 23 * 60));
        assert!(in_quiet_hours(22 * 60, 7 * 60, 30));
        assert!(!in_quiet_hours(22 * 60, 7 * 60, 7 * 60));
        assert!(!in_quiet_hours(22 * 60, 7 * 60, 12 * 60));
        assert!(in_quiet_hours(13 * 60, 15 * 60, 14 * 60), "a window in one day");
        assert!(!in_quiet_hours(13 * 60, 15 * 60, 15 * 60), "the end isn't included");
        assert!(!in_quiet_hours(600, 600, 600), "no length, no window");
    }

    #[test]
    fn the_cap_is_the_lower_of_the_room_and_the_quiet_hours() {
        let l = VolumeLimits { rooms: vec![cap("K", 50)], quiet: QuietHours { enabled: true, from: 22 * 60, to: 7 * 60, max: 15 }, keep_down: false };
        assert_eq!(room_cap(&l, "K", Some(12 * 60)), 50);
        assert_eq!(room_cap(&l, "K", Some(23 * 60)), 15);
        assert_eq!(room_cap(&l, "D", Some(12 * 60)), 100, "no limit for a room without one");
        assert_eq!(room_cap(&l, "D", Some(23 * 60)), 15, "quiet hours apply to every room");
        assert_eq!(room_cap(&l, "K", None), 50, "quiet hours can't apply when the time is unknown");
        let off = VolumeLimits { quiet: QuietHours { enabled: false, ..l.quiet.clone() }, ..l };
        assert_eq!(room_cap(&off, "K", Some(23 * 60)), 50);
    }

    #[test]
    fn a_room_is_held_to_its_limit() {
        let s = state(VolumeLimits { rooms: vec![cap("D", 35)], ..Default::default() });
        let (cmd, note) = limit(&s, Cmd::RoomVolume("D".into(), 60));
        assert!(matches!(cmd, Cmd::RoomVolume(_, 35)));
        assert_eq!(note.as_deref(), Some("Den is limited to 35"));
        assert!(matches!(limit(&s, Cmd::RoomVolume("D".into(), 30)), (Cmd::RoomVolume(_, 30), None)), "under the limit is left alone");
        assert!(matches!(limit(&s, Cmd::RoomVolume("K".into(), 90)), (Cmd::RoomVolume(_, 90), None)), "another room has no limit");
    }

    #[test]
    fn asking_for_more_never_raises_a_room_that_is_already_too_loud_but_lowering_is_allowed() {
        let s = state(VolumeLimits { rooms: vec![cap("K", 30)], ..Default::default() });
        // Kitchen is at 40, over its 30 limit.
        assert!(matches!(limit(&s, Cmd::RoomVolume("K".into(), 70)).0, Cmd::RoomVolume(_, 40)), "raising leaves it where it is");
        assert!(matches!(limit(&s, Cmd::RoomVolume("K".into(), 36)).0, Cmd::RoomVolume(_, 36)), "lowering is allowed, even if still over");
        assert!(matches!(limit(&s, Cmd::RoomVolume("K".into(), 10)), (Cmd::RoomVolume(_, 10), None)));
    }

    #[test]
    fn a_group_volume_stops_where_its_loudest_room_reaches_its_limit() {
        // Group volume 30: Kitchen 40, Den 20. Kitchen may not pass 50, so the group tops out near 37.
        let s = state(VolumeLimits { rooms: vec![cap("K", 50)], ..Default::default() });
        let (cmd, note) = limit(&s, Cmd::GroupVolume("K".into(), 60));
        let Cmd::GroupVolume(_, v) = cmd else { panic!("a group volume") };
        assert!((36..=38).contains(&v), "got {v}");
        assert!(scale_volume(40, 30, v) <= 50 && scale_volume(40, 30, v + 1) > 50, "as loud as it may be, no louder");
        assert_eq!(note.as_deref(), Some("Kitchen is limited to 50"));
        assert!(matches!(limit(&s, Cmd::GroupVolume("K".into(), 33)), (Cmd::GroupVolume(_, 33), None)));
        // Already over: raising changes nothing, lowering works.
        let tight = state(VolumeLimits { rooms: vec![cap("K", 30)], ..Default::default() });
        assert!(matches!(limit(&tight, Cmd::GroupVolume("K".into(), 60)).0, Cmd::GroupVolume(_, 30)), "stays at its current volume");
        assert!(matches!(limit(&tight, Cmd::GroupVolume("K".into(), 20)).0, Cmd::GroupVolume(_, 20)));
    }

    #[test]
    fn new_alarms_and_timers_are_held_to_the_room_limit_but_not_the_quiet_hours() {
        let s = state(VolumeLimits { rooms: vec![cap("D", 25)], quiet: QuietHours { enabled: true, from: 0, to: 1440 - 1, max: 5 }, keep_down: false });
        let alarm = crate::model::Alarm { room_uuid: "D".into(), volume: 80, ..Default::default() };
        let Cmd::SaveAlarm(a) = limit(&s, Cmd::SaveAlarm(alarm)).0 else { panic!() };
        assert_eq!(a.volume, 25, "the room's limit, not the quiet hours' 5");
        assert!(matches!(limit(&s, Cmd::StartTimer("D".into(), 10, 90)).0, Cmd::StartTimer(_, 10, 25)));
        assert!(matches!(limit(&s, Cmd::StartTimer("K".into(), 10, 90)).0, Cmd::StartTimer(_, 10, 90)), "no limit in the Kitchen");
    }

    #[test]
    fn nothing_changes_while_no_limit_is_set() {
        let s = state(VolumeLimits::default());
        assert!(matches!(limit(&s, Cmd::RoomVolume("K".into(), 100)), (Cmd::RoomVolume(_, 100), None)));
        assert!(rooms_over(&s).is_empty());
    }

    #[test]
    fn rooms_over_their_limit_are_listed() {
        let s = state(VolumeLimits { rooms: vec![cap("K", 30), cap("D", 25)], ..Default::default() });
        assert_eq!(rooms_over(&s), vec![("K".to_string(), 30)], "Kitchen is at 40, Den at 20");
    }
}
