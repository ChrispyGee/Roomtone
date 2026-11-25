//! Speaker layouts: stereo pairs, and a soundbar with a sub and rear speakers (experimental).
//!
//! The speakers report how they are bonded in their `GetZoneGroupState` answer: a stereo pair's
//! members carry `ChannelMapSet="RINCON_A:LF,LF;RINCON_B:RF,RF"`, and a soundbar lists its
//! satellites in `HTSatChanMapSet="RINCON_BAR:LF,RF;RINCON_SUB:SW;RINCON_C:LR;RINCON_D:RR"`. The
//! `DeviceProperties` actions `CreateStereoPair`, `SeparateStereoPair`, `AddHTSatellite` and
//! `RemoveHTSatellite` (all present on a real speaker, 2026-11-22) change them. This file only
//! reads and builds those strings; the calls are in `net/speaker.rs`.

use std::collections::HashMap;

/// A stereo pair: two identical speakers playing the left and right channels as one room.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pair {
    pub left: String,
    pub right: String,
    /// Room names, as the speakers report them.
    pub name: String,
    /// The channel map the speakers use for it (needed to split it again).
    pub map: String,
}

/// What a satellite of a soundbar is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Sub,
    RearLeft,
    RearRight,
}

impl Role {
    /// The channel the speakers call it.
    pub fn channel(self) -> &'static str {
        match self {
            Role::Sub => "SW",
            Role::RearLeft => "LR",
            Role::RearRight => "RR",
        }
    }
    pub fn from_channel(c: &str) -> Option<Role> {
        match c {
            "SW" => Some(Role::Sub),
            "LR" => Some(Role::RearLeft),
            "RR" => Some(Role::RearRight),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Satellite {
    pub uuid: String,
    pub name: String,
    pub role: Role,
}

/// A soundbar (or amp) with satellites bonded to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HomeTheatre {
    pub bar: String,
    pub name: String,
    pub satellites: Vec<Satellite>,
    /// The speakers' own channel map for the whole set, bar included.
    pub map: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub pairs: Vec<Pair>,
    pub sets: Vec<HomeTheatre>,
}

impl Layout {
    /// Every room that is part of a bond, with nothing else to do with it as a standalone room.
    pub fn bonded_uuids(&self) -> Vec<String> {
        let mut out = vec![];
        for p in &self.pairs {
            out.push(p.left.clone());
            out.push(p.right.clone());
        }
        for h in &self.sets {
            out.push(h.bar.clone());
            out.extend(h.satellites.iter().map(|s| s.uuid.clone()));
        }
        out
    }
    pub fn set_for(&self, bar: &str) -> Option<&HomeTheatre> {
        self.sets.iter().find(|h| h.bar == bar)
    }
}

/// Read the layout from a `GetZoneGroupState` reply.
pub fn parse(state: &str) -> Layout {
    let Ok(doc) = roxmltree::Document::parse(state) else { return Layout::default() };
    // Every speaker mentioned, visible or not, with its room name.
    let mut names: HashMap<String, String> = HashMap::new();
    for n in doc.descendants().filter(|n| n.attribute("UUID").is_some()) {
        if let Some(name) = n.attribute("ZoneName") {
            names.insert(n.attribute("UUID").unwrap_or_default().to_string(), name.to_string());
        }
    }
    let name_of = |u: &str| names.get(u).cloned().unwrap_or_default();
    let mut out = Layout::default();
    for m in doc.descendants().filter(|n| n.tag_name().name() == "ZoneGroupMember") {
        let uuid = m.attribute("UUID").unwrap_or_default().to_string();
        if let Some(map) = m.attribute("ChannelMapSet").filter(|s| !s.is_empty()) {
            // A stereo pair is listed once on each of its two members; keep one.
            let parts = parse_map(map);
            let left = parts.iter().find(|(_, ch)| ch.first().is_some_and(|c| c == "LF")).map(|(u, _)| u.clone());
            let right = parts.iter().find(|(_, ch)| ch.first().is_some_and(|c| c == "RF")).map(|(u, _)| u.clone());
            if let (Some(left), Some(right)) = (left, right) {
                if parts.len() == 2 && !out.pairs.iter().any(|p| p.left == left && p.right == right) {
                    out.pairs.push(Pair { name: name_of(&left), left, right, map: map.to_string() });
                }
            }
        }
        if let Some(map) = m.attribute("HTSatChanMapSet").filter(|s| !s.is_empty()) {
            let satellites: Vec<Satellite> = parse_map(map)
                .into_iter()
                .filter(|(u, _)| *u != uuid)
                .filter_map(|(u, ch)| ch.first().and_then(|c| Role::from_channel(c)).map(|role| Satellite { name: name_of(&u), uuid: u, role }))
                .collect();
            if !satellites.is_empty() && !out.sets.iter().any(|h| h.bar == uuid) {
                out.sets.push(HomeTheatre { name: name_of(&uuid), bar: uuid, satellites, map: map.to_string() });
            }
        }
    }
    out
}

/// `RINCON_A:LF,RF;RINCON_B:SW` as (room, channels) pairs.
fn parse_map(map: &str) -> Vec<(String, Vec<String>)> {
    map.split(';').filter_map(|part| part.split_once(':')).map(|(u, ch)| (u.trim().to_string(), ch.split(',').map(|c| c.trim().to_string()).collect())).collect()
}

/// The channel map that makes `left` and `right` a stereo pair.
pub fn pair_map(left: &str, right: &str) -> String {
    format!("{left}:LF,LF;{right}:RF,RF")
}

/// The channel map for a soundbar with its existing satellites (`current`, as the speaker reports
/// it, or empty for a bar with none) and the new ones added. The bar plays the front channels.
pub fn set_map(bar: &str, current: &str, add: &[(String, Role)]) -> String {
    let mut parts: Vec<String> = if current.is_empty() { vec![format!("{bar}:LF,RF")] } else { current.split(';').map(str::to_string).collect() };
    for (uuid, role) in add {
        parts.retain(|p| !p.starts_with(&format!("{uuid}:")));
        parts.push(format!("{uuid}:{}", role.channel()));
    }
    parts.join(";")
}

/// One thing to do to reach a scene's layout, with a plain line saying what.
#[derive(Clone, Debug)]
pub struct Step {
    pub text: String,
    pub cmd: crate::backend::Cmd,
}

/// What a scene should remember about how the speakers are set up now.
pub fn capture(now: &Layout) -> crate::config::SceneLayout {
    use crate::config::{ScenePair, SceneSet};
    crate::config::SceneLayout {
        pairs: now.pairs.iter().map(|p| ScenePair { left: p.left.clone(), right: p.right.clone(), name: p.name.clone() }).collect(),
        sets: now
            .sets
            .iter()
            .map(|h| {
                let sat = |r: Role| h.satellites.iter().find(|x| x.role == r).map(|x| x.uuid.clone());
                SceneSet { bar: h.bar.clone(), name: h.name.clone(), sub: sat(Role::Sub), rear_left: sat(Role::RearLeft), rear_right: sat(Role::RearRight) }
            })
            .collect(),
    }
}

/// What it takes to make the speakers match `want`, given how they are set up `now`. Only ever
/// adds: a pair or soundbar speaker that is already right is left alone, and nothing is split or
/// removed. If something in the way would have to be undone first (a speaker already in another
/// pair or on another soundbar), it says so instead of doing it.
pub fn plan(now: &Layout, want: &crate::config::SceneLayout) -> Result<Vec<Step>, String> {
    use crate::backend::Cmd;
    let bonded = now.bonded_uuids();
    let mut steps = vec![];
    for p in &want.pairs {
        if now.pairs.iter().any(|x| x.left == p.left && x.right == p.right) {
            continue;
        }
        if bonded.contains(&p.left) || bonded.contains(&p.right) {
            return Err(format!(
                "{} can't be made a stereo pair: one of its speakers is already part of another set-up. Undo that in Speaker setup first.",
                if p.name.is_empty() { "A pair" } else { &p.name }
            ));
        }
        steps.push(Step { text: format!("Make {} a stereo pair", if p.name.is_empty() { "the pair" } else { &p.name }), cmd: Cmd::CreatePair(p.left.clone(), p.right.clone()) });
    }
    for set in &want.sets {
        let bar_name = if set.name.is_empty() { "the soundbar" } else { &set.name };
        if now.pairs.iter().any(|p| p.left == set.bar || p.right == set.bar) {
            return Err(format!("{bar_name} is part of a stereo pair, so speakers can't be added to it."));
        }
        let existing = now.set_for(&set.bar);
        let wanted = [(Role::Sub, &set.sub), (Role::RearLeft, &set.rear_left), (Role::RearRight, &set.rear_right)];
        let mut add: Vec<(String, Role)> = vec![];
        for (role, uuid) in wanted {
            let Some(uuid) = uuid else { continue };
            let there = existing.and_then(|h| h.satellites.iter().find(|x| x.role == role));
            match there {
                Some(x) if &x.uuid == uuid => {}
                Some(_) => return Err(format!("{bar_name} already has a different {}. Remove it in Speaker setup first.", role_words(role))),
                None => {
                    if bonded.contains(uuid) {
                        return Err(format!("A speaker for the {} of {bar_name} is already part of another set-up. Undo that in Speaker setup first.", role_words(role)));
                    }
                    add.push((uuid.clone(), role));
                }
            }
        }
        if !add.is_empty() {
            let what: Vec<&str> = add.iter().map(|(_, r)| role_words(*r)).collect();
            steps.push(Step { text: format!("Add the {} to {bar_name}", what.join(" and ")), cmd: Cmd::AddSatellites(set.bar.clone(), add) });
        }
    }
    Ok(steps)
}

fn role_words(r: Role) -> &'static str {
    match r {
        Role::Sub => "sub",
        Role::RearLeft => "left rear speaker",
        Role::RearRight => "right rear speaker",
    }
}

