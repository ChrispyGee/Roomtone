//! Everything the window shows, shared between the UI and the background workers.

use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum System {
    S1,
    S2,
}

impl System {
    pub fn label(self) -> &'static str {
        match self {
            System::S1 => "S1",
            System::S2 => "S2",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RoomSettings {
    pub bass: i32,
    pub treble: i32,
    pub loudness: bool,
    pub night: Option<bool>,
    pub speech: Option<bool>,
    pub status_light: bool,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub uuid: String,
    pub name: String,
    pub ip: String,
    pub model: String,
    pub version: String,
    pub volume: u8,
    pub muted: bool,
    pub settings: Option<RoomSettings>,
    /// Did the speaker answer the last time it was asked? (None: not asked yet.)
    pub online: Option<bool>,
    pub last_seen: Option<Instant>,
    pub problem: String,
}

impl Room {
    /// Best guess from the model name; the speaker refuses the input if it has none.
    pub fn has_line_in(&self) -> bool {
        let m = self.model.to_lowercase();
        ["port", "connect", "amp", "five", "play:5", "era", "zp", "line"].iter().any(|k| m.contains(k))
    }
    pub fn has_tv(&self) -> bool {
        let m = self.model.to_lowercase();
        ["beam", "arc", "ray", "playbar", "playbase", "amp", "soundbar"].iter().any(|k| m.contains(k))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transport {
    #[default]
    Stopped,
    Playing,
    Paused,
    Transitioning,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

pub fn play_mode(shuffle: bool, repeat: Repeat) -> &'static str {
    match (shuffle, repeat) {
        (false, Repeat::Off) => "NORMAL",
        (false, Repeat::All) => "REPEAT_ALL",
        (false, Repeat::One) => "REPEAT_ONE",
        (true, Repeat::Off) => "SHUFFLE_NOREPEAT",
        (true, Repeat::All) => "SHUFFLE",
        (true, Repeat::One) => "SHUFFLE_REPEAT_ONE",
    }
}

pub fn parse_play_mode(mode: &str) -> (bool, Repeat) {
    match mode {
        "REPEAT_ALL" => (false, Repeat::All),
        "REPEAT_ONE" => (false, Repeat::One),
        "SHUFFLE_NOREPEAT" => (true, Repeat::Off),
        "SHUFFLE" => (true, Repeat::All),
        "SHUFFLE_REPEAT_ONE" => (true, Repeat::One),
        _ => (false, Repeat::Off),
    }
}

#[derive(Clone, Debug, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<String>,
    pub duration: f32,
    pub position: f32,
    pub stamp: Option<Instant>,
    pub track_no: u32,
    pub transport: Transport,
    pub source: String,
    pub is_stream: bool,
    /// The address of the song itself.
    pub uri: String,
    /// What the room was asked to play (station, queue...) and the details the speaker reports.
    pub media_uri: String,
    pub media_meta: String,
    pub track_meta: String,
}

impl NowPlaying {
    /// What's playing, as something that can be saved to Favorites.
    pub fn as_item(&self) -> Option<Item> {
        if self.is_empty() {
            return None;
        }
        let item = if self.is_stream {
            let u = &self.media_uri;
            if u.is_empty() || ["x-rincon-stream:", "x-sonos-htastream:", "x-rincon:", "x-rincon-queue:"].iter().any(|p| u.starts_with(p)) {
                return None;
            }
            let name = if !self.source.is_empty() && self.source != "Radio" { self.source.clone() } else { self.title.clone() };
            Item {
                title: name,
                uri: u.clone(),
                meta: self.media_meta.clone(),
                class: "object.item.audioItem.audioBroadcast".into(),
                art: self.art.clone(),
                description: "Station".into(),
                ..Default::default()
            }
        } else {
            if self.uri.is_empty() || self.uri.starts_with("x-rincon") {
                return None;
            }
            Item {
                title: self.title.clone(),
                artist: self.artist.clone(),
                album: self.album.clone(),
                art: self.art.clone(),
                uri: self.uri.clone(),
                meta: if self.uri.starts_with("http") { String::new() } else { self.track_meta.clone() },
                class: "object.item.audioItem.musicTrack".into(),
                duration: self.duration,
                ..Default::default()
            }
        };
        Some(item)
    }
}

impl NowPlaying {
    pub fn live_position(&self) -> f32 {
        let mut p = self.position;
        if self.transport == Transport::Playing {
            if let Some(s) = self.stamp {
                p += s.elapsed().as_secs_f32();
            }
        }
        if self.duration > 0.0 {
            p.min(self.duration)
        } else {
            p
        }
    }
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.source.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Group {
    pub id: String,
    pub coordinator: String,
    pub members: Vec<String>,
    pub now: NowPlaying,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub crossfade: bool,
    pub volume: u8,
    pub muted: bool,
    pub sleep_left: Option<u32>,
    pub queue: Option<Vec<Item>>,
}

#[derive(Clone, Debug)]
pub struct Household {
    pub id: String,
    pub system: System,
    pub rooms: Vec<Room>,
    pub groups: Vec<Group>,
}

impl Household {
    pub fn room(&self, uuid: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.uuid == uuid)
    }
    pub fn room_mut(&mut self, uuid: &str) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|r| r.uuid == uuid)
    }
    pub fn group_name(&self, g: &Group) -> String {
        let names: Vec<&str> = g.members.iter().filter_map(|m| self.room(m).map(|r| r.name.as_str())).collect();
        match names.len() {
            0 => t!("Unknown room").into(),
            1 => names[0].to_string(),
            n => {
                let first = self.room(&g.coordinator).map(|r| r.name.as_str()).unwrap_or(names[0]);
                format!("{first} + {}", n - 1)
            }
        }
    }
}

/// One entry from the speakers' content directory (favourite, playlist, track, album...).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub class: String,
    pub uri: String,
    pub meta: String,
    pub art: Option<String>,
    pub description: String,
    pub is_container: bool,
    pub duration: f32,
    /// Where it came from: library, favorites, playlists, local, mystations, radio, podcasts, archive.
    pub source: String,
    pub genre: String,
    pub track_no: u32,
}

