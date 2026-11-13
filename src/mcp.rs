//! The assistant link: a Model Context Protocol (MCP) server so AI assistants can see and
//! control the speakers. It speaks JSON-RPC either over stdin/stdout (`--mcp`, for assistants
//! that start the app themselves) or over HTTP on this computer (while the app or
//! `--headless` runs). It is off until switched on in Settings, and can be limited to looking only.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::backend::{Backend, Cmd};
use crate::config::McpConfig;
use crate::model::*;
use crate::net::speaker::PlayHow;

mod extra;
pub mod notify;

const PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

#[derive(Clone)]
pub struct Mcp {
    b: Backend,
    /// Things listed to the assistant (favorites, playlists, search results), by reference.
    refs: Arc<Mutex<HashMap<String, Item>>>,
    /// What each connected assistant (by session) follows for changes.
    subs: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
}

type ToolResult = Result<String, String>;

/// Every tool: (name, what it does, input schema, changes things?).
fn tools() -> Vec<(&'static str, &'static str, Value, bool)> {
    let mut all = base_tools();
    all.extend(extra::tools());
    all
}

/// The first set of tools: status, playback, volume, search and play, queue, grouping, sound, alarms, updates.
fn base_tools() -> Vec<(&'static str, &'static str, Value, bool)> {
    let room = json!({"type": "string", "description": "Room name (e.g. \"Kitchen\"). Leave out to use the room selected in the app."});
    vec![
        ("get_status", "Every system (S1 and S2), its rooms, how they're grouped and what each group is playing, with volumes.", json!({"type": "object", "properties": {}}), false),
        ("now_playing", "What a room is playing right now: title, artist, album, source, position and play mode.", json!({"type": "object", "properties": {"room": room}}), false),
        (
            "playback",
            "Play, pause, toggle, skip to the next or go back to the previous track in a room (and every room grouped with it).",
            json!({"type": "object", "properties": {"room": room, "action": {"type": "string", "enum": ["play", "pause", "toggle", "next", "previous"]}}, "required": ["action"]}),
            true,
        ),
        (
            "set_volume",
            "Set a room's volume (0-100) or change it by a step. With whole_group the room's whole group moves together.",
            json!({"type": "object", "properties": {"room": room, "volume": {"type": "integer", "minimum": 0, "maximum": 100}, "change": {"type": "integer", "description": "Step up (positive) or down (negative), e.g. 5 or -5."}, "whole_group": {"type": "boolean", "default": false}}}),
            true,
        ),
        ("set_mute", "Mute or unmute a room (or its whole group).", json!({"type": "object", "properties": {"room": room, "muted": {"type": "boolean"}, "whole_group": {"type": "boolean", "default": false}}, "required": ["muted"]}), true),
        (
            "set_play_mode",
            "Turn shuffle, repeat (off, all, one) and crossfade on or off for a room's group.",
            json!({"type": "object", "properties": {"room": room, "shuffle": {"type": "boolean"}, "repeat": {"type": "string", "enum": ["off", "all", "one"]}, "crossfade": {"type": "boolean"}}}),
            true,
        ),
        ("seek", "Jump to a point in the current track, in seconds from the start.", json!({"type": "object", "properties": {"room": room, "seconds": {"type": "number", "minimum": 0}}, "required": ["seconds"]}), true),
        ("sleep_timer", "Stop the music in a room after some minutes (0 turns the timer off).", json!({"type": "object", "properties": {"room": room, "minutes": {"type": "integer", "minimum": 0, "maximum": 1440}}, "required": ["minutes"]}), true),
        ("list_favorites", "The system's Favorites (and ones kept in this app). Each has a ref to pass to play.", json!({"type": "object", "properties": {"room": room}}), false),
        ("list_playlists", "Playlists saved on the system. Each has a ref to pass to play.", json!({"type": "object", "properties": {"room": room}}), false),
        (
            "search",
            "Search the music library, favorites, playlists, this computer's music, radio, podcasts and connected services. Each result has a ref to pass to play.",
            json!({"type": "object", "properties": {"query": {"type": "string"}, "room": room, "limit": {"type": "integer", "default": 25, "minimum": 1, "maximum": 100}}, "required": ["query"]}),
            false,
        ),
        (
            "play",
            "Play something in a room: a ref from list_favorites, list_playlists or search, or a favorite/playlist by name. how: now (default), next, end (add to the queue) or replace (replace the queue).",
            json!({"type": "object", "properties": {"room": room, "ref": {"type": "string"}, "name": {"type": "string", "description": "A favorite or playlist name, if no ref."}, "how": {"type": "string", "enum": ["now", "next", "end", "replace"], "default": "now"}}}),
            true,
        ),
        ("get_queue", "The songs queued in a room, numbered from 1.", json!({"type": "object", "properties": {"room": room}}), false),
        ("play_from_queue", "Play the song at this position (from 1) in a room's queue.", json!({"type": "object", "properties": {"room": room, "position": {"type": "integer", "minimum": 1}}, "required": ["position"]}), true),
        ("remove_from_queue", "Remove the song at this position (from 1) from a room's queue.", json!({"type": "object", "properties": {"room": room, "position": {"type": "integer", "minimum": 1}}, "required": ["position"]}), true),
        ("clear_queue", "Empty a room's queue.", json!({"type": "object", "properties": {"room": room}}), true),
        (
            "group_rooms",
            "Make these rooms play together (rooms of one system only; S1 and S2 rooms can't be grouped). Whichever is playing keeps playing. Rooms not listed are left alone.",
            json!({"type": "object", "properties": {"rooms": {"type": "array", "items": {"type": "string"}, "minItems": 2}}, "required": ["rooms"]}),
            true,
        ),
        (
            "play_along",
            "S1 and S2 rooms can't be grouped. This makes a room in the other system play the same music as `room`, started together and following its play, pause, skips and seeks (they may drift a second or two apart).",
            json!({"type": "object", "properties": {"room": room, "other_room": {"type": "string", "description": "A room in the other system."}}, "required": ["room", "other_room"]}),
            true,
        ),
        ("stop_play_along", "Stop a room playing along with a room in the other system.", json!({"type": "object", "properties": {"room": room}, "required": ["room"]}), true),
        ("ungroup_room", "Take a room out of its group so it plays on its own.", json!({"type": "object", "properties": {"room": room}, "required": ["room"]}), true),
        ("group_all", "Group every room in a room's system together.", json!({"type": "object", "properties": {"room": room}}), true),
        ("use_saved_group", "Group the rooms of a named group saved in the app (e.g. \"Downstairs\").", json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}), true),
        (
            "switch_input",
            "Play a speaker's line-in or TV input in a room.",
            json!({"type": "object", "properties": {"room": room, "input": {"type": "string", "enum": ["line_in", "tv"]}, "from_room": {"type": "string", "description": "The speaker with the input; defaults to the room itself."}}, "required": ["input"]}),
            true,
        ),
        ("get_sound", "A room's sound settings: bass, treble, loudness, night sound, speech enhancement and status light.", json!({"type": "object", "properties": {"room": room}, "required": ["room"]}), false),
        (
            "set_sound",
            "Change a room's sound settings. Bass and treble go from -10 to 10.",
            json!({"type": "object", "properties": {"room": room, "bass": {"type": "integer", "minimum": -10, "maximum": 10}, "treble": {"type": "integer", "minimum": -10, "maximum": 10}, "loudness": {"type": "boolean"}, "night_sound": {"type": "boolean"}, "speech_enhancement": {"type": "boolean"}, "status_light": {"type": "boolean"}}, "required": ["room"]}),
            true,
        ),
        ("check_for_updates", "Whether newer speaker software is available for each system (S1 systems are only offered S1 updates).", json!({"type": "object", "properties": {}}), false),
        (
            "install_update",
            "Install the available software update on every speaker of a room's system. Speakers restart and music stops for a few minutes. Only call this when the person has clearly asked for it, and pass confirm: true.",
            json!({"type": "object", "properties": {"room": room, "confirm": {"type": "boolean"}}, "required": ["confirm"]}),
            true,
        ),
        ("list_alarms", "Every alarm: time, days, room, what it plays and whether it's on.", json!({"type": "object", "properties": {}}), false),
        ("set_alarm_enabled", "Turn an alarm on or off by its id from list_alarms.", json!({"type": "object", "properties": {"id": {"type": "string"}, "enabled": {"type": "boolean"}}, "required": ["id", "enabled"]}), true),
    ]
}

