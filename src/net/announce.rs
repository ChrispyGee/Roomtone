//! Listening for speakers announcing themselves (SSDP `NOTIFY`): a speaker that has just
//! started or come back onto the network says so, and the app can look for it at once
//! instead of waiting for the next regular search.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use socket2::{Domain, Protocol, Socket, Type};

/// The multicast group and port speakers announce themselves on.
const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const PORT: u16 = 1900;

/// A speaker saying it is here (or going away).
#[derive(Clone, Debug, PartialEq)]
pub struct Announcement {
    pub ip: String,
    pub alive: bool,
}

/// Read a `NOTIFY` message (`None`: not from a speaker, or not an announcement at all, such as
/// a search from another controller or a notice from a TV).
pub fn parse_notify(text: &str, ip: String) -> Option<Announcement> {
    let mut lines = text.lines();
    if !lines.next()?.trim_start().to_ascii_uppercase().starts_with("NOTIFY") {
        return None;
    }
    let (mut speaker, mut nts) = (false, String::new());
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim().to_ascii_uppercase(), v.trim());
        match k.as_str() {
            "NT" | "USN" if v.contains("ZonePlayer") => speaker = true,
            "SERVER" if v.contains("Sonos") => speaker = true,
            "NTS" => nts = v.to_ascii_lowercase(),
            _ => {}
        }
    }
    if !speaker {
        return None;
    }
    match nts.as_str() {
        "ssdp:alive" => Some(Announcement { ip, alive: true }),
        "ssdp:byebye" => Some(Announcement { ip, alive: false }),
        _ => None,
    }
}

/// Start listening on the announcement port; `on` is called for each speaker announcement.
/// Quietly does nothing when the port can't be shared (the regular searches still run).
pub fn listen(on: impl Fn(Announcement) + Send + 'static) {
    spawn(PORT, on);
}

/// Start listening on `port` (0: any free port) and return the port used.
fn spawn(port: u16, on: impl Fn(Announcement) + Send + 'static) -> Option<u16> {
    let sock = open(port)?;
    let used = sock.local_addr().ok()?.port();
    std::thread::Builder::new()
        .name("announcements".into())
        .spawn(move || {
            let mut buf = [0u8; 2048];
            // An error (such as a stray "unreachable") only ends that read; the loop goes on.
            loop {
                if let Ok((n, from)) = sock.recv_from(&mut buf) {
                    if let Some(a) = parse_notify(&String::from_utf8_lossy(&buf[..n]), from.ip().to_string()) {
                        on(a);
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        })
        .ok()?;
    Some(used)
}

fn open(port: u16) -> Option<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    // Other programs on this computer may listen on the same port, so share it.
    sock.set_reuse_address(true).ok()?;
    #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
    let _ = sock.set_reuse_port(true);
    sock.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)).into()).ok()?;
    // Join the group on the default connection and on every other one that is up.
    let _ = sock.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED);
    for link in super::discovery::local_links() {
        let _ = sock.join_multicast_v4(&GROUP, &link.ip);
    }
    Some(sock.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIVE: &str = "NOTIFY * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nNT: urn:schemas-upnp-org:device:ZonePlayer:1\r\nNTS: ssdp:alive\r\nSERVER: Linux UPnP/1.0 Sonos/57.23-83121 (ZP120)\r\nUSN: uuid:RINCON_000E58AABBCC01400::urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n";

    #[test]
    fn a_speaker_arriving_or_leaving_is_read() {
        assert_eq!(parse_notify(ALIVE, "10.0.0.7".into()), Some(Announcement { ip: "10.0.0.7".into(), alive: true }));
        let bye = ALIVE.replace("ssdp:alive", "ssdp:byebye");
        assert_eq!(parse_notify(&bye, "10.0.0.7".into()), Some(Announcement { ip: "10.0.0.7".into(), alive: false }));
    }

    #[test]
    fn searches_and_other_devices_are_ignored() {
        assert!(parse_notify("M-SEARCH * HTTP/1.1\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n", "10.0.0.7".into()).is_none());
        let tv = "NOTIFY * HTTP/1.1\r\nNT: upnp:rootdevice\r\nNTS: ssdp:alive\r\nSERVER: Linux UPnP/1.0 SomeTV/2\r\n\r\n";
        assert!(parse_notify(tv, "10.0.0.9".into()).is_none());
        assert!(parse_notify(&ALIVE.replace("ssdp:alive", "ssdp:update"), "10.0.0.7".into()).is_none());
        assert!(parse_notify("", "10.0.0.7".into()).is_none());
    }

    #[test]
    fn an_announcement_reaches_the_listener() {
        let (tx, rx) = std::sync::mpsc::channel();
        let Some(port) = spawn(0, move |a| {
            let _ = tx.send(a);
        }) else {
            return; // a locked-down machine that won't open a socket: nothing to test
        };
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        s.send_to(ALIVE.as_bytes(), ("127.0.0.1", port)).unwrap();
        let got = rx.recv_timeout(std::time::Duration::from_secs(3)).expect("no announcement arrived");
        assert_eq!(got, Announcement { ip: "127.0.0.1".into(), alive: true });
    }
}