impl Item {
    pub fn subtitle(&self) -> String {
        if !self.description.is_empty() {
            return self.description.clone();
        }
        let mut parts = vec![];
        if !self.artist.is_empty() {
            parts.push(self.artist.clone());
        }
        if !self.album.is_empty() && self.album != self.title {
            parts.push(self.album.clone());
        }
        if parts.is_empty() {
            kind_label(&self.class).to_string()
        } else {
            parts.join(" · ")
        }
    }

    /// Streams and inputs replace what's playing; everything else goes through the queue.
    pub fn is_stream(&self) -> bool {
        let u = self.uri.as_str();
        [
            "x-sonosapi-stream:",
            "x-sonosapi-radio:",
            "x-rincon-mp3radio:",
            "x-sonosapi-hls:",
            "x-sonosapi-hls-static:",
            "hls-radio:",
            "aac:",
            "x-rincon-stream:",
            "x-sonos-htastream:",
            "x-sonos-vli:",
            "demo-radio:",
        ]
        .iter()
        .any(|p| u.starts_with(p))
            || self.class.contains("audioBroadcast")
    }

    /// Can this be saved to the system's Favorites? Anything the speakers can play from a
    /// single address can; folders, podcast shows and archive albums can't.
    pub fn can_favorite(&self) -> bool {
        !self.uri.is_empty() && !self.id.starts_with("FV:") && !["x-rincon-queue:", "x-rincon-stream:", "x-sonos-htastream:", "x-rincon:"].iter().any(|p| self.uri.starts_with(p))
    }

    /// Albums, artists, folders and shows the speakers can't store as a favorite
    /// (no single address) are kept as favorites in this app instead.
    pub fn can_keep_in_app(&self) -> bool {
        self.is_container
            && !self.can_favorite()
            && (self.id.contains('/') || self.id.starts_with("ARTIST:"))
            && !self.id.starts_with("ALL:")
            && self.id != "R:0/0"
            && !self.is_favorite()
    }

    pub fn can_be_favorite(&self) -> bool {
        self.can_favorite() || self.can_keep_in_app()
    }

    /// Is this one of the system's Favorites (so it can be removed)?
    pub fn is_favorite(&self) -> bool {
        self.id.starts_with("FV:2/")
    }