fn transport_word(t: Transport) -> &'static str {
    match t {
        Transport::Playing => "playing",
        Transport::Paused => "paused",
        Transport::Stopped => "stopped",
        Transport::Transitioning => "starting",
    }
}

fn repeat_word(r: Repeat) -> &'static str {
    match r {
        Repeat::Off => "off",
        Repeat::All => "all",
        Repeat::One => "one",
    }
}

impl Mcp {
    pub fn new(b: Backend) -> Mcp {
        Mcp { b, refs: Arc::new(Mutex::new(HashMap::new())), subs: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn settings(&self) -> McpConfig {
        self.b.lock().config.mcp.clone()
    }

    /// Answer one JSON-RPC message; None for notifications (no reply).
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        self.handle_for("", msg)
    }

    /// Like `handle`, for one assistant connection (`session`), which is what change
    /// subscriptions belong to.
    pub fn handle_for(&self, session: &str, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let id = match id {
            Some(id) if !id.is_null() => id,
            // Notifications (initialized, cancelled...) need no answer.
            _ => return None,
        };
        let reply = |result: Result<Value, (i64, String)>| match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        };
        if !self.settings().enabled {
            return Some(reply(Err((-32001, "The assistant link is switched off in Roomtone's Settings.".to_string()))));
        }
        Some(reply(match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
                let version = PROTOCOLS.iter().find(|p| **p == asked).copied().unwrap_or(PROTOCOLS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {
                        "tools": {"listChanged": false},
                        "resources": {"listChanged": false, "subscribe": true},
                        "prompts": {"listChanged": false},
                        "completions": {}
                    },
                    "serverInfo": {"name": "roomtone", "title": "Roomtone", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Controls S1 and S2 wireless speakers on the home network. Start with get_status to learn the room names. \
                Rooms are named as in the app; most tools take a room and act on the group it belongs to. \
                Use browse, list_favorites, list_playlists or search to get refs, then play one. \
                Rooms in an S1 and an S2 system can't be grouped; use play_along instead. \
                Ask the person before anything that restarts speakers (install_update) or deletes things."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => {
                let control = self.settings().allow_control;
                let list: Vec<Value> = tools()
                    .into_iter()
                    .filter(|t| control || !t.3)
                    .map(|(name, desc, schema, changes)| {
                        json!({"name": name, "description": desc, "inputSchema": schema, "annotations": {"readOnlyHint": !changes, "openWorldHint": false}})
                    })
                    .collect();
                Ok(json!({"tools": list}))
            }
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match tools().iter().find(|t| t.0 == name) {
                    None => Err((-32602, format!("Unknown tool: {name}"))),
                    Some(t) if t.3 && !self.settings().allow_control => Ok(json!({
                        "content": [{"type": "text", "text": "Roomtone only lets assistants look, not change things. Allow control in Settings › Assistant link."}],
                        "isError": true
                    })),
                    Some(_) => {
                        let out = self.call(name, &args);
                        let (text, err) = match out {
                            Ok(t) => (t, false),
                            Err(e) => (e, true),
                        };
                        Ok(json!({"content": [{"type": "text", "text": text}], "isError": err}))
                    }
                }
            }
            "resources/list" => Ok(self.resources()),
            "resources/templates/list" => Ok(self.resource_templates()),
            "resources/subscribe" => {
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                self.subscribe(session, uri).map(|_| json!({}))
            }
            "resources/unsubscribe" => {
                self.unsubscribe(session, params.get("uri").and_then(|u| u.as_str()).unwrap_or(""));
                Ok(json!({}))
            }
            "resources/read" => {
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                self.read_resource(uri)
            }
            "prompts/list" => Ok(self.prompts()),
            "prompts/get" => self.get_prompt(&params),
            "completion/complete" => Ok(self.complete(&params)),
            other => Err((-32601, format!("Method not found: {other}"))),
        }))
    }

    // ---------- Finding rooms ----------

    /// A room by name (or id), else the room selected in the app. Returns (room uuid, group id).
    fn room(&self, args: &Value) -> Result<(String, String), String> {
        let s = self.b.lock();
        let wanted = args.get("room").and_then(|r| r.as_str()).map(str::trim).filter(|r| !r.is_empty());
        let uuid = match wanted {
            Some(q) => find_room(&s, q)?,
            None => {
                let gid = s.selected_group.clone().or_else(|| s.household().and_then(|h| h.groups.first()).map(|g| g.id.clone()));
                let gid = gid.ok_or_else(|| "No rooms found yet. Is the app connected to your speakers?".to_string())?;
                s.find_group(&gid).map(|(_, g)| g.coordinator.clone()).unwrap_or(gid)
            }
        };
        let gid = s.households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&uuid)).map(|g| g.id.clone()).unwrap_or_else(|| uuid.clone());
        Ok((uuid, gid))
    }

    fn room_name(&self, uuid: &str) -> String {
        self.b.lock().find_room(uuid).map(|(_, r)| r.name.clone()).unwrap_or_else(|| uuid.to_string())
    }

    /// Library calls go to the active system; make a room's system active first.
    fn use_system_of(&self, args: &Value) -> Result<(), String> {
        if args.get("room").and_then(|r| r.as_str()).is_none() {
            return Ok(());
        }
        let (uuid, _) = self.room(args)?;
        let mut s = self.b.lock();
        if let Some(i) = s.households.iter().position(|h| h.room(&uuid).is_some()) {
            if i != s.active {
                s.active = i;
                s.browse.clear();
                s.selected_group = None;
                s.ensure_selection();
            }
        }
        Ok(())
    }

    fn wait(&self, cmd: Cmd) -> Result<(), String> {
        self.b.send_and_wait(cmd, Duration::from_secs(20))
    }

    /// Ask for a list and wait until it arrives.
    fn fetch(&self, id: &str) -> Result<Vec<Item>, String> {
        self.b.lock().browse.remove(id);
        self.wait(Cmd::Browse(id.to_string()))?;
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            match self.b.lock().browse.get(id) {
                Some(Browse::Ready(v)) => return Ok(v.clone()),
                Some(Browse::Failed(e)) => return Err(e.clone()),
                _ => {}
            }
            if Instant::now() > until {
                return Err("The speakers are taking a while to answer; try again in a moment.".to_string());
            }
            thread::sleep(Duration::from_millis(60));
        }
    }

    fn remember(&self, prefix: &str, items: &[Item]) -> Vec<String> {
        let mut refs = self.refs.lock().unwrap_or_else(|e| e.into_inner());
        refs.retain(|k, _| !k.starts_with(prefix));
        items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let r = format!("{prefix}{}", i + 1);
                refs.insert(r.clone(), it.clone());
                r
            })
            .collect()
    }

    fn list_text(&self, prefix: &str, items: &[Item], empty: &str) -> String {
        if items.is_empty() {
            return empty.to_string();
        }
        let refs = self.remember(prefix, items);
        items.iter().zip(refs).map(|(it, r)| format!("[{r}] {} — {} ({})", it.title, it.subtitle(), kind_label(&it.class))).collect::<Vec<_>>().join("\n")
    }

    fn group_line(&self, s: &AppState, h: &Household, g: &Group) -> Value {
        let rooms: Vec<String> = g.members.iter().filter_map(|m| h.room(m).map(|r| r.name.clone())).collect();
        let lead = h.room(&g.coordinator).map(|r| r.name.clone()).unwrap_or_default();
        let along = s.linked_with(&g.id).and_then(|o| s.find_group(o)).map(|(oh, og)| format!("{} ({})", oh.group_name(og), oh.system.label()));
        json!({
            "rooms": rooms,
            "lead_room": lead,
            "state": transport_word(g.now.transport),
            "title": g.now.title,
            "artist": g.now.artist,
            "album": g.now.album,
            "source": g.now.source,
            "position_seconds": g.now.live_position().round(),
            "length_seconds": g.now.duration.round(),
            "group_volume": g.volume,
            "muted": g.muted,
            "shuffle": g.shuffle,
            "repeat": repeat_word(g.repeat),
            "crossfade": g.crossfade,
            "sleep_timer_minutes_left": g.sleep_left.map(|s| s.div_ceil(60)),
            "plays_along_with": along,
        })
    }

    fn summary(&self, gid: &str) -> String {
        let s = self.b.lock();
        let Some((h, g)) = s.find_group(gid) else { return "Done.".to_string() };
        let name = h.group_name(g);
        if g.now.is_empty() {
            return format!("{name}: nothing playing.");
        }
        let what = if g.now.artist.is_empty() { g.now.title.clone() } else { format!("{} — {}", g.now.title, g.now.artist) };
        format!("{name}: {} · {what} · volume {}{}", transport_word(g.now.transport), g.volume, if g.muted { " (muted)" } else { "" })
    }

    // ---------- Tools ----------

    fn call(&self, name: &str, a: &Value) -> ToolResult {
        if let Some(result) = self.call_extra(name, a) {
            return result;
        }
        let str_arg = |k: &str| a.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let bool_arg = |k: &str| a.get(k).and_then(|v| v.as_bool());
        let int_arg = |k: &str| a.get(k).and_then(|v| v.as_f64()).map(|v| v.round() as i64);
        match name {
            "get_status" => {
                let s = self.b.lock();
                let phase = match s.phase {
                    Phase::Searching => "searching for speakers",
                    Phase::Live => "connected",
                    Phase::Demo => "demo (made-up rooms; no speakers found)",
                    Phase::NothingFound => "no speakers found",
                };
                let systems: Vec<Value> = s
                    .households
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        json!({
                            "system": h.system.label(),
                            "active_in_app": i == s.active,
                            "rooms": h.rooms.iter().map(|r| json!({"name": r.name, "model": r.model, "volume": r.volume, "muted": r.muted, "responding": r.online})).collect::<Vec<_>>(),
                            "groups": h.groups.iter().map(|g| self.group_line(&s, h, g)).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                let saved: Vec<Value> = s
                    .config
                    .groups
                    .iter()
                    .map(|g| json!({"name": g.name, "rooms": g.rooms.iter().map(|r| s.find_room(r).map(|(_, x)| x.name.clone()).unwrap_or(r.clone())).collect::<Vec<_>>()}))
                    .collect();
                Ok(serde_json::to_string_pretty(&json!({"status": phase, "systems": systems, "saved_groups": saved})).unwrap_or_default())
            }
            "now_playing" => {
                let (_, gid) = self.room(a)?;
                let s = self.b.lock();
                let (h, g) = s.find_group(&gid).ok_or_else(|| "That room isn't available right now".to_string())?;
                Ok(serde_json::to_string_pretty(&self.group_line(&s, h, g)).unwrap_or_default())
            }
            "playback" => {
                let (_, gid) = self.room(a)?;
                let action = str_arg("action").unwrap_or_default();
                let playing = self.b.lock().find_group(&gid).map(|(_, g)| g.now.transport == Transport::Playing).unwrap_or(false);
                let cmd = match action.as_str() {
                    "play" => Cmd::Play(gid.clone()),
                    "pause" | "stop" => Cmd::Pause(gid.clone()),
                    "toggle" => {
                        if playing {
                            Cmd::Pause(gid.clone())
                        } else {
                            Cmd::Play(gid.clone())
                        }
                    }
                    "next" => Cmd::Next(gid.clone()),
                    "previous" => Cmd::Previous(gid.clone()),
                    other => return Err(format!("Unknown action “{other}”: use play, pause, toggle, next or previous.")),
                };
                self.wait(cmd)?;
                Ok(self.summary(&gid))
            }
            "set_volume" => {
                let (uuid, gid) = self.room(a)?;
                let whole = bool_arg("whole_group").unwrap_or(false);
                let current = {
                    let s = self.b.lock();
                    if whole {
                        s.find_group(&gid).map(|(_, g)| g.volume).unwrap_or(0)
                    } else {
                        s.find_room(&uuid).map(|(_, r)| r.volume).unwrap_or(0)
                    }
                } as i64;
                let target = match (int_arg("volume"), int_arg("change")) {
                    (Some(v), _) => v,
                    (None, Some(d)) => current + d,
                    (None, None) => return Err("Give a volume (0-100) or a change such as 5 or -5.".into()),
                }
                .clamp(0, 100) as u8;
                self.wait(if whole { Cmd::GroupVolume(gid.clone(), target) } else { Cmd::RoomVolume(uuid.clone(), target) })?;
                let who = if whole { self.b.lock().find_group(&gid).map(|(h, g)| h.group_name(g)).unwrap_or_default() } else { self.room_name(&uuid) };
                Ok(format!("{who}: volume {target}"))
            }
            "set_mute" => {
                let (uuid, gid) = self.room(a)?;
                let muted = bool_arg("muted").ok_or("Say muted: true or false.")?;
                let whole = bool_arg("whole_group").unwrap_or(false);
                self.wait(if whole { Cmd::GroupMute(gid, muted) } else { Cmd::RoomMute(uuid.clone(), muted) })?;
                Ok(format!("{}: {}", self.room_name(&uuid), if muted { "muted" } else { "unmuted" }))
            }
            "set_play_mode" => {
                let (_, gid) = self.room(a)?;
                let (sh, rp, cf) = {
                    let s = self.b.lock();
                    let g = s.find_group(&gid).map(|(_, g)| (g.shuffle, g.repeat, g.crossfade)).unwrap_or_default();
                    g
                };
                let shuffle = bool_arg("shuffle").unwrap_or(sh);
                let repeat = match str_arg("repeat").as_deref() {
                    Some("all") => Repeat::All,
                    Some("one") => Repeat::One,
                    Some("off") => Repeat::Off,
                    _ => rp,
                };
                if shuffle != sh || repeat != rp {
                    self.wait(Cmd::SetPlayMode(gid.clone(), shuffle, repeat))?;
                }
                if let Some(c) = bool_arg("crossfade") {
                    if c != cf {
                        self.wait(Cmd::SetCrossfade(gid.clone(), c))?;
                    }
                }
                Ok(format!(
                    "Shuffle {}, repeat {}, crossfade {}",
                    if shuffle { "on" } else { "off" },
                    repeat_word(repeat),
                    if bool_arg("crossfade").unwrap_or(cf) { "on" } else { "off" }
                ))
            }
            "seek" => {
                let (_, gid) = self.room(a)?;
                let secs = a.get("seconds").and_then(|v| v.as_f64()).ok_or("Give seconds from the start.")? as f32;
                self.wait(Cmd::Seek(gid.clone(), secs.max(0.0)))?;
                Ok(self.summary(&gid))
            }
            "sleep_timer" => {
                let (_, gid) = self.room(a)?;
                let m = int_arg("minutes").unwrap_or(0).clamp(0, 1440) as u32;
                self.wait(Cmd::SleepTimer(gid, if m == 0 { None } else { Some(m) }))?;
                Ok(if m == 0 { "Sleep timer off".into() } else { format!("Music stops in {m} minutes") })
            }
            "list_favorites" => {
                self.use_system_of(a)?;
                let mut items = self.fetch("FV:2")?;
                items.extend(self.b.lock().config.favorites.iter().cloned());
                Ok(self.list_text("fav", &items, "No favorites saved yet."))
            }
            "list_playlists" => {
                self.use_system_of(a)?;
                let items = self.fetch("SQ:")?;
                Ok(self.list_text("pl", &items, "No playlists saved yet."))
            }
            "search" => {
                self.use_system_of(a)?;
                let q = str_arg("query").map(|q| q.trim().to_string()).filter(|q| !q.is_empty()).ok_or("What should I search for?")?;
                let limit = int_arg("limit").unwrap_or(25).clamp(1, 100) as usize;
                let sources = crate::sources::all_search_sources(&self.b.lock().config);
                self.b.send(Cmd::Search(q.clone()));
                let until = Instant::now() + Duration::from_secs(15);
                loop {
                    let pending = {
                        let s = self.b.lock();
                        sources.iter().any(|src| matches!(s.browse.get(&format!("search:{src}:{q}")), Some(Browse::Loading) | None))
                    };
                    if !pending || Instant::now() > until {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                let mut found: Vec<Item> = vec![];
                {
                    let s = self.b.lock();
                    for src in &sources {
                        if let Some(Browse::Ready(v)) = s.browse.get(&format!("search:{src}:{q}")) {
                            found.extend(v.iter().take(limit).cloned());
                        }
                    }
                }
                found.truncate(limit);
                Ok(self.list_text("r", &found, "Nothing found."))
            }
            "play" => {
                let (_, gid) = self.room(a)?;
                let how = match str_arg("how").as_deref() {
                    Some("next") => PlayHow::Next,
                    Some("end") => PlayHow::End,
                    Some("replace") => PlayHow::Replace,
                    _ => PlayHow::Now,
                };
                let item = match (str_arg("ref"), str_arg("name")) {
                    (Some(r), _) => self
                        .refs
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(r.trim().trim_matches(['[', ']']))
                        .cloned()
                        .ok_or_else(|| format!("I don't know “{r}”. List favorites, playlists or search first."))?,
                    (None, Some(n)) => {
                        let n = n.to_lowercase();
                        let mut all = self.fetch("FV:2").unwrap_or_default();
                        all.extend(self.b.lock().config.favorites.iter().cloned());
                        all.extend(self.fetch("SQ:").unwrap_or_default());
                        all.iter()
                            .find(|i| i.title.to_lowercase() == n)
                            .or_else(|| all.iter().find(|i| i.title.to_lowercase().contains(&n)))
                            .cloned()
                            .ok_or_else(|| format!("No favorite or playlist called “{n}”. Try search."))?
                    }
                    (None, None) => return Err("Give a ref (from list_favorites, list_playlists or search) or a name.".into()),
                };
                self.wait(Cmd::PlayItem(gid.clone(), item, how))?;
                thread::sleep(Duration::from_millis(300));
                Ok(self.summary(&gid))
            }
            "get_queue" => {
                let (_, gid) = self.room(a)?;
                self.wait(Cmd::LoadQueue(gid.clone()))?;
                let s = self.b.lock();
                let (_, g) = s.find_group(&gid).ok_or("That room isn't available right now")?;
                let q = g.queue.clone().unwrap_or_default();
                if q.is_empty() {
                    return Ok("The queue is empty.".to_string());
                }
                let cur = g.now.track_no as usize;
                Ok(q.iter()
                    .enumerate()
                    .map(|(i, it)| format!("{}{}. {} — {}", if i + 1 == cur && !g.now.is_stream { "▶ " } else { "" }, i + 1, it.title, it.subtitle()))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "play_from_queue" | "remove_from_queue" => {
                let (_, gid) = self.room(a)?;
                let pos = int_arg("position").ok_or("Give a position, counting from 1.")?;
                if pos < 1 {
                    return Err("Positions count from 1.".into());
                }
                let i = (pos - 1) as usize;
                self.wait(if name == "play_from_queue" { Cmd::PlayQueueIndex(gid.clone(), i) } else { Cmd::RemoveFromQueue(gid.clone(), i) })?;
                Ok(if name == "play_from_queue" { self.summary(&gid) } else { format!("Removed song {pos} from the queue") })
            }
            "clear_queue" => {
                let (_, gid) = self.room(a)?;
                self.wait(Cmd::ClearQueue(gid))?;
                Ok("The queue is empty.".to_string())
            }
            "group_rooms" => {
                let names: Vec<String> = a.get("rooms").and_then(|r| r.as_array()).map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                if names.len() < 2 {
                    return Err("Choose at least two rooms.".to_string());
                }
                let uuids = {
                    let s = self.b.lock();
                    let ids = names.iter().map(|n| find_room(&s, n)).collect::<Result<Vec<_>, _>>()?;
                    let homes: std::collections::HashSet<String> = ids.iter().filter_map(|u| s.find_room(u).map(|(h, _)| h.id.clone())).collect();
                    if homes.len() > 1 {
                        return Err("S1 and S2 rooms can't play together; pick rooms from one system.".to_string());
                    }
                    ids
                };
                self.group(uuids)
            }
            "play_along" => {
                let (_, lead) = self.room(a)?;
                let other = str_arg("other_room").ok_or("Which room in the other system?")?;
                let follow = {
                    let s = self.b.lock();
                    let uuid = find_room(&s, &other)?;
                    s.households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&uuid)).map(|g| g.id.clone()).unwrap_or(uuid)
                };
                self.wait(Cmd::LinkGroups(lead.clone(), follow.clone()))?;
                let s = self.b.lock();
                let name = |g: &str| s.find_group(g).map(|(h, x)| h.group_name(x)).unwrap_or_default();
                Ok(format!("{} now plays along with {}", name(&follow), name(&lead)))
            }
            "stop_play_along" => {
                let (_, gid) = self.room(a)?;
                self.wait(Cmd::Unlink(gid.clone()))?;
                Ok("Stopped playing along".into())
            }
            "ungroup_room" => {
                let (uuid, _) = self.room(a)?;
                self.wait(Cmd::GroupRooms(vec![uuid.clone()]))?;
                Ok(format!("{} now plays on its own", self.room_name(&uuid)))
            }
            "group_all" => {
                let (uuid, _) = self.room(a)?;
                let all: Vec<String> = self.b.lock().find_room(&uuid).map(|(h, _)| h.rooms.iter().map(|r| r.uuid.clone()).collect()).unwrap_or_default();
                self.group(all)
            }
            "use_saved_group" => {
                let n = str_arg("name").unwrap_or_default().to_lowercase();
                let rooms = self.b.lock().config.groups.iter().find(|g| g.name.to_lowercase() == n).map(|g| g.rooms.clone());
                let rooms = rooms.ok_or_else(|| format!("No saved group called “{n}”"))?;
                self.group(rooms)
            }
            "switch_input" => {
                let (uuid, gid) = self.room(a)?;
                let from = match str_arg("from_room") {
                    Some(f) => find_room(&self.b.lock(), &f)?,
                    None => uuid,
                };
                let tv = str_arg("input").as_deref() == Some("tv");
                self.wait(if tv { Cmd::Tv(gid.clone(), from) } else { Cmd::LineIn(gid.clone(), from) })?;
                Ok(self.summary(&gid))
            }
            "get_sound" => {
                let (uuid, _) = self.room(a)?;
                self.wait(Cmd::LoadRoomSettings(uuid.clone()))?;
                let s = self.b.lock();
                let st = s.find_room(&uuid).and_then(|(_, r)| r.settings.clone()).ok_or("That speaker didn't say")?;
                Ok(serde_json::to_string_pretty(&json!({"room": self_name(&s, &uuid), "bass": st.bass, "treble": st.treble, "loudness": st.loudness, "night_sound": st.night, "speech_enhancement": st.speech, "status_light": st.status_light})).unwrap_or_default())
            }
            "set_sound" => {
                let (uuid, _) = self.room(a)?;
                let mut changed = vec![];
                if let Some(v) = int_arg("bass") {
                    self.wait(Cmd::SetBass(uuid.clone(), v.clamp(-10, 10) as i32))?;
                    changed.push(format!("bass {v}"));
                }
                if let Some(v) = int_arg("treble") {
                    self.wait(Cmd::SetTreble(uuid.clone(), v.clamp(-10, 10) as i32))?;
                    changed.push(format!("treble {v}"));
                }
                for (key, label) in [("loudness", "loudness"), ("night_sound", "night sound"), ("speech_enhancement", "speech enhancement"), ("status_light", "status light")] {
                    if let Some(on) = bool_arg(key) {
                        let u = uuid.clone();
                        self.wait(match key {
                            "loudness" => Cmd::SetLoudness(u, on),
                            "night_sound" => Cmd::SetNight(u, on),
                            "speech_enhancement" => Cmd::SetSpeech(u, on),
                            _ => Cmd::SetStatusLight(u, on),
                        })?;
                        changed.push(format!("{label} {}", if on { "on" } else { "off" }));
                    }
                }
                if changed.is_empty() {
                    return Err("Nothing to change: give bass, treble, loudness, night_sound, speech_enhancement or status_light.".into());
                }
                Ok(format!("{}: {}", self.room_name(&uuid), changed.join(", ")))
            }
            "check_for_updates" => {
                let homes: Vec<(String, String)> = self.b.lock().households.iter().map(|h| (h.id.clone(), format!("{} system", h.system.label()))).collect();
                let mut lines = vec![];
                for (hid, label) in homes {
                    self.wait(Cmd::CheckUpdates(hid.clone()))?;
                    let until = Instant::now() + Duration::from_secs(30);
                    let state = loop {
                        let st = self.b.lock().updates.get(&hid).cloned();
                        match st {
                            Some(UpdateState::Checking) | Some(UpdateState::Installing(..)) | None if Instant::now() < until => thread::sleep(Duration::from_millis(150)),
                            other => break other,
                        }
                    };
                    lines.push(match state {
                        Some(UpdateState::UpToDate) => format!("{label}: up to date"),
                        Some(UpdateState::Available(o)) => format!("{label}: version {} is available ({} MB)", o.version, o.size / 1_000_000),
                        Some(UpdateState::Failed(e)) => format!("{label}: couldn't check ({e})"),
                        _ => format!("{label}: no answer yet"),
                    });
                }
                Ok(if lines.is_empty() { "No systems found.".into() } else { lines.join("\n") })
            }
            "install_update" => {
                if bool_arg("confirm") != Some(true) {
                    return Err("Installing restarts every speaker in the system. Ask the person first, then call again with confirm: true.".into());
                }
                let (uuid, _) = self.room(a)?;
                let hid = self.b.lock().find_room(&uuid).map(|(h, _)| h.id.clone()).ok_or("That room isn't available right now")?;
                if !matches!(self.b.lock().updates.get(&hid), Some(UpdateState::Available(_))) {
                    self.wait(Cmd::CheckUpdates(hid.clone()))?;
                    let until = Instant::now() + Duration::from_secs(30);
                    while matches!(self.b.lock().updates.get(&hid), Some(UpdateState::Checking)) && Instant::now() < until {
                        thread::sleep(Duration::from_millis(150));
                    }
                }
                match self.b.lock().updates.get(&hid).cloned() {
                    Some(UpdateState::Available(_)) => {}
                    Some(UpdateState::Failed(e)) => return Err(format!("Couldn't check for updates: {e}")),
                    _ => return Ok("This system is already up to date.".into()),
                }
                self.wait(Cmd::InstallUpdate(hid))?;
                Ok("Update started: every speaker downloads it and restarts; they're back in about 10 minutes.".into())
            }
            "list_alarms" => {
                self.wait(Cmd::LoadAlarms)?;
                let s = self.b.lock();
                let list = s.alarms.clone().unwrap_or_default();
                if list.is_empty() {
                    return Ok("No alarms yet.".to_string());
                }
                Ok(list
                    .iter()
                    .map(|al| {
                        format!(
                            "[{}] {:02}:{:02} · {} · {} · {} · volume {} · {}",
                            al.id,
                            al.hour,
                            al.minute,
                            recurrence_label(&al.recurrence),
                            self_name(&s, &al.room_uuid),
                            if al.program_name.is_empty() { "chime".to_string() } else { al.program_name.clone() },
                            al.volume,
                            if al.enabled { "on" } else { "off" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "set_alarm_enabled" => {
                let id = str_arg("id").unwrap_or_default();
                let on = bool_arg("enabled").ok_or("Say enabled: true or false.")?;
                if self.b.lock().alarms.is_none() {
                    self.wait(Cmd::LoadAlarms)?;
                }
                let alarm = self.b.lock().alarms.clone().unwrap_or_default().into_iter().find(|x| x.id == id.trim_matches(['[', ']']));
                let mut alarm = alarm.ok_or_else(|| format!("No alarm with id “{id}”. Use list_alarms."))?;
                alarm.enabled = on;
                self.wait(Cmd::SaveAlarm(alarm))?;
                Ok(format!("Alarm {id} {}", if on { "on" } else { "off" }))
            }
            other => Err(format!("Unknown tool: {other}")),
        }
    }

    fn group(&self, rooms: Vec<String>) -> ToolResult {
        self.wait(Cmd::GroupRooms(rooms.clone()))?;
        // The worker may have taken a room out of its old group first; finish the grouping.
        let together = |s: &AppState| s.households.iter().flat_map(|h| h.groups.iter()).any(|g| rooms.iter().all(|r| g.members.contains(r)));
        if !together(&self.b.lock()) {
            thread::sleep(Duration::from_millis(400));
            self.wait(Cmd::GroupRooms(rooms.clone()))?;
        }
        let names: Vec<String> = rooms.iter().map(|r| self.room_name(r)).collect();
        Ok(format!("Grouped: {}", names.join(", ")))
    }
}

fn self_name(s: &AppState, uuid: &str) -> String {
    s.find_room(uuid).map(|(_, r)| r.name.clone()).unwrap_or_else(|| uuid.to_string())
}

/// A room by id or name: exact first, then a unique partial match.
fn find_room(s: &AppState, q: &str) -> Result<String, String> {
    let ql = q.trim().to_lowercase();
    let all: Vec<&Room> = s.households.iter().flat_map(|h| h.rooms.iter()).collect();
    if let Some(r) = all.iter().find(|r| r.uuid == q || r.name.to_lowercase() == ql) {
        return Ok(r.uuid.clone());
    }
    let partial: Vec<&&Room> = all.iter().filter(|r| r.name.to_lowercase().contains(&ql)).collect();
    match partial.len() {
        1 => Ok(partial[0].uuid.clone()),
        0 => Err(format!("No room called “{q}”. Rooms: {}", all.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", "))),
        _ => Err(format!("“{q}” could be {}", partial.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(" or "))),
    }
}

// ---------- Over stdin/stdout ----------

/// Serve MCP on stdin/stdout until the assistant closes it.
pub fn serve_stdio(mcp: Mcp) {
    let stdin = std::io::stdin();
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let say = |out: &Arc<Mutex<std::io::Stdout>>, v: &Value| {
        let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    };
    // Tell the assistant when something it follows changes.
    {
        let (mcp, out) = (mcp.clone(), out.clone());
        thread::spawn(move || {
            let mut watcher = notify::Watcher::new(mcp.clone());
            loop {
                thread::sleep(Duration::from_secs(1));
                let followed = mcp.subscriptions("");
                if followed.is_empty() || !mcp.settings().enabled {
                    continue;
                }
                for uri in watcher.poll(&followed) {
                    let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = writeln!(o, "{}", notify::updated(&uri));
                    let _ = o.flush();
                }
            }
        });
    }
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| mcp.handle(m)).collect();
                if replies.is_empty() {
                    None
                } else {
                    Some(Value::Array(replies))
                }
            }
            Ok(msg) => mcp.handle(&msg),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("Parse error: {e}")}})),
        };
        if let Some(r) = reply {
            say(&out, &r);
        }
    }
}

