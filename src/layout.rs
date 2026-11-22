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
    fn only_identical_models_pair_and_only_subs_are_subs() {
        assert!(pairable("Sonos One", "sonos one"));
        assert!(!pairable("Sonos One", "Sonos Five"));
        assert!(!pairable("", ""));
        assert!(is_sub("Sonos Sub Mini") && !is_sub("Sonos Beam"));
    }
}