    /// Can this be opened to show what's inside (artist, album, folder)?
    pub fn browsable(&self) -> bool {
        self.is_container && crate::sources::is_source_id(&self.id)
            || self.is_container && !self.id.starts_with("SQ:") && self.uri.is_empty()
            || self.id.starts_with("A:") && self.is_container
            || self.id.starts_with("S:") && self.is_container
            || self.id.starts_with("SQ:") && self.is_container
    }
}

/// The kind of thing an item is, for grouping and filtering search results.
pub fn item_type(item: &Item) -> &'static str {
    let c = item.class.as_str();
    if item.source == "podcasts" {
        return if item.is_container { "Podcasts" } else { "Episodes" };
    }
    if c.contains("musicArtist") || c.contains("person") {
        "Artists"
    } else if c.contains("musicAlbum") {
        "Albums"
    } else if c.contains("musicTrack") {
        "Songs"
    } else if c.contains("playlist") {
        "Playlists"
    } else if c.contains("audioBroadcast") || item.is_stream() {
        "Stations"
    } else if c.contains("genre") {
        "Genres"
    } else if item.is_container {
        "Folders"
    } else {
        "Other"
    }
}

pub const ITEM_TYPES: &[&str] =
    &[n_!("Artists"), n_!("Albums"), n_!("Songs"), n_!("Playlists"), n_!("Stations"), n_!("Podcasts"), n_!("Episodes"), n_!("Folders"), n_!("Genres"), n_!("Other")];

pub fn kind_label(class: &str) -> &'static str {
    if class.contains("musicTrack") {
        crate::i18n::tr(n_!("Song"))
    } else if class.contains("musicAlbum") {
        crate::i18n::tr(n_!("Album"))
    } else if class.contains("musicArtist") || class.contains("person") {
        crate::i18n::tr(n_!("Artist"))
    } else if class.contains("playlist") {
        crate::i18n::tr(n_!("Playlist"))
    } else if class.contains("audioBroadcast") {
        crate::i18n::tr(n_!("Station"))
    } else if class.contains("genre") {
        crate::i18n::tr(n_!("Genre"))
    } else if class.contains("container") {
        crate::i18n::tr(n_!("Folder"))
    } else {
        crate::i18n::tr(n_!("Item"))
    }
}

#[derive(Clone, Debug)]
pub enum Browse {
    Loading,
    Ready(Vec<Item>),
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub struct Alarm {
    pub id: String,
    pub hour: u32,
    pub minute: u32,
    pub duration_min: u32,
    pub recurrence: String,
    pub enabled: bool,
    pub room_uuid: String,
    pub program_uri: String,
    pub program_meta: String,
    pub program_name: String,
    pub play_mode: String,
    pub volume: u8,
    pub include_linked: bool,
}

pub fn recurrence_label(r: &str) -> String {
    match r {
        "DAILY" => t!("Every day").into(),
        "WEEKDAYS" => t!("Weekdays").into(),
        "WEEKENDS" => t!("Weekends").into(),
        "ONCE" => t!("Once").into(),
        other if other.starts_with("ON_") => {
            let names = [t!("Sun"), t!("Mon"), t!("Tue"), t!("Wed"), t!("Thu"), t!("Fri"), t!("Sat")];
            other[3..].chars().filter_map(|c| c.to_digit(10)).filter_map(|d| names.get(d as usize)).copied().collect::<Vec<_>>().join(", ")
        }
        other => other.to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Searching,
    Live,
    Demo,
    /// Shown when a search finds nothing; today the demo system appears instead.
    #[allow(dead_code)]
    NothingFound,
}

pub struct Toast {
    pub text: String,
    pub at: Instant,
    pub error: bool,
}

pub struct AppState {
    pub phase: Phase,
    pub households: Vec<Household>,
    pub active: usize,
    pub selected_group: Option<String>,
    pub browse: HashMap<String, Browse>,
    pub alarms: Option<Vec<Alarm>>,
    pub toasts: Vec<Toast>,
    pub manual_ips: Vec<String>,
    pub recent: Vec<Item>,
    pub config: crate::config::Config,
    pub local_status: String,
    /// Streaming services the speakers say they support.
    pub music_services: Vec<crate::sources::smapi::MusicService>,
    pub link: Option<LinkState>,
    pub reconnecting: bool,
    /// Polls of the selected room that failed in a row.
    pub failures: u32,
    /// The last command the worker finished, and recent failures by command number
    /// (so the assistant link can tell whether what it asked for worked).
    pub done_seq: u64,
    pub errors: Vec<(u64, String)>,
    /// Is the assistant link (MCP) listening right now, and where.
    pub mcp_status: String,
    /// Results of "Check" on the Music Services page, by service number (empty: checking).
    pub service_checks: HashMap<u32, String>,
    /// Groups in different systems playing the same thing: (leading group, following group).
    /// S1 and S2 speakers can't truly group, so the app starts the same music on both and
    /// passes on play, pause, skips and seeks.
    pub links: Vec<(String, String)>,
    /// Software update status by system (household id).
    pub updates: HashMap<String, UpdateState>,
}

/// Where a system's software update stands.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Checking,
    UpToDate,
    Available(crate::net::speaker::UpdateOffer),
    /// Sending the update to the speakers: how many have been told so far, of how many.
    Installing(usize, usize),
    /// The speakers are downloading and restarting.
    Started(String),
    Failed(String),
}

/// A sign-in to a streaming service in progress.
#[derive(Clone, Debug)]
pub struct LinkState {
    pub sid: u32,
    pub name: String,
    pub url: String,
    pub code: String,
    pub status: String,
    pub done: bool,
    pub failed: bool,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            phase: Phase::Searching,
            households: vec![],
            active: 0,
            selected_group: None,
            browse: HashMap::new(),
            alarms: None,
            toasts: vec![],
            manual_ips: vec![],
            recent: vec![],
            config: crate::config::Config::default(),
            local_status: String::new(),
            music_services: vec![],
            link: None,
            reconnecting: false,
            failures: 0,
            done_seq: 0,
            errors: vec![],
            mcp_status: String::new(),
            service_checks: HashMap::new(),
            links: vec![],
            updates: HashMap::new(),
        }
    }
}

