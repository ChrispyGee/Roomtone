//! Finding speakers: SSDP search on the local network, then the topology each one reports.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use socket2::{Domain, Protocol, Socket, Type};
use std::time::{Duration, Instant};

use super::soap;
use crate::model::{Group, Household, Room, System};

pub struct Found {
    pub ip: String,
    pub household: Option<String>,
}

/// One network connection to search from: its address and, when known, its broadcast address.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Link {
    pub ip: Ipv4Addr,
    pub broadcast: Option<Ipv4Addr>,
}

/// What the system says about one IPv4 address of one network connection.
pub(crate) struct Candidate {
    pub ip: Ipv4Addr,
    pub broadcast: Option<Ipv4Addr>,
    pub up: bool,
}

/// The connections worth searching from: up, not loopback, not self-assigned (169.254.x.x),
/// each address once, and at most `MAX_LINKS` so a machine full of virtual adapters stays quick.
pub(crate) fn pick_links(all: &[Candidate]) -> Vec<Link> {
    const MAX_LINKS: usize = 16;
    let mut links: Vec<Link> = vec![];
    for c in all {
        if !c.up || c.ip.is_loopback() || c.ip.is_link_local() || c.ip.is_unspecified() || links.iter().any(|l| l.ip == c.ip) {
            continue;
        }
        links.push(Link { ip: c.ip, broadcast: c.broadcast });
        if links.len() == MAX_LINKS {
            break;
        }
    }
    links
}

pub(crate) fn local_links() -> Vec<Link> {
    let Ok(all) = if_addrs::get_if_addrs() else { return vec![] };
    let candidates: Vec<Candidate> = all
        .iter()
        .filter_map(|i| match &i.addr {
            if_addrs::IfAddr::V4(a) => Some(Candidate { ip: a.ip, broadcast: a.broadcast, up: i.oper_status != if_addrs::IfOperStatus::Down && !i.is_p2p }),
            _ => None,
        })
        .collect();
    pick_links(&candidates)
}

/// A socket for sending the search out of one connection (`None`: let the system choose).
fn search_socket(link: Option<&Link>) -> Option<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    let bind_ip = link.map_or(Ipv4Addr::UNSPECIFIED, |l| l.ip);
    sock.bind(&SocketAddr::from((bind_ip, 0)).into()).ok()?;
    if let Some(l) = link {
        // Without this the system sends multicast out of its default connection only.
        sock.set_multicast_if_v4(&l.ip).ok()?;
    }
    let _ = sock.set_broadcast(true);
    let _ = sock.set_multicast_ttl_v4(4);
    sock.set_nonblocking(true).ok()?;
    Some(sock.into())
}