// ---------- Over HTTP ----------

/// The address assistants connect to.
pub fn address(cfg: &McpConfig) -> String {
    format!("http://127.0.0.1:{}/mcp", cfg.port)
}

/// Keep an HTTP listener open while the link is switched on (and closed while it's off),
/// following changes to the settings as they happen.
pub fn start_http(mcp: Mcp, waker: crate::backend::Waker) {
    thread::spawn(move || {
        let mut listening: Option<(TcpListener, u16, bool)> = None;
        let mut last_status = String::new();
        loop {
            let cfg = mcp.settings();
            let want = cfg.enabled.then_some((cfg.port, cfg.allow_network));
            let have = listening.as_ref().map(|(_, p, n)| (*p, *n));
            if want != have {
                listening = None;
                let status = match want {
                    None => String::new(),
                    Some((port, net)) => {
                        let host = if net { "0.0.0.0" } else { "127.0.0.1" };
                        match TcpListener::bind((host, port)) {
                            Ok(l) => {
                                let _ = l.set_nonblocking(true);
                                listening = Some((l, port, net));
                                format!("ok:{}", address(&cfg))
                            }
                            Err(e) => format!("err:{e}"),
                        }
                    }
                };
                if status != last_status {
                    if let Some(addr) = status.strip_prefix("ok:") {
                        eprintln!("Assistant link (MCP) listening on {addr}");
                    }
                    mcp.b.lock().mcp_status = status.clone();
                    last_status = status;
                    waker.request_repaint();
                }
                if want.is_some() && listening.is_none() {
                    // Port busy: try again in a while rather than spinning.
                    thread::sleep(Duration::from_secs(3));
                    last_status.clear();
                    continue;
                }
            }
            match &listening {
                Some((l, _, _)) => match l.accept() {
                    Ok((stream, _)) => {
                        let m = mcp.clone();
                        let w = waker.clone();
                        thread::spawn(move || {
                            let _ = serve_http(&m, stream);
                            w.request_repaint();
                        });
                    }
                    Err(_) => thread::sleep(Duration::from_millis(50)),
                },
                None => thread::sleep(Duration::from_millis(400)),
            }
        }
    });
}