impl AppState {
    pub fn household(&self) -> Option<&Household> {
        self.households.get(self.active)
    }
    pub fn find_group(&self, gid: &str) -> Option<(&Household, &Group)> {
        self.households.iter().find_map(|h| h.groups.iter().find(|g| g.id == gid).map(|g| (h, g)))
    }
    pub fn group_mut(&mut self, gid: &str) -> Option<&mut Group> {
        self.households.iter_mut().find_map(|h| h.groups.iter_mut().find(|g| g.id == gid))
    }
    pub fn find_room(&self, uuid: &str) -> Option<(&Household, &Room)> {
        self.households.iter().find_map(|h| h.room(uuid).map(|r| (h, r)))
    }
    pub fn room_mut(&mut self, uuid: &str) -> Option<&mut Room> {
        self.households.iter_mut().find_map(|h| h.room_mut(uuid))
    }
    /// The group in another system that follows this one, or the one it follows.
    pub fn linked_with(&self, gid: &str) -> Option<&str> {
        self.links.iter().find_map(|(l, f)| {
            if l == gid {
                Some(f.as_str())
            } else if f == gid {
                Some(l.as_str())
            } else {
                None
            }
        })
    }
    pub fn coordinator_ip(&self, gid: &str) -> Option<String> {
        let (h, g) = self.find_group(gid)?;
        h.room(&g.coordinator).map(|r| r.ip.clone())
    }
    pub fn room_ip(&self, uuid: &str) -> Option<String> {
        self.find_room(uuid).map(|(_, r)| r.ip.clone())
    }
    /// Any speaker of the active system, for library and alarm calls.
    pub fn any_ip(&self) -> Option<String> {
        self.household().and_then(|h| h.rooms.first()).map(|r| r.ip.clone())
    }
    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast { text: text.into(), at: Instant::now(), error });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }
    pub fn ensure_selection(&mut self) {
        let Some(h) = self.households.get(self.active) else {
            self.selected_group = None;
            return;
        };
        let valid = self.selected_group.as_ref().is_some_and(|s| h.groups.iter().any(|g| &g.id == s));
        if !valid {
            let pick = h.groups.iter().find(|g| g.now.transport == Transport::Playing).or(h.groups.first());
            self.selected_group = pick.map(|g| g.id.clone());
        }
    }
}