/// Look for speakers: the same search is sent out of every network connection (Wi-Fi, cable,
/// and so on), so speakers on a connection that isn't the default one are found too.
pub fn search(wait: Duration) -> Vec<Found> {
    let mut found: Vec<Found> = vec![];
    // The default connection is always included, so this never finds less than a plain search.
    let mut socks: Vec<(UdpSocket, Option<Ipv4Addr>)> = search_socket(None).map(|s| (s, None)).into_iter().collect();
    for link in local_links() {
        if let Some(s) = search_socket(Some(&link)) {
            socks.push((s, link.broadcast));
        }
    }
    if socks.is_empty() {
        return found;
    }
    let msg = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n";
    let start = Instant::now();
    let mut sent = 0;
    let mut buf = [0u8; 2048];
    while start.elapsed() < wait {
        if sent < 3 && start.elapsed() >= Duration::from_millis(400 * sent) {
            for (sock, broadcast) in &socks {
                let mut targets: Vec<SocketAddr> = vec![SocketAddr::from(([239, 255, 255, 250], 1900)), SocketAddr::from(([255, 255, 255, 255], 1900))];
                if let Some(b) = broadcast {
                    targets.push(SocketAddr::from((*b, 1900)));
                }
                for t in &targets {
                    let _ = sock.send_to(msg.as_bytes(), t);
                }
            }
            sent += 1;
        }
        for (sock, _) in &socks {
            // Read what has arrived; any error (nothing yet, or a stray "unreachable") ends this round.
            while let Ok((n, from)) = sock.recv_from(&mut buf) {
                if let Some(f) = parse_reply(&String::from_utf8_lossy(&buf[..n]), from.ip().to_string()) {
                    if !found.iter().any(|x| x.ip == f.ip) {
                        found.push(f);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    found
}

/// A speaker's answer to the search (`None`: something else answered).
fn parse_reply(text: &str, ip: String) -> Option<Found> {
    let mut headers = HashMap::new();
    for line in text.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_uppercase(), v.trim().to_string());
        }
    }
    let is_player = headers.get("ST").is_some_and(|s| s.contains("ZonePlayer")) || headers.get("SERVER").is_some_and(|s| s.contains("Sonos"));
    is_player.then(|| Found { ip, household: headers.get("X-RINCON-HOUSEHOLD").cloned() })
}

pub(crate) struct Description {
    pub model: String,
    pub display_version: String,
    pub software_version: String,
}

pub(crate) fn describe(ip: &str) -> Option<Description> {
    let xml = soap::get_text(&format!("http://{ip}:1400/xml/device_description.xml")).ok()?;
    let doc = roxmltree::Document::parse(&xml).ok()?;
    let field = |name: &str| doc.descendants().find(|n| n.tag_name().name() == name).and_then(|n| n.text()).unwrap_or_default().to_string();
    let display = field("displayName");
    let model = if display.is_empty() { field("modelName") } else { display };
    Some(Description { model, display_version: field("displayVersion"), software_version: field("softwareVersion") })
}

/// S1 firmware stops at 11.x (build 57.x); S2 starts at 12.0 (build 58.x).
pub fn system_from(display_version: &str, software_version: &str) -> System {
    let major = |s: &str| s.split('.').next().and_then(|m| m.trim().parse::<u32>().ok());
    if let Some(m) = major(display_version) {
        return if m < 12 { System::S1 } else { System::S2 };
    }
    match major(software_version) {
        Some(m) if m <= 57 => System::S1,
        _ => System::S2,
    }
}

/// The visible rooms, the groups and each room's build number.
pub(crate) type ZoneGroups = (Vec<Room>, Vec<Group>, Vec<String>);

/// Read a `GetZoneGroupState` reply: the visible rooms, the groups (coordinator first) and each
/// room's build number. Hidden members (bonded surrounds, subs) and bridges are left out.
pub(crate) fn parse_zone_groups(state: &str) -> Result<ZoneGroups, String> {
    let doc = roxmltree::Document::parse(state).map_err(|e| format!("topology: {e}"))?;
    let mut rooms = vec![];
    let mut groups = vec![];
    let mut versions = vec![];
    for zg in doc.descendants().filter(|n| n.tag_name().name() == "ZoneGroup") {
        let coordinator = zg.attribute("Coordinator").unwrap_or_default().to_string();
        let mut members = vec![];
        for m in zg.children().filter(|n| n.tag_name().name() == "ZoneGroupMember") {
            if m.attribute("Invisible") == Some("1") || m.attribute("IsZoneBridge") == Some("1") {
                continue;
            }
            let uuid = m.attribute("UUID").unwrap_or_default().to_string();
            let location = m.attribute("Location").unwrap_or_default();
            let member_ip = location.trim_start_matches("http://").split(':').next().unwrap_or_default().to_string();
            versions.push(m.attribute("SoftwareVersion").unwrap_or_default().to_string());
            rooms.push(Room {
                uuid: uuid.clone(),
                name: m.attribute("ZoneName").unwrap_or("Room").to_string(),
                ip: member_ip,
                model: String::new(),
                version: String::new(),
                volume: 0,
                muted: false,
                settings: None,
                online: None,
                last_seen: None,
                problem: String::new(),
            });
            members.push(uuid);
        }
        if members.is_empty() {
            continue;
        }
        if !members.contains(&coordinator) {
            continue;
        }
        // The coordinator first, then the rest.
        members.sort_by_key(|m| m != &coordinator);
        groups.push(Group { id: coordinator.clone(), coordinator, members, ..Default::default() });
    }
    if rooms.is_empty() {
        return Err("The speaker reported no rooms".into());
    }
    Ok((rooms, groups, versions))
}

/// Ask one speaker for its whole system: rooms and who is grouped with whom.
pub fn household_from(ip: &str, household_id: Option<String>) -> Result<Household, String> {
    household_with(ip, household_id, None)
}

/// Ask a speaker we already know again. What it says now wins over what was remembered: a
/// speaker that was reset or set up again belongs to a different system (or none). The
/// remembered id is used only when the speaker doesn't say.
pub fn household_again(ip: &str, remembered: &str) -> Result<Household, String> {
    household_with(ip, None, Some(remembered))
}

/// The id of a system: the one the speaker announced, else what it says when asked, else (it
/// didn't answer) the remembered one, else a made-up one from its address.
fn choose_id(announced: Option<String>, asked: impl FnOnce() -> Option<String>, remembered: Option<&str>, ip: &str) -> String {
    if let Some(h) = announced.filter(|h| !h.is_empty()) {
        return h;
    }
    match asked() {
        Some(h) if !h.is_empty() => h,
        // The speaker answered but belongs to no system: it was never set up.
        Some(_) => format!("{}{ip}", crate::model::UNSET_PREFIX),
        None => remembered.filter(|h| !h.is_empty()).map(String::from).unwrap_or_else(|| format!("household-{ip}")),
    }
}

fn household_with(ip: &str, announced: Option<String>, remembered: Option<&str>) -> Result<Household, String> {
    let reply = soap::call(ip, soap::ZONE_TOPOLOGY, "GetZoneGroupState", &[])?;
    let state = reply.get("ZoneGroupState").cloned().unwrap_or_default();
    let (mut rooms, mut groups, versions) = parse_zone_groups(&state)?;
    let mut system = None;
    for room in rooms.iter_mut() {
        if let Some(d) = describe(&room.ip) {
            room.online = Some(true);
            room.last_seen = Some(Instant::now());
            room.model = d.model;
            room.version = if d.display_version.is_empty() { d.software_version.clone() } else { d.display_version.clone() };
            system.get_or_insert(system_from(&d.display_version, &d.software_version));
        }
    }
    let system = system.unwrap_or_else(|| system_from("", versions.first().map(String::as_str).unwrap_or("")));
    let id = choose_id(announced, || soap::call(ip, soap::DEVICE_PROPS, "GetHouseholdID", &[]).ok().and_then(|m| m.get("CurrentHouseholdID").cloned()), remembered, ip);
    rooms.sort_by_key(|a| a.name.to_lowercase());
    groups.sort_by_key(|g| rooms.iter().position(|r| r.uuid == g.coordinator).unwrap_or(usize::MAX));
    Ok(Household { id, system, rooms, groups })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speaker_that_changed_system_is_believed_over_the_remembered_one() {
        let asked = |h: &'static str| move || Some(h.to_string());
        assert_eq!(choose_id(Some("Sonos_new".into()), || panic!("announced is enough"), Some("Sonos_old"), "10.0.0.7"), "Sonos_new");
        assert_eq!(choose_id(None, asked("Sonos_new"), Some("Sonos_old"), "10.0.0.7"), "Sonos_new", "asked, it names its new system");
        assert_eq!(choose_id(Some(String::new()), asked("Sonos_new"), None, "10.0.0.7"), "Sonos_new", "an empty announcement is no answer");
        assert_eq!(choose_id(None, asked(""), Some("Sonos_old"), "10.0.0.7"), format!("{}10.0.0.7", crate::model::UNSET_PREFIX), "reset: no system, whatever was remembered");
        assert_eq!(choose_id(None, || None, Some("Sonos_old"), "10.0.0.7"), "Sonos_old", "no answer: keep what was remembered");
        assert_eq!(choose_id(None, || None, None, "10.0.0.7"), "household-10.0.0.7");
    }

    fn cand(ip: [u8; 4], b: Option<[u8; 4]>, up: bool) -> Candidate {
        Candidate { ip: Ipv4Addr::from(ip), broadcast: b.map(Ipv4Addr::from), up }
    }

    #[test]
    fn every_usable_connection_is_searched_once() {
        let links = pick_links(&[
            cand([127, 0, 0, 1], None, true),
            cand([192, 168, 1, 20], Some([192, 168, 1, 255]), true),
            cand([192, 168, 1, 20], Some([192, 168, 1, 255]), true),
            cand([169, 254, 3, 4], None, true),
            cand([10, 0, 0, 5], Some([10, 0, 0, 255]), true),
            cand([172, 16, 0, 9], None, false),
        ]);
        let ips: Vec<String> = links.iter().map(|l| l.ip.to_string()).collect();
        assert_eq!(ips, ["192.168.1.20", "10.0.0.5"]);
        assert_eq!(links[0].broadcast, Some(Ipv4Addr::new(192, 168, 1, 255)));
    }

    #[test]
    fn a_machine_full_of_adapters_is_capped() {
        let many: Vec<Candidate> = (1..=40u8).map(|n| cand([10, 1, n, 2], None, true)).collect();
        assert_eq!(pick_links(&many).len(), 16);
    }

    #[test]
    fn only_speakers_answers_are_kept() {
        let reply = "HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\nX-RINCON-HOUSEHOLD: Sonos_abc\r\n\r\n";
        let f = parse_reply(reply, "10.0.0.7".into()).unwrap();
        assert_eq!((f.ip.as_str(), f.household.as_deref()), ("10.0.0.7", Some("Sonos_abc")));
        assert!(parse_reply("HTTP/1.1 200 OK\r\nST: upnp:rootdevice\r\nSERVER: Linux UPnP/1.0 Foo/1\r\n\r\n", "10.0.0.8".into()).is_none());
    }

    /// Read-only look at the local network: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn finds_speakers() {
        let found = super::search(std::time::Duration::from_millis(3000));
        println!("found {} speaker(s)", found.len());
        if let Some(f) = found.first() {
            match super::household_from(&f.ip, f.household.clone()) {
                Ok(h) => println!("{:?} system, {} rooms, {} groups", h.system, h.rooms.len(), h.groups.len()),
                Err(e) => println!("topology error: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn s1_or_s2_from_versions() {
        assert_eq!(system_from("11.15", ""), System::S1);
        assert_eq!(system_from("16.4", ""), System::S2);
        assert_eq!(system_from("12.0", ""), System::S2);
        assert_eq!(system_from("", "57.19-41110"), System::S1);
        assert_eq!(system_from("", "86.3-61160"), System::S2);
        assert_eq!(system_from("", ""), System::S2, "unknown: assume the current generation");
    }

    const STATE: &str = r#"<ZoneGroupState><ZoneGroups>
<ZoneGroup Coordinator="RINCON_B" ID="RINCON_B:1">
  <ZoneGroupMember UUID="RINCON_A" Location="http://192.168.1.11:1400/xml/device_description.xml" ZoneName="Kitchen" SoftwareVersion="86.3-61160"/>
  <ZoneGroupMember UUID="RINCON_B" Location="http://192.168.1.12:1400/xml/device_description.xml" ZoneName="Living Room" SoftwareVersion="86.3-61160"/>
  <ZoneGroupMember UUID="RINCON_SUB" Location="http://192.168.1.13:1400/xml/device_description.xml" ZoneName="Living Room" Invisible="1"/>
</ZoneGroup>
<ZoneGroup Coordinator="RINCON_C" ID="RINCON_C:2">
  <ZoneGroupMember UUID="RINCON_C" Location="http://192.168.1.14:1400/xml/device_description.xml" ZoneName="Bedroom" SoftwareVersion="86.3-61160"/>
</ZoneGroup>
<ZoneGroup Coordinator="RINCON_BRIDGE" ID="RINCON_BRIDGE:3">
  <ZoneGroupMember UUID="RINCON_BRIDGE" Location="http://192.168.1.15:1400/x" ZoneName="Bridge" IsZoneBridge="1"/>
</ZoneGroup>
</ZoneGroups></ZoneGroupState>"#;

    #[test]
    fn reads_rooms_and_groups() {
        let (rooms, groups, versions) = parse_zone_groups(STATE).unwrap();
        let names: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Kitchen", "Living Room", "Bedroom"], "hidden surrounds/subs and bridges are left out");
        assert_eq!(rooms[0].ip, "192.168.1.11");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].coordinator, "RINCON_B");
        assert_eq!(groups[0].members, vec!["RINCON_B".to_string(), "RINCON_A".to_string()], "coordinator first");
        assert_eq!(groups[0].id, "RINCON_B");
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn no_rooms_is_an_error() {
        assert!(parse_zone_groups("<ZoneGroupState><ZoneGroups/></ZoneGroupState>").is_err());
        assert!(parse_zone_groups("not xml").is_err());
    }
}
