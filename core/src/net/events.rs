//! Speaker-side events (UPnP eventing): instead of asking the speakers again and again what
//! they're doing, subscribe to them and let them say when something changes.
//!
//! How it works:
//! 1. A small HTTP listener runs on this computer (any free port, home network only).
//! 2. For each speaker we send `SUBSCRIBE` for its playback, volume and (once per system)
//!    room-layout services, giving the listener's address as the callback.
//! 3. A speaker sends `NOTIFY` to that address whenever something changes. We don't read the
//!    details: a notice only says *which speaker and what kind of thing changed*, and the
//!    backend then asks that speaker with the same calls polling uses. That keeps one way of
//!    reading state, and a lost or odd notice can never leave the app showing wrong data.
//! 4. Subscriptions run out (the speakers grant about 30 minutes), so they're renewed at
//!    half-time, and dropped again when they're no longer wanted.
//!
//! Only notices from speakers we subscribed to, carrying the matching subscription id, count.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// What a notice is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Playing, paused, the track, play mode (from a group's coordinator).
    Transport,
    /// Volume, mute, EQ (from any room).
    Rendering,
    /// Which rooms are grouped together (from one room per system).
    Topology,
}

impl Kind {
    fn path(self) -> &'static str {
        match self {
            Kind::Transport => "/MediaRenderer/AVTransport/Event",
            Kind::Rendering => "/MediaRenderer/RenderingControl/Event",
            Kind::Topology => "/ZoneGroupTopology/Event",
        }
    }
}

/// "This speaker said something changed."
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Notice {
    pub ip: String,
    pub kind: Kind,
}

/// A speaker needs to be listening for us within this long of subscribing, or the callback
/// probably isn't getting through (a firewall, or a separate guest network).
const FIRST_NOTICE_WITHIN: Duration = Duration::from_secs(20);
/// Wait this long before trying a speaker again after a failed subscribe.
const RETRY_AFTER: Duration = Duration::from_secs(30);
const MAX_BODY: usize = 256 * 1024;
const MAX_CONNECTIONS: usize = 16;

struct Sub {
    sid: String,
    renew_at: Instant,
    expires: Instant,
    since: Instant,
    got_notice: bool,
}

#[derive(Default)]
struct State {
    subs: HashMap<(String, Kind), Sub>,
    /// Speakers we failed to subscribe to, and when to try again.
    failed: HashMap<(String, Kind), (Instant, String)>,
    dirty: HashSet<Notice>,
    listener: Option<(u16, Arc<AtomicBool>)>,
}

/// The shared hub: subscriptions, the listener and the notices waiting to be acted on.
#[derive(Default)]
pub struct Events {
    state: Mutex<State>,
}

/// The one hub for this run.
pub fn hub() -> &'static Events {
    static HUB: OnceLock<Events> = OnceLock::new();
    HUB.get_or_init(Events::default)
}

/// How things stand, for the Settings page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Speakers (playback or volume) we're subscribed to and have heard from.
    pub working: usize,
    /// Speakers we're subscribed to but haven't heard from (callback not getting through?).
    pub silent: usize,
    /// Speakers we couldn't subscribe to, and the latest reason.
    pub failed: usize,
    pub last_error: String,
}

impl Events {
    /// Is this speaker's playback reliably reported by events right now? Then polling it
    /// can slow right down.
    pub fn reliable(&self, ip: &str, kind: Kind) -> bool {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.subs.get(&(ip.to_string(), kind)).is_some_and(|sub| sub.got_notice && Instant::now() < sub.expires)
    }

