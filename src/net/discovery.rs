//! Finding speakers: SSDP search on the local network, then the topology each one reports.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use super::soap;
use crate::model::{Group, Household, Room, System};

pub struct Found {
    pub ip: String,
    pub household: Option<String>,
}

pub fn search(wait: Duration) -> Vec<Found> {
    let mut found: Vec<Found> = vec![];
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return found;
    };
    let _ = sock.set_broadcast(true);
    let _ = sock.set_multicast_ttl_v4(4);
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let msg = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n";
    let targets: [SocketAddr; 2] = ["239.255.255.250:1900".parse().unwrap(), "255.255.255.255:1900".parse().unwrap()];
    let start = Instant::now();
    let mut sent = 0;
    let mut buf = [0u8; 2048];
    while start.elapsed() < wait {
        if sent < 3 && start.elapsed() >= Duration::from_millis(400 * sent) {
            for t in &targets {
                let _ = sock.send_to(msg.as_bytes(), t);
            }
            sent += 1;
        }
        let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
        let text = String::from_utf8_lossy(&buf[..n]);
        let mut headers = HashMap::new();
        for line in text.lines().skip(1) {
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_ascii_uppercase(), v.trim().to_string());
            }
        }
        let is_player = headers.get("ST").is_some_and(|s| s.contains("ZonePlayer")) || headers.get("SERVER").is_some_and(|s| s.contains("Sonos"));
        if !is_player {
            continue;
        }
        let ip = from.ip().to_string();
        if found.iter().any(|f| f.ip == ip) {
            continue;
        }
        found.push(Found { ip, household: headers.get("X-RINCON-HOUSEHOLD").cloned() });
    }
    found
}

struct Description {
    model: String,
    display_version: String,
    software_version: String,
}

fn describe(ip: &str) -> Option<Description> {
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

/// Ask one speaker for its whole system: rooms and who is grouped with whom.
pub fn household_from(ip: &str, household_id: Option<String>) -> Result<Household, String> {
    let reply = soap::call(ip, soap::ZONE_TOPOLOGY, "GetZoneGroupState", &[])?;
    let state = reply.get("ZoneGroupState").cloned().unwrap_or_default();
    let doc = roxmltree::Document::parse(&state).map_err(|e| format!("topology: {e}"))?;
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
    let id = match household_id {
        Some(h) => h,
        None => soap::call(ip, soap::DEVICE_PROPS, "GetHouseholdID", &[]).ok().and_then(|m| m.get("CurrentHouseholdID").cloned()).unwrap_or_else(|| format!("household-{ip}")),
    };
    rooms.sort_by_key(|a| a.name.to_lowercase());
    groups.sort_by_key(|g| rooms.iter().position(|r| r.uuid == g.coordinator).unwrap_or(usize::MAX));
    Ok(Household { id, system, rooms, groups })
}

#[cfg(test)]
mod tests {
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
