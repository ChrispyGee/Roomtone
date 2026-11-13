//! More of the assistant link: browsing, favorites, queue editing, rooms, alarms, saved groups,
//! services, stations and podcasts, plus MCP resources (live status), prompts and completions.

use super::*;
use crate::config::{Podcast, SavedGroup, Station};

/// The extra tools: (name, what it does, input schema, changes things?).
pub(super) fn tools() -> Vec<(&'static str, &'static str, Value, bool)> {
    let room = json!({"type": "string", "description": "Room name. Leave out to use the room selected in the app."});
    let reff = json!({"type": "string", "description": "A ref from browse, search, list_favorites, list_playlists or list_recent."});
    vec![
        (
            "browse",
            "Open the music sources step by step. Without a ref, lists the top level (favorites, playlists, the system's library, this computer's music, radio, podcasts, Internet Archive, connected services). Pass a ref from a previous browse to open it; play a ref with play.",
            json!({"type": "object", "properties": {"ref": reff, "room": room}}),
            false,
        ),
        ("list_recent", "What was played recently from this app, with refs to play again.", json!({"type": "object", "properties": {}}), false),
        (
            "add_favorite",
            "Save something to Favorites: a ref, or what's playing in a room now (current: true).",
            json!({"type": "object", "properties": {"ref": reff, "current": {"type": "boolean"}, "room": room}}),
            true,
        ),
        ("remove_favorite", "Remove a favorite, by its ref from list_favorites.", json!({"type": "object", "properties": {"ref": reff}, "required": ["ref"]}), true),
        ("save_queue_as_playlist", "Save a room's queue as a playlist on the system.", json!({"type": "object", "properties": {"room": room, "name": {"type": "string"}}, "required": ["name"]}), true),
        (
            "move_in_queue",
            "Move a song in a room's queue from one position to another (both counted from 1).",
            json!({"type": "object", "properties": {"room": room, "from": {"type": "integer", "minimum": 1}, "to": {"type": "integer", "minimum": 1}}, "required": ["from", "to"]}),
            true,
        ),
        ("pause_all", "Pause every room that's playing, in every system (or only the system of a given room).", json!({"type": "object", "properties": {"room": room}}), true),
        ("rename_room", "Give a room a new name.", json!({"type": "object", "properties": {"room": room, "name": {"type": "string"}}, "required": ["room", "name"]}), true),
        (
            "create_alarm",
            "Make a new alarm. time is \"HH:MM\" (24-hour). days: daily, weekdays, weekends, once, or a list such as [\"mon\", \"wed\"]. Plays the chime unless a ref (a favorite or station) is given.",
            json!({"type": "object", "properties": {
                "room": room, "time": {"type": "string"},
                "days": {"description": "daily, weekdays, weekends, once, or a list of day names"},
                "volume": {"type": "integer", "minimum": 0, "maximum": 100, "default": 20},
                "minutes": {"type": "integer", "minimum": 1, "maximum": 180, "default": 60},
                "ref": reff
            }, "required": ["room", "time"]}),
            true,
        ),
        ("delete_alarm", "Delete an alarm by its id from list_alarms.", json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}), true),
        (
            "save_group",
            "Save a named group of rooms (from one system) to use later with use_saved_group. Replaces a saved group of the same name.",
            json!({"type": "object", "properties": {"name": {"type": "string"}, "rooms": {"type": "array", "items": {"type": "string"}, "minItems": 2}}, "required": ["name", "rooms"]}),
            true,
        ),
        ("delete_saved_group", "Delete a saved group by name.", json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}), true),
        ("list_services", "The streaming services and free sources, and whether each is connected, ready to connect, favorites only or switched on.", json!({"type": "object", "properties": {}}), false),
        (
            "check_service",
            "Check a connected streaming service end to end without playing anything: browsing, search and which of the system's accounts plays it.",
            json!({"type": "object", "properties": {"service": {"type": "string", "description": "e.g. Spotify"}}, "required": ["service"]}),
            false,
        ),
        ("find_speakers", "Search the network for speakers again (after adding or moving one).", json!({"type": "object", "properties": {}}), false),
        (
            "add_station",
            "Add an internet radio station to My stations by its stream address (.pls and .m3u links are followed).",
            json!({"type": "object", "properties": {"name": {"type": "string"}, "url": {"type": "string"}, "genre": {"type": "string"}}, "required": ["url"]}),
            true,
        ),
        ("remove_station", "Remove a station from My stations, by name or stream address.", json!({"type": "object", "properties": {"station": {"type": "string"}}, "required": ["station"]}), true),
        ("subscribe_podcast", "Subscribe to a podcast show, by its ref from search.", json!({"type": "object", "properties": {"ref": reff}, "required": ["ref"]}), true),
        ("unsubscribe_podcast", "Unsubscribe from a podcast show, by name.", json!({"type": "object", "properties": {"show": {"type": "string"}}, "required": ["show"]}), true),
    ]
}