    /// Notices that arrived since the last call (each distinct one once).
    pub fn take_notices(&self) -> Vec<Notice> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.dirty.drain().collect()
    }

    pub fn summary(&self) -> Summary {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let mut out = Summary::default();
        let mut ips: HashMap<&str, bool> = HashMap::new();
        for ((ip, _), sub) in &s.subs {
            let ok = sub.got_notice || now.duration_since(sub.since) < FIRST_NOTICE_WITHIN;
            let e = ips.entry(ip.as_str()).or_insert(false);
            *e |= ok;
        }
        for ok in ips.values() {
            if *ok {
                out.working += 1;
            } else {
                out.silent += 1;
            }
        }
        let failed_ips: HashSet<&str> = s.failed.keys().map(|(ip, _)| ip.as_str()).filter(|ip| !ips.contains_key(ip)).collect();
        out.failed = failed_ips.len();
        out.last_error = s.failed.values().map(|(_, e)| e.clone()).next().unwrap_or_default();
        out
    }

    /// Bring the subscriptions in line with `wanted` (every speaker and kind we want events
    /// from), subscribing, renewing and dropping as needed. `speaker_port` is 1400 for real
    /// speakers. Returns false if the listener couldn't start.
    pub fn sync(&self, wanted: &[(String, Kind)], speaker_port: u16) -> bool {
        let Some(port) = self.ensure_listener() else { return false };
        let want: HashSet<&(String, Kind)> = wanted.iter().collect();
        let now = Instant::now();

        // Drop what's no longer wanted.
        let stale: Vec<((String, Kind), String)> = {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            s.failed.retain(|k, _| want.contains(k));
            let keys: Vec<(String, Kind)> = s.subs.keys().filter(|k| !want.contains(k)).cloned().collect();
            keys.into_iter().filter_map(|k| s.subs.remove(&k).map(|sub| (k, sub.sid))).collect()
        };
        for ((ip, kind), sid) in stale {
            let _ = unsubscribe(&format!("{ip}:{speaker_port}"), kind.path(), &sid);
        }

        for key in wanted {
            let (ip, kind) = key;
            enum Do {
                Nothing,
                Renew(String),
                New,
            }
            let action = {
                let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                match s.subs.get(key) {
                    Some(sub) if now < sub.renew_at => Do::Nothing,
                    Some(sub) => Do::Renew(sub.sid.clone()),
                    None if s.failed.get(key).is_some_and(|(at, _)| now < *at) => Do::Nothing,
                    None => Do::New,
                }
            };
            let addr = format!("{ip}:{speaker_port}");
            match action {
                Do::Nothing => {}
                Do::Renew(sid) => match renew(&addr, kind.path(), &sid) {
                    Ok(ttl) => {
                        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(sub) = s.subs.get_mut(key) {
                            sub.renew_at = Instant::now() + ttl / 2;
                            sub.expires = Instant::now() + ttl;
                        }
                    }
                    Err(e) => {
                        // The speaker forgot us (restart, or too slow): start over next time.
                        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                        s.subs.remove(key);
                        s.failed.insert(key.clone(), (Instant::now() + Duration::from_secs(2), e));
                    }
                },
                Do::New => match subscribe(&addr, kind.path(), port) {
                    Ok((sid, ttl)) => {
                        let at = Instant::now();
                        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                        s.failed.remove(key);
                        s.subs.insert(key.clone(), Sub { sid, renew_at: at + ttl / 2, expires: at + ttl, since: at, got_notice: false });
                    }
                    Err(e) => {
                        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                        s.failed.insert(key.clone(), (Instant::now() + RETRY_AFTER, e));
                    }
                },
            }
        }
        true
    }

    /// Drop every subscription and close the listener (events switched off, or quitting).
    pub fn stop(&self, speaker_port: u16) {
        let (subs, listener) = {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            s.failed.clear();
            s.dirty.clear();
            (std::mem::take(&mut s.subs), s.listener.take())
        };
        if let Some((_, stop)) = listener {
            stop.store(true, Ordering::SeqCst);
        }
        for ((ip, kind), sub) in subs {
            let _ = unsubscribe(&format!("{ip}:{speaker_port}"), kind.path(), &sub.sid);
        }
    }

    /// Is the listener running (and on which port)?
    #[cfg(test)]
    pub fn port(&self) -> Option<u16> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).listener.as_ref().map(|(p, _)| *p)
    }

    fn ensure_listener(&self) -> Option<u16> {
        {
            let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((p, _)) = &s.listener {
                return Some(*p);
            }
        }
        let l = TcpListener::bind("0.0.0.0:0").ok()?;
        l.set_nonblocking(true).ok()?;
        let port = l.local_addr().ok()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        self.state.lock().unwrap_or_else(|e| e.into_inner()).listener = Some((port, stop.clone()));
        thread::spawn(move || listen(l, stop));
        Some(port)
    }

    /// A notice arrived with this subscription id from this address.
    fn received(&self, peer: IpAddr, sid: &str) -> bool {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let found = s.subs.iter_mut().find(|((ip, _), sub)| sub.sid == sid && ip.parse::<IpAddr>().is_ok_and(|a| a == peer));
        let Some(((ip, kind), sub)) = found else { return false };
        sub.got_notice = true;
        let notice = Notice { ip: ip.clone(), kind: *kind };
        s.dirty.insert(notice);
        true
    }
}

