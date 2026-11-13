//! "Can't find my speakers": a read-only check of the usual reasons, with plain advice and a
//! report with no names or addresses in it that can be pasted into a bug report.
//!
//! It looks at this computer's network address, repeats the search for speakers, and asks each
//! speaker it knows of (found, already in the app or added by address) whether it answers on
//! its web port. Nothing is changed on the speakers.

use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    /// Found by the search (rather than only known from before or typed in).
    pub via_search: bool,
    pub reachable: bool,
    /// On the same home network as this computer (judged from the first three numbers of the
    /// addresses); `None` when that can't be told.
    pub same_network: Option<bool>,
    pub model: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Diagnosis {
    /// This computer has an address like 192.168.x.x, 10.x.x.x or 172.16-31.x.x.
    pub private_network: bool,
    /// How many speakers answered the search.
    pub search_answers: usize,
    pub probes: Vec<Probe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advice {
    /// This computer isn't on a home network.
    NoNetwork,
    /// Nothing answered the search and no known speaker answered either.
    NothingAnswered,
    /// The search heard nothing, but speakers known or added by address do answer.
    SearchBlocked,
    /// A speaker answers from a different network than this computer's.
    DifferentNetwork,
    /// Some speakers that are known don't answer.
    SomeSilent,
    /// Everything that was tried answers.
    AllGood,
}

/// What the results mean, most important first.
pub fn advice(d: &Diagnosis) -> Vec<Advice> {
    let reachable = d.probes.iter().filter(|p| p.reachable).count();
    let mut out = vec![];
    if !d.private_network {
        out.push(Advice::NoNetwork);
    }
    if d.search_answers == 0 && reachable == 0 {
        out.push(Advice::NothingAnswered);
    } else if d.search_answers == 0 {
        out.push(Advice::SearchBlocked);
    }
    if d.probes.iter().any(|p| p.same_network == Some(false)) {
        out.push(Advice::DifferentNetwork);
    }
    if reachable > 0 && d.probes.iter().any(|p| !p.reachable) {
        out.push(Advice::SomeSilent);
    }
    if out.is_empty() {
        out.push(Advice::AllGood);
    }
    out
}

pub fn is_private(ip: &str) -> bool {
    let Ok(a) = ip.parse::<std::net::Ipv4Addr>() else { return false };
    a.is_private() && !a.is_loopback()
}

/// The first three numbers of an IPv4 address, or `None`.
fn prefix(ip: &str) -> Option<String> {
    let a: std::net::Ipv4Addr = ip.parse().ok()?;
    let o = a.octets();
    Some(format!("{}.{}.{}", o[0], o[1], o[2]))
}

pub fn same_network(computer: &str, speaker: &str) -> Option<bool> {
    Some(prefix(computer)? == prefix(speaker)?)
}

/// English text for a bug report: no room names, no addresses.
pub fn report(d: &Diagnosis, platform: &str, version: &str) -> String {
    let yes = |b: bool| if b { "yes" } else { "no" };
    let mut s = format!(
        "Roomtone {version} on {platform}\nThis computer has a home-network address: {}\nSpeakers that answered the search: {}\n",
        yes(d.private_network),
        d.search_answers
    );
    s.push_str(&format!("Speakers tried: {}, answering: {}\n", d.probes.len(), d.probes.iter().filter(|p| p.reachable).count()));
    for (i, p) in d.probes.iter().enumerate() {
        let net = match p.same_network {
            Some(b) => yes(b),
            None => "unknown",
        };
        let what = [p.model.as_str(), p.version.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ");
        s.push_str(&format!(
            "- Speaker {}: {}found by search: {}, answers: {}, same network: {}\n",
            i + 1,
            if what.is_empty() { String::new() } else { format!("{what}, ") },
            yes(p.via_search),
            yes(p.reachable),
            net
        ));
    }
    s.push_str(&format!("Result: {}\n", advice(d).iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ")));
    s
}

/// Do the check. `known` are the addresses of speakers the app already has or was given.
pub fn run(known: &[String]) -> Diagnosis {
    let computer = crate::sources::local::lan_ip();
    let found = crate::net::discovery::search(Duration::from_secs(3));
    let mut addrs: Vec<(String, bool)> = found.iter().map(|f| (f.ip.clone(), true)).collect();
    for k in known {
        if !addrs.iter().any(|(a, _)| a == k) {
            addrs.push((k.clone(), false));
        }
    }
    addrs.truncate(16);
    let handles: Vec<_> = addrs
        .iter()
        .cloned()
        .map(|(ip, via_search)| {
            let computer = computer.clone();
            std::thread::spawn(move || {
                let d = crate::net::discovery::describe(&ip);
                Probe {
                    via_search,
                    reachable: d.is_some(),
                    same_network: same_network(&computer, &ip),
                    model: d.as_ref().map(|d| d.model.clone()).unwrap_or_default(),
                    version: d.as_ref().map(|d| d.display_version.clone()).unwrap_or_default(),
                }
            })
        })
        .collect();
    let probes = handles.into_iter().filter_map(|h| h.join().ok()).collect();
    Diagnosis { private_network: is_private(&computer), search_answers: found.len(), probes }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn probe(reachable: bool, same: Option<bool>) -> Probe {
        Probe { via_search: reachable, reachable, same_network: same, model: "Play:1".into(), version: "11.14".into() }
    }

    fn d(private: bool, answers: usize, probes: Vec<Probe>) -> Diagnosis {
        Diagnosis { private_network: private, search_answers: answers, probes }
    }

    #[test]
    fn advice_follows_what_answered() {
        assert_eq!(advice(&d(true, 2, vec![probe(true, Some(true)), probe(true, Some(true))])), vec![Advice::AllGood]);
        assert_eq!(advice(&d(true, 0, vec![])), vec![Advice::NothingAnswered]);
        assert_eq!(advice(&d(false, 0, vec![])), vec![Advice::NoNetwork, Advice::NothingAnswered]);
        assert_eq!(advice(&d(true, 0, vec![probe(true, Some(true))])), vec![Advice::SearchBlocked]);
        assert_eq!(advice(&d(true, 1, vec![probe(true, Some(false))])), vec![Advice::DifferentNetwork]);
        assert_eq!(advice(&d(true, 1, vec![probe(true, Some(true)), probe(false, Some(true))])), vec![Advice::SomeSilent]);
        // Silent speakers and nothing else answering is "nothing answered", not "some silent".
        assert_eq!(advice(&d(true, 0, vec![probe(false, None)])), vec![Advice::NothingAnswered]);
    }

    #[test]
    fn networks_are_compared_by_their_first_three_numbers() {
        assert!(is_private("192.168.1.20") && is_private("10.0.0.5") && is_private("172.16.4.1"));
        assert!(!is_private("127.0.0.1") && !is_private("8.8.8.8") && !is_private("nonsense"));
        assert_eq!(same_network("192.168.1.20", "192.168.1.99"), Some(true));
        assert_eq!(same_network("192.168.1.20", "192.168.2.99"), Some(false));
        assert_eq!(same_network("192.168.1.20", "not an address"), None);
    }

    #[test]
    fn the_report_has_no_addresses_or_names() {
        let r = report(&d(true, 1, vec![probe(true, Some(true)), probe(false, Some(false))]), "macOS", "0.6.0");
        assert!(r.contains("Roomtone 0.6.0 on macOS") && r.contains("Play:1 11.14, found by search: yes, answers: yes"), "{r}");
        assert!(r.contains("DifferentNetwork") && r.contains("SomeSilent"), "{r}");
        assert!(!r.chars().any(|c| c == '@') && !regex_like_ip(&r), "{r}");
    }

    fn regex_like_ip(s: &str) -> bool {
        s.split(|c: char| !(c.is_ascii_digit() || c == '.')).any(|w| w.matches('.').count() == 3 && w.split('.').all(|p| p.parse::<u8>().is_ok()))
    }
}