/// "07:30" → (7, 30).
pub(crate) fn parse_time(t: &str) -> Option<(u32, u32)> {
    let (h, m) = t.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().get(..2).unwrap_or(m.trim()).parse().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

/// The speakers' repeat code for an alarm: DAILY, WEEKDAYS, WEEKENDS, ONCE or ON_<days, Sunday = 0>.
pub(crate) fn parse_days(v: Option<&Value>) -> Result<String, String> {
    let names = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    match v {
        None => Ok("DAILY".into()),
        Some(Value::String(s)) => match s.trim().to_lowercase().as_str() {
            "" | "daily" | "every day" | "everyday" => Ok("DAILY".into()),
            "weekdays" => Ok("WEEKDAYS".into()),
            "weekends" => Ok("WEEKENDS".into()),
            "once" => Ok("ONCE".into()),
            other => parse_days(Some(&Value::Array(other.split([',', ' ']).filter(|x| !x.is_empty()).map(|x| Value::String(x.into())).collect()))),
        },
        Some(Value::Array(list)) => {
            let mut digits: Vec<usize> = vec![];
            for d in list {
                let d = d.as_str().unwrap_or("").trim().to_lowercase();
                let i = names.iter().position(|n| d.starts_with(n)).ok_or_else(|| format!("I don't know the day “{d}”"))?;
                if !digits.contains(&i) {
                    digits.push(i);
                }
            }
            if digits.is_empty() {
                return Err("Give at least one day".into());
            }
            digits.sort();
            Ok(format!("ON_{}", digits.iter().map(|d| d.to_string()).collect::<String>()))
        }
        Some(_) => Err("days should be a word such as weekdays, or a list of day names".into()),
    }
}

impl Mcp {
    fn item_by_ref(&self, r: &str) -> Result<Item, String> {
        let key = r.trim().trim_matches(['[', ']']);
        self.refs.lock().unwrap_or_else(|e| e.into_inner()).get(key).cloned().ok_or_else(|| format!("I don't know “{r}”. List or search first."))
    }

    /// The top level of browse: one entry per source.
    fn browse_root(&self) -> Vec<Item> {
        let s = self.b.lock();
        let cat = |id: &str, title: &str, description: &str| Item {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            class: "object.container".into(),
            is_container: true,
            ..Default::default()
        };
        let mut out = vec![
            cat("FV:2", "Favorites", "Saved on the system"),
            cat("SQ:", "Playlists", "Saved on the system"),
            cat("A:ARTIST", "Artists", "The system's music library"),
            cat("A:ALBUM", "Albums", "The system's music library"),
            cat("A:GENRE", "Genres", "The system's music library"),
            cat("S:", "Folders", "Shared folders on the network"),
        ];
        if s.config.service_on("local") {
            out.push(cat("LOCAL:ARTISTS", "This computer: artists", "Music folders on this computer"));
            out.push(cat("LOCAL:ALBUMS", "This computer: albums", "Music folders on this computer"));
        }
        out.push(cat("RADIO:MINE", "My stations", "Radio stations added in the app"));
        if s.config.service_on("radio") {
            out.push(cat("RADIO:TOP", "Popular radio", "The free radio directory"));
        }
        if s.config.service_on("podcasts") {
            out.push(cat("POD:SUBS", "Your podcasts", "Subscribed shows"));
        }
        if s.config.service_on("archive") {
            out.push(cat("IA:FEATURED", "Internet Archive", "Free concerts and recordings"));
        }
        for a in &s.config.accounts {
            out.push(cat(&format!("SVC:{}/", a.sid), &a.name, "Connected streaming service"));
        }
        out
    }

    fn service_status(&self) -> String {
        let s = self.b.lock();
        let live = s.phase == Phase::Live;
        crate::sources::SERVICES
            .iter()
            .map(|svc| {
                let state = if svc.sid == 0 {
                    if s.config.service_on(svc.id) {
                        "switched on"
                    } else {
                        "switched off"
                    }
                } else if s.config.account(svc.sid).is_some() {
                    "connected (browse, search and play)"
                } else if !svc.note.is_empty() {
                    "favorites only (sign-in only works in the speaker maker's app)"
                } else if live && !s.music_services.iter().any(|m| m.sid == svc.sid) {
                    "not offered on this system"
                } else {
                    "ready to connect (sign in from Roomtone's Music Services page)"
                };
                format!("{}: {state}", svc.name)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Run one of the extra tools; `None` if `name` isn't one of them.
    pub(super) fn call_extra(&self, name: &str, a: &Value) -> Option<ToolResult> {
        let str_arg = |k: &str| a.get(k).and_then(|v| v.as_str()).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let bool_arg = |k: &str| a.get(k).and_then(|v| v.as_bool());
        let int_arg = |k: &str| a.get(k).and_then(|v| v.as_f64()).map(|v| v.round() as i64);
        if !name_is_extra(name) {
            return None;
        }
        let result: ToolResult = (|| match name {
            "browse" => match str_arg("ref") {
                None => {
                    let root = self.browse_root();
                    Ok(self.list_text("b", &root, "Nothing to browse yet."))
                }
                Some(r) => {
                    let item = self.item_by_ref(&r)?;
                    if !item.is_container {
                        return Ok(format!("“{}” is something to play, not a folder. Use play with ref {r}.", item.title));
                    }
                    self.use_system_of(a)?;
                    let items = self.fetch(&item.id)?;
                    Ok(format!("{}:\n{}", item.title, self.list_text("b", &items, "Nothing here.")))
                }
            },
            "list_recent" => {
                let recent = self.b.lock().recent.clone();
                Ok(self.list_text("rec", &recent, "Nothing played from this app yet."))
            }
            "add_favorite" => {
                let (gid, item) = if bool_arg("current") == Some(true) || str_arg("ref").is_none() {
                    let (_, gid) = self.room(a)?;
                    let now = self.b.lock().find_group(&gid).and_then(|(_, g)| g.now.as_item());
                    (Some(gid), now.ok_or("Nothing that can be saved is playing there (line-in, TV and the queue itself can't be favorites).")?)
                } else {
                    let gid = self.room(a).ok().map(|(_, g)| g);
                    (gid, self.item_by_ref(&str_arg("ref").unwrap_or_default())?)
                };
                if !item.can_be_favorite() {
                    return Err(format!("“{}” can't be a favorite.", item.title));
                }
                let title = item.title.clone();
                self.wait(Cmd::AddFavorite(gid, item))?;
                Ok(format!("Added “{title}” to Favorites"))
            }
            "remove_favorite" => {
                let item = self.item_by_ref(&str_arg("ref").unwrap_or_default())?;
                let kept = self.b.lock().config.favorites.iter().any(|f| f.id == item.id);
                if !item.is_favorite() && !kept {
                    return Err(format!("“{}” isn't a favorite. Use a ref from list_favorites.", item.title));
                }
                let title = item.title.clone();
                self.wait(Cmd::RemoveFavorite(item.id, item.title))?;
                Ok(format!("Removed “{title}” from Favorites"))
            }
            "save_queue_as_playlist" => {
                let (_, gid) = self.room(a)?;
                let name = str_arg("name").ok_or("Give the playlist a name.")?;
                self.wait(Cmd::SaveQueue(gid, name.clone()))?;
                Ok(format!("Saved the queue as “{name}”"))
            }
            "move_in_queue" => {
                let (_, gid) = self.room(a)?;
                let (from, to) = (int_arg("from").unwrap_or(0), int_arg("to").unwrap_or(0));
                if from < 1 || to < 1 {
                    return Err("Positions count from 1.".into());
                }
                self.wait(Cmd::LoadQueue(gid.clone()))?;
                let len = self.b.lock().find_group(&gid).and_then(|(_, g)| g.queue.as_ref().map(|q| q.len())).unwrap_or(0) as i64;
                if from > len || to > len {
                    return Err(format!("The queue has {len} songs."));
                }
                self.wait(Cmd::MoveInQueue(gid, (from - 1) as usize, (to - 1) as usize))?;
                Ok(format!("Moved song {from} to position {to}"))
            }
            "pause_all" => {
                let only = match str_arg("room") {
                    Some(_) => {
                        let (uuid, _) = self.room(a)?;
                        self.b.lock().find_room(&uuid).map(|(h, _)| h.id.clone())
                    }
                    None => None,
                };
                let playing: Vec<String> = {
                    let s = self.b.lock();
                    s.households
                        .iter()
                        .filter(|h| only.as_ref().is_none_or(|o| &h.id == o))
                        .flat_map(|h| h.groups.iter())
                        .filter(|g| g.now.transport == Transport::Playing)
                        .map(|g| g.id.clone())
                        .collect()
                };
                for g in &playing {
                    self.wait(Cmd::Pause(g.clone()))?;
                }
                Ok(if playing.is_empty() { "Nothing was playing.".into() } else { format!("Paused {} group(s)", playing.len()) })
            }
            "rename_room" => {
                let (uuid, _) = self.room(a)?;
                let name = str_arg("name").ok_or("Give the new name.")?;
                if name.chars().count() > 40 {
                    return Err("Room names can be up to 40 characters.".into());
                }
                let old = self.room_name(&uuid);
                self.wait(Cmd::Rename(uuid, name.clone()))?;
                Ok(format!("Renamed “{old}” to “{name}”"))
            }
            "create_alarm" => {
                let (uuid, _) = self.room(a)?;
                let (hour, minute) = parse_time(&str_arg("time").unwrap_or_default()).ok_or("Give the time as HH:MM, for example 07:30.")?;
                let recurrence = parse_days(a.get("days"))?;
                let mut alarm = Alarm {
                    hour,
                    minute,
                    duration_min: int_arg("minutes").unwrap_or(60).clamp(1, 180) as u32,
                    recurrence,
                    enabled: true,
                    room_uuid: uuid,
                    program_uri: "x-rincon-buzzer:0".into(),
                    program_name: "Chime".into(),
                    play_mode: "NORMAL".into(),
                    volume: int_arg("volume").unwrap_or(20).clamp(0, 100) as u8,
                    ..Default::default()
                };
                if let Some(r) = str_arg("ref") {
                    let item = self.item_by_ref(&r)?;
                    if item.uri.is_empty() {
                        return Err(format!("“{}” can't be played by an alarm; pick a favorite or a station.", item.title));
                    }
                    alarm.program_meta = crate::net::didl::metadata_for(&item);
                    alarm.program_uri = item.uri;
                    alarm.program_name = item.title;
                }
                let summary = format!("{:02}:{:02} · {} · {}", alarm.hour, alarm.minute, recurrence_label(&alarm.recurrence), alarm.program_name);
                self.wait(Cmd::SaveAlarm(alarm))?;
                Ok(format!("New alarm: {summary}"))
            }
            "delete_alarm" => {
                let id = str_arg("id").unwrap_or_default().trim_matches(['[', ']']).to_string();
                if self.b.lock().alarms.is_none() {
                    self.wait(Cmd::LoadAlarms)?;
                }
                if !self.b.lock().alarms.clone().unwrap_or_default().iter().any(|x| x.id == id) {
                    return Err(format!("No alarm with id “{id}”. Use list_alarms."));
                }
                self.wait(Cmd::DeleteAlarm(id.clone()))?;
                Ok(format!("Deleted alarm {id}"))
            }
            "save_group" => {
                let name = str_arg("name").ok_or("Give the group a name.")?;
                let names: Vec<String> = a.get("rooms").and_then(|r| r.as_array()).map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                if names.len() < 2 {
                    return Err("Choose at least two rooms.".into());
                }
                let (household, rooms) = {
                    let s = self.b.lock();
                    let ids = names.iter().map(|n| find_room(&s, n)).collect::<Result<Vec<_>, _>>()?;
                    let homes: std::collections::HashSet<String> = ids.iter().filter_map(|u| s.find_room(u).map(|(h, _)| h.id.clone())).collect();
                    if homes.len() != 1 {
                        return Err("A saved group's rooms must all be in one system.".into());
                    }
                    (homes.into_iter().next().unwrap_or_default(), ids)
                };
                self.wait(Cmd::SaveGroup(None, SavedGroup { name: name.clone(), household, rooms }))?;
                Ok(format!("Saved group “{name}”"))
            }
            "delete_saved_group" => {
                let name = str_arg("name").unwrap_or_default();
                let real = self.b.lock().config.groups.iter().find(|g| g.name.eq_ignore_ascii_case(&name)).map(|g| g.name.clone());
                let real = real.ok_or_else(|| format!("No saved group called “{name}”"))?;
                self.wait(Cmd::DeleteGroup(real.clone()))?;
                Ok(format!("Deleted group “{real}”"))
            }
            "list_services" => Ok(self.service_status()),
            "check_service" => {
                let wanted = str_arg("service").unwrap_or_default().to_lowercase();
                let acc = self.b.lock().config.accounts.iter().find(|x| x.name.to_lowercase().contains(&wanted)).cloned();
                let acc = acc.ok_or_else(|| format!("“{wanted}” isn't connected. See list_services."))?;
                self.b.lock().service_checks.remove(&acc.sid);
                self.wait(Cmd::CheckService(acc.sid))?;
                let until = Instant::now() + Duration::from_secs(45);
                loop {
                    if let Some(r) = self.b.lock().service_checks.get(&acc.sid).filter(|r| !r.is_empty()) {
                        let failed = r.starts_with('!');
                        let text = format!("{}: {}", acc.name, r.trim_start_matches('!'));
                        return if failed { Err(text) } else { Ok(text) };
                    }
                    if Instant::now() > until {
                        return Err(format!("{} is taking a while to answer; try again.", acc.name));
                    }
                    thread::sleep(Duration::from_millis(150));
                }
            }
            "find_speakers" => {
                self.wait(Cmd::Discover)?;
                thread::sleep(Duration::from_secs(4));
                let s = self.b.lock();
                let rooms: usize = s.households.iter().map(|h| h.rooms.len()).sum();
                Ok(format!("{} system(s), {rooms} room(s) found", s.households.len()))
            }
            "add_station" => {
                let url = str_arg("url").ok_or("Give the stream's web address.")?;
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err("The address should start with http:// or https://".into());
                }
                let station = Station { name: str_arg("name").unwrap_or_default(), url, genre: str_arg("genre").unwrap_or_default() };
                self.wait(Cmd::AddStation(station))?;
                Ok("Added to My stations".into())
            }
            "remove_station" => {
                let q = str_arg("station").unwrap_or_default().to_lowercase();
                let found = self.b.lock().config.stations.iter().find(|x| x.url.to_lowercase() == q || x.name.to_lowercase() == q).cloned();
                let found = found.ok_or_else(|| format!("No station “{q}” in My stations"))?;
                self.wait(Cmd::RemoveStation(found.url.clone()))?;
                Ok(format!("Removed “{}” from My stations", found.name))
            }
            "subscribe_podcast" => {
                let item = self.item_by_ref(&str_arg("ref").unwrap_or_default())?;
                let show: Podcast =
                    crate::sources::podcasts::podcast_of(&item).ok_or_else(|| format!("“{}” isn't a podcast show. Search for the show and use its ref.", item.title))?;
                let title = show.title.clone();
                self.wait(Cmd::Subscribe(show))?;
                Ok(format!("Subscribed to “{title}”"))
            }
            "unsubscribe_podcast" => {
                let q = str_arg("show").unwrap_or_default().to_lowercase();
                let found = self.b.lock().config.podcasts.iter().find(|p| p.title.to_lowercase().contains(&q)).cloned();
                let found = found.ok_or_else(|| format!("You're not subscribed to “{q}”"))?;
                self.wait(Cmd::Unsubscribe(found.feed.clone()))?;
                Ok(format!("Unsubscribed from “{}”", found.title))
            }
            _ => Err(String::new()),
        })();
        Some(result)
    }

    // ---------- Resources ----------

    pub(super) fn resources(&self) -> Value {
        json!({"resources": [
            {"uri": "roomtone://status", "name": "status", "title": "Systems, rooms and what's playing", "mimeType": "application/json"},
            {"uri": "roomtone://now-playing", "name": "now-playing", "title": "What every group is playing", "mimeType": "application/json"},
            {"uri": "roomtone://favorites", "name": "favorites", "title": "The system's favorites", "mimeType": "text/plain"},
            {"uri": "roomtone://alarms", "name": "alarms", "title": "Every alarm", "mimeType": "text/plain"},
        ]})
    }

    pub(super) fn resource_templates(&self) -> Value {
        json!({"resourceTemplates": [
            {"uriTemplate": "roomtone://queue/{room}", "name": "queue", "title": "A room's queue", "mimeType": "text/plain"},
        ]})
    }

    pub(super) fn read_resource(&self, uri: &str) -> Result<Value, (i64, String)> {
        let text = |mime: &str, t: ToolResult| match t {
            Ok(t) => Ok(json!({"contents": [{"uri": uri, "mimeType": mime, "text": t}]})),
            Err(e) => Err((-32603, e)),
        };
        if let Some(room) = uri.strip_prefix("roomtone://queue/") {
            let room = percent_decode(room);
            return text("text/plain", self.call("get_queue", &json!({"room": room})));
        }
        match uri {
            "roomtone://status" => text("application/json", self.call("get_status", &json!({}))),
            "roomtone://now-playing" => {
                let s = self.b.lock();
                let groups: Vec<Value> = s.households.iter().flat_map(|h| h.groups.iter().map(|g| self.group_line(&s, h, g))).collect();
                text("application/json", Ok(serde_json::to_string_pretty(&groups).unwrap_or_default()))
            }
            "roomtone://favorites" => text("text/plain", self.call("list_favorites", &json!({}))),
            "roomtone://alarms" => text("text/plain", self.call("list_alarms", &json!({}))),
            _ => Err((-32002, format!("Resource not found: {uri}"))),
        }
    }

    // ---------- Prompts ----------

    pub(super) fn prompts(&self) -> Value {
        json!({"prompts": [
            {"name": "play_for_mood", "title": "Play music for a mood", "description": "Find and play something that fits a mood or activity.",
             "arguments": [{"name": "mood", "description": "e.g. relaxed dinner, focus, party", "required": true}, {"name": "room", "description": "Where to play it", "required": false}]},
            {"name": "bedtime", "title": "Wind down a room", "description": "Lower the volume gently and set a sleep timer.",
             "arguments": [{"name": "room", "required": true}, {"name": "minutes", "description": "Sleep timer length (default 30)", "required": false}]},
            {"name": "whole_home", "title": "Play everywhere", "description": "Group the rooms of a system and play something in all of them.",
             "arguments": [{"name": "what", "description": "What to play", "required": true}]},
            {"name": "morning_alarm", "title": "Set a wake-up alarm", "description": "Create an alarm that plays a favorite.",
             "arguments": [{"name": "room", "required": true}, {"name": "time", "description": "HH:MM", "required": true}]},
        ]})
    }

    pub(super) fn get_prompt(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let arg = |k: &str, d: &str| args.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).unwrap_or(d).to_string();
        let text = match name {
            "play_for_mood" => format!(
                "Play music for this mood in {}: \"{}\". First call get_status. Look through list_favorites and list_playlists for something that fits; if nothing does, use search. Then call play with the best ref and tell me what you picked.",
                arg("room", "the room selected in the app"),
                arg("mood", "relaxed")
            ),
            "bedtime" => format!(
                "Wind down {}: call now_playing, lower the volume to about two-thirds of its current level with set_volume (never above 25), then set sleep_timer to {} minutes. Tell me what you changed.",
                arg("room", "the room"),
                arg("minutes", "30")
            ),
            "whole_home" => format!(
                "Play \"{}\" in every room of the main system: call get_status, then group_all, then find it with list_favorites, list_playlists or search and play it in the lead room. Rooms of the other system can follow with play_along.",
                arg("what", "my favorites")
            ),
            "morning_alarm" => format!(
                "Set a wake-up alarm in {} at {} on weekdays: pick a calm favorite from list_favorites and call create_alarm with its ref, volume 15 and 45 minutes. Confirm the alarm afterwards with list_alarms.",
                arg("room", "the bedroom"),
                arg("time", "07:00")
            ),
            other => return Err((-32602, format!("Unknown prompt: {other}"))),
        };
        Ok(json!({"description": name, "messages": [{"role": "user", "content": {"type": "text", "text": text}}]}))
    }

    // ---------- Completions ----------

    /// Suggest room names (and saved groups, days) while an assistant fills in an argument.
    pub(super) fn complete(&self, params: &Value) -> Value {
        let arg = params.get("argument").cloned().unwrap_or(json!({}));
        let name = arg.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let typed = arg.get("value").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        let s = self.b.lock();
        let options: Vec<String> = match name {
            "room" | "rooms" | "other_room" | "from_room" => s.households.iter().flat_map(|h| h.rooms.iter().map(|r| r.name.clone())).collect(),
            "name" => s.config.groups.iter().map(|g| g.name.clone()).collect(),
            "days" => ["daily", "weekdays", "weekends", "once"].iter().map(|x| x.to_string()).collect(),
            "mood" => ["relaxed", "focus", "dinner", "party", "workout", "sleep"].iter().map(|x| x.to_string()).collect(),
            _ => vec![],
        };
        let mut values: Vec<String> = options.into_iter().filter(|o| o.to_lowercase().starts_with(&typed) || o.to_lowercase().contains(&typed)).collect();
        values.sort();
        values.dedup();
        let total = values.len();
        values.truncate(100);
        json!({"completion": {"values": values, "total": total, "hasMore": total > 100}})
    }
}

fn name_is_extra(name: &str) -> bool {
    tools().iter().any(|t| t.0 == name)
}

/// "Living%20Room" → "Living Room".
pub(super) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