fn listen(l: TcpListener, stop: Arc<AtomicBool>) {
    let busy = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::SeqCst) {
        match l.accept() {
            Ok((stream, peer)) => {
                if busy.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    continue;
                }
                busy.fetch_add(1, Ordering::SeqCst);
                let busy = busy.clone();
                thread::spawn(move || {
                    let _ = handle(stream, peer);
                    busy.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(_) => thread::sleep(Duration::from_millis(60)),
        }
    }
}

fn handle(mut stream: TcpStream, peer: SocketAddr) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let (head, _body) = read_request(&mut stream)?;
    let ok = match parse_notify(&head) {
        Some(sid) => hub().received(peer.ip(), &sid),
        None => false,
    };
    let reply =
        if ok { "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" } else { "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" };
    stream.write_all(reply.as_bytes())?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

/// Read one HTTP request: the head as text, and (drained, capped) the body.
fn read_request(stream: &mut TcpStream) -> std::io::Result<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(p) = find(&buf, b"\r\n\r\n") {
            break p;
        }
        if buf.len() > 16 * 1024 {
            return Err(std::io::Error::other("request head too long"));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(std::io::Error::other("closed early"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let len: usize = header(&head, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0).min(MAX_BODY);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Ok((head, body))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The value of a header (name compared without regard to case).
fn header(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_string())
    })
}

/// The subscription id of a `NOTIFY` request, if it is one.
fn parse_notify(head: &str) -> Option<String> {
    let first = head.lines().next()?;
    if first.split_whitespace().next()? != "NOTIFY" {
        return None;
    }
    header(head, "sid").filter(|s| !s.is_empty())
}

/// This computer's address as the speaker will see it.
fn local_ip_for(speaker: &str) -> String {
    let via = || -> std::io::Result<String> {
        let s = UdpSocket::bind("0.0.0.0:0")?;
        s.connect(speaker)?;
        Ok(s.local_addr()?.ip().to_string())
    };
    via().unwrap_or_else(|_| crate::sources::local::lan_ip())
}

fn connect(addr: &str) -> Result<TcpStream, String> {
    let sa = addr.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let s = TcpStream::connect_timeout(&sa, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Send one request and return the status code and the reply's headers.
fn request(addr: &str, method: &str, path: &str, headers: &[(&str, String)]) -> Result<(u16, String), String> {
    let mut s = connect(addr)?;
    let mut req = format!("{method} {path} HTTP/1.1\r\nHOST: {addr}\r\nUser-Agent: Roomtone\r\nContent-Length: 0\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if find(&buf, b"\r\n\r\n").is_some() || buf.len() > 16 * 1024 {
                    break;
                }
            }
            Err(e) => {
                if buf.is_empty() {
                    return Err(e.to_string());
                }
                break;
            }
        }
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    let code = text.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).ok_or("no reply")?;
    Ok((code, text))
}

/// "Second-1800" → 1800 s (a missing or odd value counts as 30 minutes; never under a minute).
fn parse_timeout(v: Option<String>) -> Duration {
    let secs = v.and_then(|t| t.rsplit('-').next().and_then(|n| n.trim().parse::<u64>().ok())).unwrap_or(1800);
    Duration::from_secs(secs.max(60))
}

fn subscribe(addr: &str, path: &str, port: u16) -> Result<(String, Duration), String> {
    let ours = local_ip_for(addr);
    let (code, text) = request(addr, "SUBSCRIBE", path, &[("CALLBACK", format!("<http://{ours}:{port}/>")), ("NT", "upnp:event".into()), ("TIMEOUT", "Second-1800".into())])?;
    if code != 200 {
        return Err(format!("the speaker refused to send events (HTTP {code})"));
    }
    let sid = header(&text, "sid").ok_or("the speaker's reply had no subscription id")?;
    Ok((sid, parse_timeout(header(&text, "timeout"))))
}

fn renew(addr: &str, path: &str, sid: &str) -> Result<Duration, String> {
    let (code, text) = request(addr, "SUBSCRIBE", path, &[("SID", sid.to_string()), ("TIMEOUT", "Second-1800".into())])?;
    if code != 200 {
        return Err(format!("renewing was refused (HTTP {code})"));
    }
    Ok(parse_timeout(header(&text, "timeout")))
}

