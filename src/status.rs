//! How a speaker is connected to the network, from what it reports about its own network
//! interfaces (`/status/ifconfig`, read-only). A speaker that's plugged into the router or
//! another speaker with a cable carries its traffic on `eth0`; one on Wi-Fi or SonosNet carries
//! it on its wireless interface (`ath0`, `ath1`, `ra0`, `wlan0`).
//!
//! The signal strength isn't shown: the speakers' status pages don't give one that can be
//! relied on, so Roomtone doesn't make one up.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    Wired,
    Wireless,
    /// Not enough traffic to tell yet, or an interface layout this doesn't know.
    Unknown,
}

const WIRELESS: &[&str] = &["ath0", "ath1", "ra0", "wlan0", "mlan0"];

/// Packets received on each interface, from `ifconfig` text.
fn received(text: &str) -> Vec<(String, u64)> {
    let mut out = vec![];
    let mut name: Option<String> = None;
    for line in text.lines() {
        if let Some((before, _)) = line.split_once("Link encap") {
            // The first interface shares its line with the page's XML, so take the last word
            // before "Link encap" (after any closing `>`).
            name = before.rsplit(['>', ' ', '\t']).find(|w| !w.is_empty()).map(String::from);
        } else if let Some(rest) = line.trim().strip_prefix("RX packets:") {
            if let (Some(n), Some(count)) = (&name, rest.split_whitespace().next().and_then(|c| c.parse::<u64>().ok())) {
                out.push((n.clone(), count));
            }
        }
    }
    out
}

/// Decide from the packet counts: whichever of the cable (`eth0`) and the radio carried clearly
/// more is how the speaker is connected.
pub fn from_ifconfig(text: &str) -> Connection {
    let rx = received(text);
    let cable = rx.iter().find(|(n, _)| n == "eth0").map(|(_, c)| *c).unwrap_or(0);
    let radio = rx.iter().filter(|(n, _)| WIRELESS.contains(&n.as_str())).map(|(_, c)| *c).max().unwrap_or(0);
    const ENOUGH: u64 = 100;
    if cable >= ENOUGH && cable > radio.saturating_mul(2) {
        Connection::Wired
    } else if radio >= ENOUGH && radio > cable.saturating_mul(2) {
        Connection::Wireless
    } else {
        Connection::Unknown
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn text(eth0: u64, ath0: u64) -> String {
        format!(
            "<Command cmdline='/sbin/ifconfig'>ath0      Link encap:Ethernet  HWaddr 00:00\n          UP BROADCAST RUNNING MULTICAST  MTU:1500  Metric:1\n          RX packets:{ath0} errors:0 dropped:0 overruns:0 frame:0\n\n\
             br0       Link encap:Ethernet  HWaddr 00:00\n          inet addr:192.168.1.5\n          RX packets:15118057 errors:0 dropped:0 overruns:0 frame:0\n\n\
             eth0      Link encap:Ethernet  HWaddr 00:00\n          UP BROADCAST RUNNING MULTICAST  MTU:1500  Metric:1\n          RX packets:{eth0} errors:0 dropped:0 overruns:0 frame:58\n\n\
             lo        Link encap:Local Loopback\n          RX packets:41 errors:0 dropped:0 overruns:0 frame:0\n</Command>"
        )
    }

    #[test]
    fn traffic_shows_how_a_speaker_is_connected() {
        assert_eq!(from_ifconfig(&text(15_347_629, 0)), Connection::Wired);
        assert_eq!(from_ifconfig(&text(12, 4_000_000)), Connection::Wireless);
        assert_eq!(from_ifconfig(&text(30, 20)), Connection::Unknown, "too little to tell");
        assert_eq!(from_ifconfig(&text(5_000, 4_000)), Connection::Unknown, "both busy: not guessed");
        assert_eq!(from_ifconfig("nothing useful"), Connection::Unknown);
    }

    #[test]
    fn interfaces_are_read_by_name() {
        let rx = received(&text(7, 9));
        assert!(rx.contains(&("eth0".to_string(), 7)) && rx.contains(&("ath0".to_string(), 9)) && rx.contains(&("lo".to_string(), 41)), "{rx:?}");
    }

    /// Read-only check against a real speaker (prints how it's connected, no addresses):
    /// `ROOMTONE_SPEAKER=<ip> cargo test real_speaker_connection -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_speaker_connection() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else {
            eprintln!("Set ROOMTONE_SPEAKER=<ip> to run this.");
            return;
        };
        let c = crate::net::speaker::connection(&ip);
        eprintln!("connection: {c:?}");
        assert!(c.is_some(), "the speaker answers its status page");
    }
}
