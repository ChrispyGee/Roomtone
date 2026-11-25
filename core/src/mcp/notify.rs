//! Change notifications for the assistant link: an assistant can *subscribe* to a resource
//! (`resources/subscribe`) and is told with `notifications/resources/updated` when it changes,
//! instead of having to ask again and again. Over stdin/stdout the notice is a line of its own;
//! over HTTP it arrives on the event stream opened with `GET /mcp` (see `serve_http`).
//!
//! Changes are found by looking at what Roomtone already knows, once a second, and comparing it
//! with the last look. Ticking values (the song position, the minutes left on a sleep timer)
//! are left out, so a playing song doesn't send a notice every second. Queues and alarms are
//! asked for again now and then while someone is subscribed (a quick read from the speakers).

use super::*;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// Most resources one session may follow.
pub const MAX_SUBSCRIPTIONS: usize = 20;
/// Most sessions kept at once.
pub const MAX_SESSIONS: usize = 64;

const QUEUE_REFRESH: Duration = Duration::from_secs(10);
const ALARM_REFRESH: Duration = Duration::from_secs(30);

/// Values that tick on their own and would make every look a "change".
const TICKING: &[&str] = &["position_seconds", "sleep_timer_minutes_left"];

fn strip_ticking(v: &mut Value) {
    match v {
        Value::Object(o) => {
            for k in TICKING {
                o.remove(*k);
            }
            o.values_mut().for_each(strip_ticking);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_ticking),
        _ => {}
    }
}

fn hash_of(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl Mcp {
    /// Whether this resource can be followed, and which one it is (the room resolved).
    pub(super) fn subscribable(&self, uri: &str) -> Result<(), (i64, String)> {
        match uri {
            "roomtone://status" | "roomtone://now-playing" | "roomtone://alarms" => Ok(()),
            _ => match uri.strip_prefix("roomtone://queue/") {
                Some(room) => {
                    let room = super::extra::percent_decode(room);
                    find_room(&self.b.lock(), &room).map(|_| ()).map_err(|e| (-32002, e))
                }
                None => Err((-32002, format!("Can't follow {uri}. Subscribe to roomtone://status, roomtone://now-playing, roomtone://alarms or roomtone://queue/<room>."))),
            },
        }
    }

    pub(super) fn subscribe(&self, session: &str, uri: &str) -> Result<(), (i64, String)> {
        self.subscribable(uri)?;
        let mut all = self.subs.lock().unwrap_or_else(|e| e.into_inner());
        if !all.contains_key(session) && all.len() >= MAX_SESSIONS {
            return Err((-32000, "Too many assistants are following changes at once.".into()));
        }
        let mine = all.entry(session.to_string()).or_default();
        if !mine.contains(uri) && mine.len() >= MAX_SUBSCRIPTIONS {
            return Err((-32000, format!("At most {MAX_SUBSCRIPTIONS} resources can be followed at once.")));
        }
        mine.insert(uri.to_string());
        Ok(())
    }

    pub(super) fn unsubscribe(&self, session: &str, uri: &str) {
        let mut all = self.subs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mine) = all.get_mut(session) {
            mine.remove(uri);
        }
    }

    pub(super) fn end_session(&self, session: &str) {
        self.subs.lock().unwrap_or_else(|e| e.into_inner()).remove(session);
    }

    pub(super) fn subscriptions(&self, session: &str) -> HashSet<String> {
        self.subs.lock().unwrap_or_else(|e| e.into_inner()).get(session).cloned().unwrap_or_default()
    }

    /// What a followed resource looks like now, from what's already known (nothing is asked of
    /// the speakers here), with the ticking values left out.
    fn snapshot(&self, uri: &str) -> Option<String> {
        if let Some(room) = uri.strip_prefix("roomtone://queue/") {
            let room = super::extra::percent_decode(room);
            let s = self.b.lock();
            let uuid = find_room(&s, &room).ok()?;
            let gid = s.households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&uuid))?.id.clone();
            let (_, g) = s.find_group(&gid)?;
            let titles: Vec<String> = g.queue.iter().flatten().map(|i| format!("{}|{}", i.title, i.subtitle())).collect();
            return Some(format!("{}|{}|{}", g.now.track_no, g.now.is_stream, titles.join("\n")));
        }
        if uri == "roomtone://alarms" {
            return Some(format!("{:?}", self.b.lock().alarms));
        }
        let text = match uri {
            "roomtone://status" => self.call("get_status", &json!({})).ok()?,
            _ => {
                let r = self.read_resource(uri).ok()?;
                r["contents"][0]["text"].as_str()?.to_string()
            }
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(mut v) => {
                strip_ticking(&mut v);
                Some(v.to_string())
            }
            Err(_) => Some(text),
        }
    }

    /// Ask for a fresh queue or alarm list now and then, without waiting for the answer.
    fn refresh(&self, uri: &str) {
        if uri == "roomtone://alarms" {
            self.b.send(Cmd::LoadAlarms);
        } else if let Some(room) = uri.strip_prefix("roomtone://queue/") {
            let room = super::extra::percent_decode(room);
            let gid = {
                let s = self.b.lock();
                find_room(&s, &room).ok().and_then(|u| s.households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&u)).map(|g| g.id.clone()))
            };
            if let Some(gid) = gid {
                self.b.send(Cmd::LoadQueue(gid));
            }
        }
    }
}

/// Remembers what each followed resource looked like at the last look.
pub struct Watcher {
    mcp: Mcp,
    last: HashMap<String, u64>,
    asked: HashMap<String, Instant>,
}

impl Watcher {
    pub fn new(mcp: Mcp) -> Watcher {
        Watcher { mcp, last: HashMap::new(), asked: HashMap::new() }
    }

    /// Look at everything followed; return the ones that changed since the last look. The first
    /// look at a resource only records it.
    pub fn poll(&mut self, followed: &HashSet<String>) -> Vec<String> {
        self.last.retain(|u, _| followed.contains(u));
        self.asked.retain(|u, _| followed.contains(u));
        let mut changed = vec![];
        let mut uris: Vec<&String> = followed.iter().collect();
        uris.sort();
        for uri in uris {
            let every = if uri.starts_with("roomtone://queue/") {
                Some(QUEUE_REFRESH)
            } else if uri == "roomtone://alarms" {
                Some(ALARM_REFRESH)
            } else {
                None
            };
            if let Some(every) = every {
                if self.asked.get(uri).is_none_or(|t| t.elapsed() >= every) {
                    self.asked.insert(uri.clone(), Instant::now());
                    self.mcp.refresh(uri);
                }
            }
            let Some(now) = self.mcp.snapshot(uri).map(|s| hash_of(&s)) else { continue };
            match self.last.insert(uri.clone(), now) {
                Some(before) if before != now => changed.push(uri.clone()),
                _ => {}
            }
        }
        changed
    }
}

pub fn updated(uri: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/resources/updated", "params": {"uri": uri}})
}