fn unsubscribe(addr: &str, path: &str, sid: &str) -> Result<(), String> {
    request(addr, "UNSUBSCRIBE", path, &[("SID", sid.to_string())]).map(|_| ())
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn reads_the_subscription_id_of_a_notify() {
        let head = "NOTIFY / HTTP/1.1\r\nHOST: 192.168.1.5:50000\r\nNT: upnp:event\r\nNTS: upnp:propchange\r\nSid: uuid:RINCON_1-sub42\r\nSEQ: 0\r\nContent-Length: 12";
        assert_eq!(parse_notify(head).as_deref(), Some("uuid:RINCON_1-sub42"));
        assert_eq!(parse_notify("GET / HTTP/1.1\r\nSID: x"), None, "only NOTIFY counts");
        assert_eq!(parse_notify("NOTIFY / HTTP/1.1\r\nHOST: a"), None, "no id, no notice");
    }

    #[test]
    fn timeouts_are_read_leniently() {
        assert_eq!(parse_timeout(Some("Second-1800".into())), Duration::from_secs(1800));
        assert_eq!(parse_timeout(Some("Second-5".into())), Duration::from_secs(60));
        assert_eq!(parse_timeout(None), Duration::from_secs(1800));
        assert_eq!(parse_timeout(Some("infinite".into())), Duration::from_secs(1800));
    }

    /// A pretend speaker: answers SUBSCRIBE/UNSUBSCRIBE and can send a NOTIFY to the callback.
    struct FakeSpeaker {
        port: u16,
        log: Arc<Mutex<Vec<String>>>,
        callback: Arc<Mutex<Option<String>>>,
    }

    fn fake_speaker() -> FakeSpeaker {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(vec![]));
        let callback = Arc::new(Mutex::new(None));
        let (log2, cb2) = (log.clone(), callback.clone());
        thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut stream = stream;
                let Ok((head, _)) = read_request(&mut stream) else { continue };
                let method = head.split_whitespace().next().unwrap_or("").to_string();
                log2.lock().unwrap().push(format!("{method} {}", head.split_whitespace().nth(1).unwrap_or("")));
                if let Some(cb) = header(&head, "callback") {
                    *cb2.lock().unwrap() = Some(cb.trim_matches(|c| c == '<' || c == '>').to_string());
                }
                let sid = header(&head, "sid").unwrap_or_else(|| "uuid:RINCON_TEST-sub1".into());
                let reply = format!("HTTP/1.1 200 OK\r\nSID: {sid}\r\nTIMEOUT: Second-1800\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        FakeSpeaker { port, log, callback }
    }

    fn notify(callback: &str, sid: &str) -> u16 {
        let addr = callback.trim_start_matches("http://").trim_end_matches('/');
        let body = "<e:propertyset/>";
        let req = format!("NOTIFY / HTTP/1.1\r\nHOST: {addr}\r\nNT: upnp:event\r\nNTS: upnp:propchange\r\nSID: {sid}\r\nSEQ: 0\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(req.as_bytes()).unwrap();
        let mut reply = String::new();
        let _ = s.read_to_string(&mut reply);
        reply.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
    }

    #[test]
    fn subscribes_hears_notices_renews_and_unsubscribes() {
        let speaker = fake_speaker();
        // The listener reports to the global hub, so run the whole flow on it.
        let hub = hub();
        let key = ("127.0.0.1".to_string(), Kind::Transport);
        let wanted = vec![key.clone()];

        assert!(hub.sync(&wanted, speaker.port));
        assert!(hub.port().is_some(), "the listener is up");
        assert_eq!(speaker.log.lock().unwrap().as_slice(), ["SUBSCRIBE /MediaRenderer/AVTransport/Event"]);
        let callback = speaker.callback.lock().unwrap().clone().expect("a callback address was sent");
        assert!(callback.starts_with("http://"), "{callback}");
        assert!(!hub.reliable("127.0.0.1", Kind::Transport), "not reliable until it has been heard from");

        // A notice with the wrong id is turned away; the right one is accepted.
        let callback = format!("http://127.0.0.1:{}/", hub.port().unwrap());
        assert_eq!(notify(&callback, "uuid:SOMEONE-ELSE"), 412);
        assert!(hub.take_notices().is_empty());
        assert_eq!(notify(&callback, "uuid:RINCON_TEST-sub1"), 200);
        assert_eq!(hub.take_notices(), vec![Notice { ip: "127.0.0.1".into(), kind: Kind::Transport }]);
        assert!(hub.take_notices().is_empty(), "each notice is handed over once");
        assert!(hub.reliable("127.0.0.1", Kind::Transport));
        assert_eq!(hub.summary().working, 1);

        // Nothing is due yet, so syncing again sends nothing.
        assert!(hub.sync(&wanted, speaker.port));
        assert_eq!(speaker.log.lock().unwrap().len(), 1);

        // No longer wanted: the subscription is cancelled.
        assert!(hub.sync(&[], speaker.port));
        assert_eq!(speaker.log.lock().unwrap().last().unwrap(), "UNSUBSCRIBE /MediaRenderer/AVTransport/Event");
        assert!(!hub.reliable("127.0.0.1", Kind::Transport));

        hub.stop(speaker.port);
        assert!(hub.port().is_none());
    }

    #[test]
    fn a_speaker_that_refuses_is_reported_and_retried_later() {
        // Nothing is listening on this port.
        let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let ev = Events::default();
        assert!(ev.sync(&[("127.0.0.1".into(), Kind::Rendering)], dead));
        let s = ev.summary();
        assert_eq!((s.working, s.failed), (0, 1));
        assert!(!s.last_error.is_empty());
        ev.stop(dead);
    }

    /// Silent, read-only check against a real speaker:
    /// `ROOMTONE_SPEAKER=<ip> cargo test real_speaker_events -- --ignored --nocapture`.
    /// Subscribes to all three kinds, waits for the speaker's notices, renews once, cancels.
    #[test]
    #[ignore]
    fn real_speaker_events() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else {
            eprintln!("Set ROOMTONE_SPEAKER=<ip> to run this.");
            return;
        };
        let hub = hub();
        let wanted: Vec<(String, Kind)> = [Kind::Transport, Kind::Rendering, Kind::Topology].iter().map(|k| (ip.clone(), *k)).collect();
        assert!(hub.sync(&wanted, 1400), "the listener should start");
        let s = hub.summary();
        eprintln!("after subscribing: working={} silent={} failed={} error={:?}", s.working, s.silent, s.failed, s.last_error);
        let mut heard: Vec<Kind> = vec![];
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until && heard.len() < 3 {
            thread::sleep(Duration::from_millis(250));
            for n in hub.take_notices() {
                if !heard.contains(&n.kind) {
                    eprintln!("first notice for {:?} after {:?}", n.kind, Instant::now() + Duration::from_secs(15) - until);
                    heard.push(n.kind);
                }
            }
        }
        eprintln!("heard from: {heard:?}");
        // Optionally keep listening, to see changes that happen on their own (a song ending, a
        // volume change from another app): ROOMTONE_LISTEN=<seconds>.
        if let Some(secs) = std::env::var("ROOMTONE_LISTEN").ok().and_then(|v| v.parse::<u64>().ok()) {
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            let end = Instant::now() + Duration::from_secs(secs);
            while Instant::now() < end {
                thread::sleep(Duration::from_millis(250));
                for n in hub.take_notices() {
                    *counts.entry(format!("{:?}", n.kind)).or_default() += 1;
                }
            }
            eprintln!("notices over {secs} s of listening: {counts:?}");
        }
        let subs: Vec<(Kind, String, Duration)> = {
            let st = hub.state.lock().unwrap();
            st.subs.iter().map(|((_, k), sub)| (*k, sub.sid.clone(), sub.expires.saturating_duration_since(Instant::now()))).collect()
        };
        for (k, sid, left) in &subs {
            eprintln!("{k:?}: granted about {}s, sid has {} characters", left.as_secs(), sid.len());
            match renew(&format!("{ip}:1400"), k.path(), sid) {
                Ok(ttl) => eprintln!("{k:?}: renewing works, granted {}s", ttl.as_secs()),
                Err(e) => eprintln!("{k:?}: renewing failed: {e}"),
            }
        }
        hub.stop(1400);
        eprintln!("cancelled; listener closed: {}", hub.port().is_none());
        assert_eq!(subs.len(), 3, "all three subscriptions should have been accepted");
    }
}