/// Can this room be paired with `left`: another visible room of the same model, not yet bonded?
pub fn pairable(left_model: &str, other_model: &str) -> bool {
    !left_model.is_empty() && left_model.eq_ignore_ascii_case(other_model)
}

/// Is this model a sub? (Soundbars and ordinary speakers are not.)
pub fn is_sub(model: &str) -> bool {
    model.to_lowercase().contains("sub")
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = r#"<ZoneGroupState><ZoneGroups>
      <ZoneGroup Coordinator="RINCON_L" ID="RINCON_L:1">
        <ZoneGroupMember UUID="RINCON_L" ZoneName="Study" ChannelMapSet="RINCON_L:LF,LF;RINCON_R:RF,RF"/>
        <ZoneGroupMember UUID="RINCON_R" ZoneName="Study" Invisible="1" ChannelMapSet="RINCON_L:LF,LF;RINCON_R:RF,RF"/>
      </ZoneGroup>
      <ZoneGroup Coordinator="RINCON_BAR" ID="RINCON_BAR:2">
        <ZoneGroupMember UUID="RINCON_BAR" ZoneName="Cinema" HTSatChanMapSet="RINCON_BAR:LF,RF;RINCON_SUB:SW;RINCON_C:LR;RINCON_D:RR">
          <Satellite UUID="RINCON_SUB" ZoneName="Cinema" Invisible="1"/>
          <Satellite UUID="RINCON_C" ZoneName="Cinema" Invisible="1"/>
          <Satellite UUID="RINCON_D" ZoneName="Cinema" Invisible="1"/>
        </ZoneGroupMember>
      </ZoneGroup>
      <ZoneGroup Coordinator="RINCON_K" ID="RINCON_K:3"><ZoneGroupMember UUID="RINCON_K" ZoneName="Kitchen"/></ZoneGroup>
    </ZoneGroups></ZoneGroupState>"#;

    #[test]
    fn a_stereo_pair_and_a_home_theatre_set_are_read_from_the_topology() {
        let l = parse(STATE);
        assert_eq!(l.pairs.len(), 1, "a pair is listed on both its members but counted once");
        assert_eq!((l.pairs[0].left.as_str(), l.pairs[0].right.as_str(), l.pairs[0].name.as_str()), ("RINCON_L", "RINCON_R", "Study"));
        assert_eq!(l.sets.len(), 1);
        let set = &l.sets[0];
        assert_eq!(set.bar, "RINCON_BAR");
        let roles: Vec<(&str, Role)> = set.satellites.iter().map(|s| (s.uuid.as_str(), s.role)).collect();
        assert_eq!(roles, vec![("RINCON_SUB", Role::Sub), ("RINCON_C", Role::RearLeft), ("RINCON_D", Role::RearRight)]);
        assert!(!l.bonded_uuids().contains(&"RINCON_K".to_string()), "a plain room isn't bonded");
        assert!(l.bonded_uuids().contains(&"RINCON_D".to_string()));
    }

    #[test]
    fn nothing_bonded_and_bad_xml_give_an_empty_layout() {
        assert_eq!(parse("<ZoneGroupState><ZoneGroups/></ZoneGroupState>"), Layout::default());
        assert_eq!(parse("not xml"), Layout::default());
    }

    #[test]
    fn the_channel_maps_are_built_the_way_the_speakers_write_them() {
        assert_eq!(pair_map("RINCON_L", "RINCON_R"), "RINCON_L:LF,LF;RINCON_R:RF,RF");
        assert_eq!(set_map("RINCON_BAR", "", &[("RINCON_SUB".into(), Role::Sub)]), "RINCON_BAR:LF,RF;RINCON_SUB:SW");
        assert_eq!(
            set_map("RINCON_BAR", "RINCON_BAR:LF,RF;RINCON_SUB:SW", &[("RINCON_C".into(), Role::RearLeft), ("RINCON_D".into(), Role::RearRight)]),
            "RINCON_BAR:LF,RF;RINCON_SUB:SW;RINCON_C:LR;RINCON_D:RR"
        );
        assert_eq!(set_map("B", "B:LF,RF;S:SW", &[("S".into(), Role::Sub)]), "B:LF,RF;S:SW", "adding what is already there changes nothing");
    }

    #[test]
    fn a_scene_layout_adds_what_is_missing_and_refuses_to_undo_anything() {
        use crate::backend::Cmd;
        use crate::config::{SceneLayout, ScenePair, SceneSet};
        let now = parse(STATE);
        let free = Layout::default();
        let pair = ScenePair { left: "RINCON_K".into(), right: "RINCON_Q".into(), name: "Kitchen".into() };
        let want = SceneLayout { pairs: vec![pair.clone()], sets: vec![] };
        let steps = plan(&free, &want).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(&steps[0].cmd, Cmd::CreatePair(l, r) if l == "RINCON_K" && r == "RINCON_Q"));
        // Already a pair: nothing to do.
        let have = SceneLayout { pairs: vec![ScenePair { left: "RINCON_L".into(), right: "RINCON_R".into(), name: "Study".into() }], sets: vec![] };
        assert!(plan(&now, &have).unwrap().is_empty());
        // One of its speakers is in another pair: refused, not split.
        let clash = SceneLayout { pairs: vec![ScenePair { left: "RINCON_L".into(), right: "RINCON_Q".into(), name: "Study".into() }], sets: vec![] };
        assert!(plan(&now, &clash).unwrap_err().contains("already part of another set-up"));
        // A soundbar that has its sub and rears: nothing to add; a missing sub is added; a different sub is refused.
        let mut set =
            SceneSet { bar: "RINCON_BAR".into(), name: "Cinema".into(), sub: Some("RINCON_SUB".into()), rear_left: Some("RINCON_C".into()), rear_right: Some("RINCON_D".into()) };
        assert!(plan(&now, &SceneLayout { pairs: vec![], sets: vec![set.clone()] }).unwrap().is_empty());
        set.sub = Some("RINCON_OTHER".into());
        assert!(plan(&now, &SceneLayout { pairs: vec![], sets: vec![set.clone()] }).unwrap_err().contains("different sub"));
        let bare = parse(
            r#"<ZoneGroupState><ZoneGroups><ZoneGroup Coordinator="RINCON_BAR" ID="x"><ZoneGroupMember UUID="RINCON_BAR" ZoneName="Cinema"/></ZoneGroup></ZoneGroups></ZoneGroupState>"#,
        );
        let steps = plan(&bare, &SceneLayout { pairs: vec![], sets: vec![SceneSet { sub: Some("RINCON_SUB".into()), ..set.clone() }] }).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(&steps[0].cmd, Cmd::AddSatellites(b, add) if b == "RINCON_BAR" && add.len() == 3));
        // The scene remembers the set-up as it is now.
        let remembered = capture(&now);
        assert_eq!(remembered.pairs.len(), 1);
        assert_eq!(remembered.sets[0].sub.as_deref(), Some("RINCON_SUB"));
        assert!(plan(&now, &remembered).unwrap().is_empty(), "what was captured needs nothing done");
    }

    #[test]
    fn only_identical_models_pair_and_only_subs_are_subs() {
        assert!(pairable("Sonos One", "sonos one"));
        assert!(!pairable("Sonos One", "Sonos Five"));
        assert!(!pairable("", ""));
        assert!(is_sub("Sonos Sub Mini") && !is_sub("Sonos Beam"));
    }
}