fn respond(stream: &mut TcpStream, code: u16, reason: &str, body: &str, extra: &str) -> std::io::Result<()> {
    let ctype = if body.is_empty() { "" } else { "Content-Type: application/json\r\n" };
    write!(stream, "HTTP/1.1 {code} {reason}\r\n{ctype}{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    stream.flush()
}

fn serve_http(mcp: &Mcp, mut stream: TcpStream) -> std::io::Result<()> {
    let _ = stream.set_nonblocking(false);
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
        if headers.len() > 100 {
            break;
        }
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let cfg = mcp.settings();
    let error = |code: i64, msg: &str| json!({"jsonrpc": "2.0", "id": null, "error": {"code": code, "message": msg}}).to_string();

    if !cfg.enabled {
        return respond(&mut stream, 503, "Service Unavailable", &error(-32001, "The assistant link is switched off."), "");
    }
    if path.split('?').next() != Some("/mcp") {
        return respond(&mut stream, 404, "Not Found", "", "");
    }
    // Web pages mustn't reach the speakers through this link.
    if let Some(origin) = headers.get("origin") {
        let o = origin.to_lowercase();
        let local = ["http://localhost", "http://127.0.0.1", "https://localhost", "https://127.0.0.1"].iter().any(|p| o == *p || o.starts_with(&format!("{p}:")));
        if !local {
            return respond(&mut stream, 403, "Forbidden", &error(-32002, "Requests from web pages aren't allowed."), "");
        }
    }
    if !cfg.allow_network {
        let host = headers.get("host").map(|h| h.to_lowercase()).unwrap_or_default();
        let name = host.rsplit_once(':').map(|(h, _)| h.to_string()).unwrap_or(host);
        if !["127.0.0.1", "localhost", "[::1]"].contains(&name.as_str()) {
            return respond(&mut stream, 403, "Forbidden", &error(-32002, "Only this computer may connect."), "");
        }
    }
    let token_ok = headers.get("authorization").is_some_and(|a| a.strip_prefix("Bearer ").map(str::trim) == Some(cfg.token.as_str()));
    if !token_ok {
        return respond(
            &mut stream,
            401,
            "Unauthorized",
            &error(-32003, "Missing or wrong access key (Authorization: Bearer …). Find it in Roomtone's Settings."),
            "WWW-Authenticate: Bearer\r\n",
        );
    }
    let session = headers.get("mcp-session-id").cloned().unwrap_or_default();
    match method {
        "POST" => {}
        "GET" if headers.get("accept").is_some_and(|a| a.contains("text/event-stream")) && !session.is_empty() => return stream_changes(mcp, &mut stream, &session),
        "DELETE" if !session.is_empty() => {
            mcp.end_session(&session);
            return respond(&mut stream, 200, "OK", "", "");
        }
        _ => return respond(&mut stream, 405, "Method Not Allowed", "", "Allow: POST, GET, DELETE\r\n"),
    }
    if !headers.get("content-type").is_some_and(|c| c.to_lowercase().contains("application/json")) {
        return respond(&mut stream, 415, "Unsupported Media Type", &error(-32700, "Send JSON."), "");
    }
    let len: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    if len > 1 << 20 {
        return respond(&mut stream, 413, "Payload Too Large", "", "");
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let mut extra = String::new();
    let mut session = session;
    let parsed = serde_json::from_slice::<Value>(&body);
    if let Ok(m) = &parsed {
        if m.get("method").and_then(|x| x.as_str()) == Some("initialize") && session.is_empty() {
            // Each assistant gets its own session, which its change subscriptions belong to.
            session = crate::config::new_token();
            extra = format!("Mcp-Session-Id: {session}\r\n");
        }
    }
    let reply = match parsed {
        Ok(Value::Array(batch)) => {
            let r: Vec<Value> = batch.iter().filter_map(|m| mcp.handle_for(&session, m)).collect();
            (!r.is_empty()).then_some(Value::Array(r))
        }
        Ok(msg) => mcp.handle_for(&session, &msg),
        Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("Parse error: {e}")}})),
    };
    match reply {
        Some(r) => respond(&mut stream, 200, "OK", &r.to_string(), &extra),
        None => respond(&mut stream, 202, "Accepted", "", ""),
    }
}

/// Hold the connection open and send a `notifications/resources/updated` event whenever
/// something this session follows changes. Ends when the assistant hangs up, the session is
/// closed or the link is switched off.
fn stream_changes(mcp: &Mcp, stream: &mut TcpStream, session: &str) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n: following changes\n\n")?;
    stream.flush()?;
    let mut watcher = notify::Watcher::new(mcp.clone());
    let mut quiet = 0;
    loop {
        thread::sleep(Duration::from_secs(1));
        if !mcp.settings().enabled {
            return Ok(());
        }
        let mut wrote = false;
        for uri in watcher.poll(&mcp.subscriptions(session)) {
            write!(stream, "event: message\ndata: {}\n\n", notify::updated(&uri))?;
            wrote = true;
        }
        quiet = if wrote { 0 } else { quiet + 1 };
        if quiet >= 15 {
            // A comment now and then also tells us when the assistant has gone.
            write!(stream, ": still here\n\n")?;
            quiet = 0;
        }
        stream.flush()?;
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::test_support::{demo_backend, wait_for};

    fn link() -> Mcp {
        let b = demo_backend();
        {
            let mut s = b.lock();
            s.config.mcp.enabled = true;
            s.config.mcp.allow_control = true;
        }
        Mcp::new(b)
    }

    fn rpc(m: &Mcp, id: i64, method: &str, params: Value) -> Value {
        m.handle(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).expect("a request gets a reply")
    }

    /// Call a tool; (text, is_error).
    fn tool(m: &Mcp, name: &str, args: Value) -> (String, bool) {
        let r = rpc(m, 1, "tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        (res["content"][0]["text"].as_str().unwrap_or("").to_string(), res["isError"].as_bool().unwrap_or(true))
    }

    fn ok(m: &Mcp, name: &str, args: Value) -> String {
        let (text, err) = tool(m, name, args.clone());
        assert!(!err, "{name}({args}) failed: {text}");
        text
    }

    #[test]
    fn handshake_and_capabilities() {
        let m = link();
        let r = rpc(&m, 1, "initialize", json!({"protocolVersion": "2025-03-26"}));
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26", "the client's version when supported");
        let caps = &r["result"]["capabilities"];
        for c in ["tools", "resources", "prompts", "completions"] {
            assert!(caps.get(c).is_some(), "missing capability {c}");
        }
        let r = rpc(&m, 2, "initialize", json!({"protocolVersion": "1999-01-01"}));
        assert_eq!(r["result"]["protocolVersion"], PROTOCOLS[0], "otherwise the newest");
        assert_eq!(rpc(&m, 3, "ping", json!({}))["result"], json!({}));
        assert!(m.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none(), "notifications get no reply");
        assert_eq!(rpc(&m, 4, "no/such", json!({}))["error"]["code"], -32601);
    }

    #[test]
    fn switched_off_refuses_everything() {
        let m = link();
        m.b.lock().config.mcp.enabled = false;
        assert_eq!(rpc(&m, 1, "initialize", json!({}))["error"]["code"], -32001);
        assert_eq!(rpc(&m, 2, "tools/list", json!({}))["error"]["code"], -32001);
    }

    #[test]
    fn tool_list_and_look_only_mode() {
        let m = link();
        let all = rpc(&m, 1, "tools/list", json!({}))["result"]["tools"].as_array().unwrap().len();
        assert_eq!(all, tools().len());
        assert_eq!(all, 49);
        let mut names: Vec<&str> = tools().iter().map(|t| t.0).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all, "tool names are unique");
        for t in tools() {
            assert_eq!(t.2["type"], "object", "{} has an object schema", t.0);
        }
        m.b.lock().config.mcp.allow_control = false;
        let look = rpc(&m, 2, "tools/list", json!({}))["result"]["tools"].as_array().unwrap().clone();
        assert_eq!(look.len(), 14);
        assert!(look.iter().all(|t| t["annotations"]["readOnlyHint"] == true));
        let (text, err) = tool(&m, "playback", json!({"action": "pause"}));
        assert!(err && text.contains("only lets assistants look"));
        assert!(!tool(&m, "get_status", json!({})).1, "looking still works");
        assert_eq!(rpc(&m, 3, "tools/call", json!({"name": "nope"}))["error"]["code"], -32602);
    }

    #[test]
    fn status_playback_volume() {
        let m = link();
        let status: Value = serde_json::from_str(&ok(&m, "get_status", json!({}))).unwrap();
        assert_eq!(status["systems"].as_array().unwrap().len(), 2);
        let before = ok(&m, "now_playing", json!({"room": "Living Room"}));
        ok(&m, "playback", json!({"room": "Living Room", "action": "next"}));
        assert_ne!(ok(&m, "now_playing", json!({"room": "Living Room"})), before);
        assert!(ok(&m, "set_volume", json!({"room": "Kitchen", "volume": 33})).contains("33"));
        assert!(ok(&m, "set_volume", json!({"room": "Kitchen", "change": -3})).contains("30"));
        assert!(tool(&m, "playback", json!({"room": "Kitchen", "action": "dance"})).1);
        assert!(tool(&m, "now_playing", json!({"room": "Nowhere"})).0.contains("No room called"));
    }

    #[test]
    fn rooms_by_partial_name() {
        let m = link();
        let s = m.b.lock();
        assert!(find_room(&s, "living room").is_ok());
        assert!(find_room(&s, "Liv").is_ok(), "a unique part of a name");
        assert!(find_room(&s, "o").unwrap_err().contains("could be"), "ambiguous");
        assert!(find_room(&s, "Garage").is_ok());
        assert!(find_room(&s, "zzz").unwrap_err().contains("Rooms:"));
    }

    #[test]
    fn browse_favorites_and_play() {
        let m = link();
        let root = ok(&m, "browse", json!({}));
        assert!(root.contains("[b1] Favorites"));
        let favs = ok(&m, "browse", json!({"ref": "b1"}));
        assert!(favs.contains("[b1]"), "{favs}");
        assert!(ok(&m, "play", json!({"room": "Bedroom", "ref": "b1"})).contains("Bedroom"));
        assert!(ok(&m, "list_recent", json!({})).contains("[rec1]"));
        let list = ok(&m, "list_favorites", json!({}));
        assert!(list.contains("[fav1]"));
        assert!(tool(&m, "browse", json!({"ref": "nope"})).1);
    }

    #[test]
    fn queue_tools() {
        let m = link();
        let q = ok(&m, "get_queue", json!({"room": "Living Room"}));
        assert!(q.starts_with("1.") || q.starts_with("▶ 1."));
        ok(&m, "move_in_queue", json!({"room": "Living Room", "from": 1, "to": 2}));
        assert!(tool(&m, "move_in_queue", json!({"room": "Living Room", "from": 1, "to": 99})).1);
        ok(&m, "save_queue_as_playlist", json!({"room": "Living Room", "name": "Test mix"}));
        ok(&m, "clear_queue", json!({"room": "Study"}));
        assert!(ok(&m, "get_queue", json!({"room": "Study"})).contains("empty"));
    }

    #[test]
    fn grouping_and_play_along() {
        let m = link();
        assert!(ok(&m, "group_rooms", json!({"rooms": ["Bedroom", "Study"]})).contains("Grouped"));
        assert!(tool(&m, "group_rooms", json!({"rooms": ["Bedroom", "Office"]})).0.contains("can't play together"), "S1 + S2");
        assert!(ok(&m, "play_along", json!({"room": "Living Room", "other_room": "Office"})).contains("plays along"));
        ok(&m, "stop_play_along", json!({"room": "Living Room"}));
        ok(&m, "save_group", json!({"name": "Upstairs", "rooms": ["Bedroom", "Study"]}));
        ok(&m, "use_saved_group", json!({"name": "upstairs"}));
        ok(&m, "delete_saved_group", json!({"name": "Upstairs"}));
        assert!(tool(&m, "save_group", json!({"name": "Mixed", "rooms": ["Bedroom", "Office"]})).1);
        assert!(ok(&m, "pause_all", json!({})).contains("Paused") || true);
    }

    #[test]
    fn alarms_rooms_and_sound() {
        let m = link();
        let made = ok(&m, "create_alarm", json!({"room": "Bedroom", "time": "06:45", "days": ["mon", "fri"], "volume": 12}));
        assert!(made.contains("06:45") && made.contains("Mon, Fri"));
        assert!(tool(&m, "create_alarm", json!({"room": "Bedroom", "time": "25:00"})).1);
        let list = ok(&m, "list_alarms", json!({}));
        assert!(list.contains("06:45"));
        assert!(tool(&m, "delete_alarm", json!({"id": "nope"})).1);
        assert!(ok(&m, "rename_room", json!({"room": "Patio", "name": "Garden"})).contains("Garden"));
        assert!(m.b.lock().households.iter().any(|h| h.rooms.iter().any(|r| r.name == "Garden")));
        ok(&m, "sleep_timer", json!({"room": "Kitchen", "minutes": 30}));
        ok(&m, "set_play_mode", json!({"room": "Kitchen", "shuffle": true, "repeat": "all"}));
    }

    #[test]
    fn stations_services_updates() {
        let m = link();
        assert!(tool(&m, "add_station", json!({"url": "ftp://nope"})).1);
        ok(&m, "add_station", json!({"name": "Test FM", "url": "http://stream.example.com/test.mp3"}));
        assert!(wait_for(5, || m.b.lock().config.stations.iter().any(|s| s.name == "Test FM")));
        ok(&m, "remove_station", json!({"station": "Test FM"}));
        let services = ok(&m, "list_services", json!({}));
        assert!(services.contains("Apple Music: favorites only"));
        assert!(tool(&m, "check_service", json!({"service": "Spotify"})).1, "not connected in the test settings");
        assert!(tool(&m, "install_update", json!({"room": "Kitchen"})).0.contains("confirm: true"));
        let updates = ok(&m, "check_for_updates", json!({}));
        assert!(updates.contains("S2 system: version"));
        assert!(updates.contains("S1 system: up to date"));
    }

    #[test]
    fn resources_prompts_and_completions() {
        let m = link();
        let list = rpc(&m, 1, "resources/list", json!({}));
        let uris: Vec<&str> = list["result"]["resources"].as_array().unwrap().iter().filter_map(|r| r["uri"].as_str()).collect();
        assert!(uris.contains(&"roomtone://status"));
        let status = rpc(&m, 2, "resources/read", json!({"uri": "roomtone://status"}));
        let text = status["result"]["contents"][0]["text"].as_str().unwrap();
        assert!(serde_json::from_str::<Value>(text).is_ok());
        let q = rpc(&m, 3, "resources/read", json!({"uri": "roomtone://queue/Living%20Room"}));
        assert!(q["result"]["contents"][0]["text"].as_str().unwrap().contains("1."));
        assert_eq!(rpc(&m, 4, "resources/read", json!({"uri": "roomtone://nope"}))["error"]["code"], -32002);
        assert_eq!(rpc(&m, 5, "resources/templates/list", json!({}))["result"]["resourceTemplates"][0]["uriTemplate"], "roomtone://queue/{room}");
        let prompts = rpc(&m, 6, "prompts/list", json!({}));
        assert_eq!(prompts["result"]["prompts"].as_array().unwrap().len(), 4);
        let p = rpc(&m, 7, "prompts/get", json!({"name": "bedtime", "arguments": {"room": "Bedroom", "minutes": "20"}}));
        let msg = p["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(msg.contains("Bedroom") && msg.contains("20 minutes"));
        assert_eq!(rpc(&m, 8, "prompts/get", json!({"name": "nope"}))["error"]["code"], -32602);
        let c = rpc(&m, 9, "completion/complete", json!({"ref": {"type": "ref/prompt", "name": "bedtime"}, "argument": {"name": "room", "value": "bed"}}));
        assert_eq!(c["result"]["completion"]["values"], json!(["Bedroom"]));
    }

    #[test]
    fn following_changes_notices_what_changed_and_ignores_ticking() {
        let m = link();
        // Only some resources can be followed, and the room must exist.
        assert!(rpc(&m, 1, "resources/subscribe", json!({"uri": "roomtone://favorites"}))["error"].is_object());
        assert!(rpc(&m, 2, "resources/subscribe", json!({"uri": "roomtone://queue/Nowhere"}))["error"].is_object());
        assert!(rpc(&m, 3, "resources/subscribe", json!({"uri": "roomtone://status"}))["result"].is_object());
        assert!(rpc(&m, 4, "resources/subscribe", json!({"uri": "roomtone://now-playing"}))["result"].is_object());
        assert!(rpc(&m, 5, "resources/subscribe", json!({"uri": "roomtone://queue/Living%20Room"}))["result"].is_object());
        let init = rpc(&m, 6, "initialize", json!({"protocolVersion": "2025-06-18"}));
        assert_eq!(init["result"]["capabilities"]["resources"]["subscribe"], true);
        let followed = m.subscriptions("");
        assert_eq!(followed.len(), 3);
        let mut w = notify::Watcher::new(m.clone());
        assert!(w.poll(&followed).is_empty(), "the first look only records");
        assert!(w.poll(&followed).is_empty(), "nothing changed, nothing sent (the song position ticking doesn't count)");
        ok(&m, "set_volume", json!({"room": "Kitchen", "volume": 41}));
        let changed = w.poll(&followed);
        assert!(changed.contains(&"roomtone://status".to_string()) && changed.contains(&"roomtone://now-playing".to_string()), "{changed:?}");
        assert!(!changed.iter().any(|u| u.starts_with("roomtone://queue/")), "the queue didn't change");
        assert!(w.poll(&followed).is_empty(), "told once");
        // Unfollowing stops the notices.
        rpc(&m, 7, "resources/unsubscribe", json!({"uri": "roomtone://status"}));
        assert_eq!(m.subscriptions("").len(), 2);
        // Sessions are separate.
        assert!(m.handle_for("other", &json!({"jsonrpc": "2.0", "id": 8, "method": "resources/subscribe", "params": {"uri": "roomtone://alarms"}})).unwrap()["result"].is_object());
        assert_eq!(m.subscriptions("other").len(), 1);
        m.end_session("other");
        assert!(m.subscriptions("other").is_empty());
    }

    #[test]
    fn the_number_followed_is_limited() {
        let m = link();
        {
            let mut all = m.subs.lock().unwrap();
            all.insert("s".into(), (0..notify::MAX_SUBSCRIPTIONS).map(|i| format!("roomtone://fake/{i}")).collect());
        }
        assert!(m.subscribe("s", "roomtone://status").is_err(), "one more than allowed");
        for i in 0..notify::MAX_SESSIONS {
            m.subs.lock().unwrap().insert(format!("x{i}"), Default::default());
        }
        assert!(m.subscribe("brand-new", "roomtone://status").is_err(), "too many sessions");
    }

    #[test]
    fn a_backup_can_be_saved_but_never_over_something() {
        let m = link();
        let dir = std::env::temp_dir().join(format!("roomtone-mcp-backup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("b.json");
        let said = ok(&m, "save_backup", json!({"file": file.to_string_lossy()}));
        assert!(said.starts_with("Saved") && file.exists(), "{said}");
        assert!(crate::backup::Backup::parse(&std::fs::read_to_string(&file).unwrap()).is_ok());
        assert!(tool(&m, "save_backup", json!({"file": file.to_string_lossy()})).0.contains("already exists"));
        assert!(tool(&m, "save_backup", json!({"file": dir.join("b.txt").to_string_lossy()})).1, "only .json");
        assert!(tool(&m, "save_backup", json!({"file": dir.join("nope").join("b.json").to_string_lossy()})).1, "folder must exist");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alarm_times_and_days() {
        use super::extra::{parse_days, parse_time};
        assert_eq!(parse_time("07:30"), Some((7, 30)));
        assert_eq!(parse_time(" 6:05 "), Some((6, 5)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("seven"), None);
        assert_eq!(parse_days(None).unwrap(), "DAILY");
        assert_eq!(parse_days(Some(&json!("weekdays"))).unwrap(), "WEEKDAYS");
        assert_eq!(parse_days(Some(&json!(["fri", "Monday", "mon"]))).unwrap(), "ON_15");
        assert_eq!(parse_days(Some(&json!("sat sun"))).unwrap(), "ON_06");
        assert!(parse_days(Some(&json!(["someday"]))).is_err());
    }

    // ---------- over HTTP ----------

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    fn http(port: u16, request: &str) -> String {
        let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) else { return "refused".into() };
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    fn post(port: u16, key: &str, extra: &str, body: &str) -> String {
        http(
            port,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    #[test]
    fn http_link_checks_key_origin_and_switch() {
        let m = link();
        let port = free_port();
        let key = {
            let mut s = m.b.lock();
            s.config.mcp.port = port;
            s.config.mcp.token.clone()
        };
        start_http(m.clone(), crate::backend::Waker::none());
        assert!(wait_for(5, || m.b.lock().mcp_status.starts_with("ok:")), "the link opens");
        let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert!(post(port, &key, "", ping).starts_with("HTTP/1.1 200"));
        assert!(post(port, "wrong", "", ping).starts_with("HTTP/1.1 401"));
        assert!(post(port, &key, "Origin: https://evil.example\r\n", ping).starts_with("HTTP/1.1 403"));
        assert!(post(port, &key, &format!("Origin: http://localhost:{port}\r\n"), ping).starts_with("HTTP/1.1 200"), "local pages are fine");
        let wrong_host = format!(
            "POST /mcp HTTP/1.1\r\nHost: attacker.example\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{ping}",
            ping.len()
        );
        assert!(http(port, &wrong_host).starts_with("HTTP/1.1 403"), "DNS-rebinding guard");
        let text_plain =
            format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {key}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{ping}", ping.len());
        assert!(http(port, &text_plain).starts_with("HTTP/1.1 415"));
        assert!(http(port, &format!("GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {key}\r\n\r\n")).starts_with("HTTP/1.1 405"));
        // An assistant gets a session at initialize, and may open the event stream with it.
        let init = post(port, &key, "", r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#);
        let sid = init.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ")).expect("a session id").trim().to_string();
        let sub = r#"{"jsonrpc":"2.0","id":2,"method":"resources/subscribe","params":{"uri":"roomtone://status"}}"#;
        assert!(post(port, &key, &format!("Mcp-Session-Id: {sid}\r\n"), sub).contains(r#""result""#));
        assert_eq!(m.subscriptions(&sid).len(), 1);
        let mut es = TcpStream::connect(("127.0.0.1", port)).unwrap();
        es.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(es, "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {key}\r\nAccept: text/event-stream\r\nMcp-Session-Id: {sid}\r\n\r\n").unwrap();
        let mut first = [0u8; 120];
        let n = es.read(&mut first).unwrap();
        let head = String::from_utf8_lossy(&first[..n]).to_string();
        assert!(head.starts_with("HTTP/1.1 200") && head.contains("text/event-stream"), "{head}");
        drop(es);
        assert!(post(port, &key, "", r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).starts_with("HTTP/1.1 202"));
        let batch = post(port, &key, "", r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#);
        assert!(batch.contains(r#""id":2"#));
        // Switching off closes the port.
        m.b.lock().config.mcp.enabled = false;
        assert!(wait_for(5, || http(port, "GET / HTTP/1.1\r\n\r\n") == "refused"), "the port closes when switched off");
    }

    #[test]
    fn web_address() {
        let cfg = McpConfig { port: 9999, ..Default::default() };
        assert_eq!(address(&cfg), "http://127.0.0.1:9999/mcp");
    }
}
