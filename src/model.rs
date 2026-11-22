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
    // Home-theatre settings. Each is `None` unless the room has what it needs: a sub or surrounds
    // bonded to it, a soundbar model, a measured room tuning (see `SoundKind`).
    pub sub_enabled: Option<bool>,
    pub sub_level: Option<i32>,
    pub sub_inverted: Option<bool>,
    pub surround_enabled: Option<bool>,
    pub surround_tv: Option<i32>,
    pub surround_music: Option<i32>,
    pub height: Option<i32>,
    pub delay: Option<i32>,
    pub trueplay: Option<bool>,
}

/// The home-theatre settings a room may have, besides bass, treble and the on/off ones.
/// On/off ones carry 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundKind {
    SubEnabled,
    SubLevel,
    SubInverted,
    SurroundEnabled,
    SurroundTv,
    SurroundMusic,
    Height,
    Delay,
    /// A measured room tuning (Trueplay) on or off. Roomtone can't make one.
    Trueplay,
}

impl SoundKind {
    pub const ALL: [SoundKind; 9] = [
        SoundKind::SubEnabled,
        SoundKind::SubLevel,
        SoundKind::SubInverted,
        SoundKind::SurroundEnabled,
        SoundKind::SurroundTv,
        SoundKind::SurroundMusic,
        SoundKind::Height,
        SoundKind::Delay,
        SoundKind::Trueplay,
    ];

    /// The `EQType` the speaker's `GetEQ` and `SetEQ` know it by (not for the room tuning, which has its own action).
    pub fn eq_type(self) -> Option<&'static str> {
        Some(match self {
            SoundKind::SubEnabled => "SubEnabled",
            SoundKind::SubLevel => "SubGain",
            SoundKind::SubInverted => "SubPolarity",
            SoundKind::SurroundEnabled => "SurroundEnabled",
            SoundKind::SurroundTv => "SurroundLevel",
            SoundKind::SurroundMusic => "MusicSurroundLevel",
            SoundKind::Height => "HeightChannelLevel",
            SoundKind::Delay => "AudioDelay",
            SoundKind::Trueplay => return None,
        })
    }

    /// On/off (0 or 1), as opposed to a level.
    pub fn is_toggle(self) -> bool {
        matches!(self, SoundKind::SubEnabled | SoundKind::SubInverted | SoundKind::SurroundEnabled | SoundKind::Trueplay)
    }

    /// The lowest and highest value Roomtone sends. These are the ranges the maker's own app is
    /// known to use; the speakers don't publish them, so a value outside them is refused by the speaker.
    pub fn range(self) -> (i32, i32) {
        match self {
            SoundKind::SubLevel | SoundKind::SurroundTv | SoundKind::SurroundMusic => (-15, 15),
            SoundKind::Height => (-10, 10),
            SoundKind::Delay => (0, 5),
            _ => (0, 1),
        }
    }

    /// The name assistants use for it.
    pub fn key(self) -> &'static str {
        match self {
            SoundKind::SubEnabled => "sub_enabled",
            SoundKind::SubLevel => "sub_level",
            SoundKind::SubInverted => "sub_inverted",
            SoundKind::SurroundEnabled => "surround_enabled",
            SoundKind::SurroundTv => "surround_tv_level",
            SoundKind::SurroundMusic => "surround_music_level",
            SoundKind::Height => "height_level",
            SoundKind::Delay => "audio_delay",
            SoundKind::Trueplay => "trueplay",
        }
    }

    /// What a room needs to have this setting, in words (for an assistant that asks for one it lacks).
    pub fn needs(self) -> &'static str {
        match self {
            SoundKind::SubEnabled | SoundKind::SubLevel | SoundKind::SubInverted => "sub bonded to it",
            SoundKind::SurroundEnabled | SoundKind::SurroundTv | SoundKind::SurroundMusic => "pair of surround speakers bonded to it",
            SoundKind::Height => "height channel (an Arc)",
            SoundKind::Delay => "soundbar",
            SoundKind::Trueplay => "tuning measured in the Sonos app",
        }
    }

    pub fn clamp(self, v: i32) -> i32 {
        let (lo, hi) = self.range();
        v.clamp(lo, hi)
    }
}

impl RoomSettings {
    /// The value of a home-theatre setting, or `None` if this room doesn't have it.
    pub fn get(&self, kind: SoundKind) -> Option<i32> {
        let b = |v: Option<bool>| v.map(i32::from);
        match kind {
            SoundKind::SubEnabled => b(self.sub_enabled),
            SoundKind::SubLevel => self.sub_level,
            SoundKind::SubInverted => b(self.sub_inverted),
            SoundKind::SurroundEnabled => b(self.surround_enabled),
            SoundKind::SurroundTv => self.surround_tv,
            SoundKind::SurroundMusic => self.surround_music,
            SoundKind::Height => self.height,
            SoundKind::Delay => self.delay,
            SoundKind::Trueplay => b(self.trueplay),
        }
    }

    pub fn set(&mut self, kind: SoundKind, v: i32) {
        let v = kind.clamp(v);
        match kind {
            SoundKind::SubEnabled => self.sub_enabled = Some(v != 0),
            SoundKind::SubLevel => self.sub_level = Some(v),
            SoundKind::SubInverted => self.sub_inverted = Some(v != 0),
            SoundKind::SurroundEnabled => self.surround_enabled = Some(v != 0),
            SoundKind::SurroundTv => self.surround_tv = Some(v),
            SoundKind::SurroundMusic => self.surround_music = Some(v),
            SoundKind::Height => self.height = Some(v),
            SoundKind::Delay => self.delay = Some(v),
            SoundKind::Trueplay => self.trueplay = Some(v != 0),
        }
    }

    /// Does the room have any home-theatre setting?
    pub fn has_home_theatre(&self) -> bool {
        SoundKind::ALL.iter().any(|k| self.get(*k).is_some())
    }
}

/// A soundbar (or amp) model name: the ones with TV audio, which have night sound and a lip-sync delay.
pub fn model_is_soundbar(model: &str) -> bool {
    let m = model.to_lowercase();
    ["beam", "arc", "ray", "playbar", "playbase", "amp", "soundbar"].iter().any(|k| m.contains(k))
}

/// The models that have a height channel (Dolby Atmos speakers on top).
pub fn model_has_height(model: &str) -> bool {
    model.to_lowercase().contains("arc")
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

/// Bookkeeping for one play-along link: who was last used, and whether the two have drifted apart.
#[derive(Clone, Debug, Default)]
pub struct LinkSync {
    /// The group the person last used and when: the one the other follows if they drift.
    pub acted: Option<(String, Instant)>,
    /// Since when the two have been on different songs or in different states.
    pub drift_since: Option<Instant>,
    /// When the follower was last brought back in step.
    pub resynced: Option<Instant>,
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
    /// An alarm is ringing here: it can be snoozed or stopped.
    pub alarm_ringing: bool,
    pub queue: Option<Vec<Item>>,
    /// The podcast episode playing here (its audio address), and since when we've known so.
    pub episode: Option<(String, Instant)>,
}

#[derive(Clone, Debug)]
pub struct Household {
    pub id: String,
    pub system: System,
    pub rooms: Vec<Room>,
    pub groups: Vec<Group>,
}

/// The id given to a speaker that reports no household (it was never set up in the Sonos app).
pub const UNSET_PREFIX: &str = "unset-";

impl Household {
    /// False for a speaker that belongs to no system yet: its household id is empty.
    pub fn is_set_up(&self) -> bool {
        !self.id.is_empty() && !self.id.starts_with(UNSET_PREFIX)
    }
    pub fn room(&self, uuid: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.uuid == uuid)
    }
    pub fn room_mut(&mut self, uuid: &str) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|r| r.uuid == uuid)
    }
    /// The groups in the order the person arranged their rooms: a group goes where its
    /// earliest-listed room is. Rooms not in `order` keep the speakers' order, after the others.
    pub fn ordered_groups(&self, order: &[String]) -> Vec<&Group> {
        let rank = |g: &Group| g.members.iter().filter_map(|m| order.iter().position(|o| o == m)).min().unwrap_or(usize::MAX);
        let mut v: Vec<&Group> = self.groups.iter().collect();
        v.sort_by_key(|g| rank(g));
        v
    }

    /// `order` after moving one group up (`-1`) or down (`1`) in what's shown. Rooms of other
    /// systems keep their place in the list. `None` if it can't move that way.
    pub fn order_after_move(&self, order: &[String], group_id: &str, delta: i32) -> Option<Vec<String>> {
        let shown = self.ordered_groups(order);
        let at = shown.iter().position(|g| g.id == group_id)?;
        let to = at as i32 + delta;
        if to < 0 || to as usize >= shown.len() {
            return None;
        }
        let mut ids: Vec<&Group> = shown;
        ids.swap(at, to as usize);
        // The whole order of this system, spelled out so nothing depends on the speakers' order.
        let mine: Vec<String> = ids.iter().flat_map(|g| g.members.iter().cloned()).collect();
        let mut out: Vec<String> = order.iter().filter(|o| !self.rooms.iter().any(|r| &r.uuid == *o)).cloned().collect();
        out.extend(mine);
        Some(out)
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
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// The state of the *Can't find my speakers?* check.
#[derive(Clone, Debug, PartialEq)]
pub enum DiagState {
    Running,
    Done(crate::diagnose::Diagnosis),
}

/// The state of the S2 compatibility look for one system.
#[derive(Clone, Debug, PartialEq)]
pub enum S2State {
    Checking,
    Done(Vec<crate::s2::RoomReport>),
    Failed(String),
}

/// What the last backup action did.
#[derive(Clone, Debug, PartialEq)]
pub enum BackupResult {
    Saved { file: String, counts: crate::backup::Counts, notes: Vec<String> },
    Restored(crate::backup::Report),
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
    /// The system's alarms can't ring because it has no time zone set (read when alarms load).
    pub alarms_zone_unset: bool,
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
    /// Is the now-playing display listening: `ok:PORT`, `err:WHY` or empty when it's off.
    pub frame_status: String,
    /// The speakers' clock: the seconds since midnight (local time) it showed, and when we read it.
    pub clock: Option<(Instant, u32)>,
    /// When the last "limited to" note was shown, so dragging a slider doesn't repeat it.
    pub limit_note_at: Option<Instant>,
    /// A backup file the person chose to restore, shown for them to look over.
    pub backup_preview: Option<(std::path::PathBuf, crate::backup::Summary)>,
    /// What the last save or restore did.
    pub backup_result: Option<BackupResult>,
    /// The settings file couldn't be read at start-up (it was kept aside); shown in Settings.
    pub config_problem: Option<crate::config::Problem>,
    /// How speaker events are going (empty when switched off); see `net::events::Summary`.
    pub events: crate::net::events::Summary,
    /// What the last look for a newer Roomtone found.
    pub new_version: crate::version::Outcome,
    pub version_checking: bool,
    /// Lyrics looked up this run, by `lyrics::key` (kept in memory only).
    pub lyrics: HashMap<String, crate::lyrics::State>,
    /// A check was asked for (*Check now*) and hasn't run yet.
    pub version_check_asked: bool,
    /// The installer being fetched for a newer Roomtone.
    pub update_download: crate::update::Download,
    /// Looks that failed in a row, and when the last one did (kept in memory only).
    pub version_failures: u32,
    pub version_failed_at: u64,
    /// Results of "Check" on the Music Services page, by service number (empty: checking).
    pub service_checks: HashMap<u32, String>,
    /// Groups in different systems playing the same thing: (leading group, following group).
    /// S1 and S2 speakers can't truly group, so the app starts the same music on both and
    /// passes on play, pause, skips and seeks.
    pub links: Vec<(String, String)>,
    /// How each link is getting on, by the leading group's id (see `LinkSync`).
    pub link_sync: HashMap<String, LinkSync>,
    /// Software update status by system (household id).
    pub updates: HashMap<String, UpdateState>,
    /// What the *Can it move to S2?* look found, by system.
    pub s2: HashMap<String, S2State>,
    /// The latest *Can't find my speakers?* check.
    pub diagnosis: Option<DiagState>,
    /// How each room is connected (by room id), once asked in the *Your speakers* window.
    pub connections: HashMap<String, crate::status::Connection>,
}

/// Where a system's software update stands.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Checking,
    UpToDate,
    Available(crate::net::speaker::UpdateOffer),
    /// Sending the update to the speakers: how many have been told so far, of how many.
    Installing(usize, usize),
    /// The speakers are downloading and restarting; how far they have got.
    Progress(UpdateProgress),
    /// Every speaker reports the new software.
    Finished(String),
    /// Still not all on the new software after a long wait.
    Unfinished(UpdateProgress),
    Failed(String),
}

/// How far a software update has got, from what each room itself reports.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateProgress {
    pub version: String,
    pub total: usize,
    /// Rooms that report the new software.
    pub done: usize,
    /// Rooms that aren't answering just now (most likely restarting).
    pub restarting: usize,
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
            alarms_zone_unset: false,
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
            frame_status: String::new(),
            clock: None,
            limit_note_at: None,
            backup_preview: None,
            backup_result: None,
            config_problem: None,
            events: Default::default(),
            new_version: Default::default(),
            version_checking: false,
            lyrics: HashMap::new(),
            version_check_asked: false,
            update_download: Default::default(),
            version_failures: 0,
            version_failed_at: 0,
            service_checks: HashMap::new(),
            links: vec![],
            link_sync: HashMap::new(),
            updates: HashMap::new(),
            s2: HashMap::new(),
            diagnosis: None,
            connections: HashMap::new(),
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
    /// Every room's address in the active system, the first room's first.
    pub fn room_ips(&self) -> Vec<String> {
        self.household().map(|h| h.rooms.iter().map(|r| r.ip.clone()).filter(|ip| !ip.is_empty()).collect()).unwrap_or_default()
    }
    pub fn any_ip(&self) -> Option<String> {
        self.household().and_then(|h| h.rooms.first()).map(|r| r.ip.clone())
    }
    /// The minute of the day (0 to 1439) by the speakers' clock, which is the home's local time;
    /// the demo system uses this computer's clock (UTC). `None` until the clock has been read.
    pub fn local_minute(&self) -> Option<u32> {
        if self.phase == Phase::Demo {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            return Some((secs % 86_400 / 60) as u32);
        }
        let (at, secs) = self.clock?;
        Some(((secs as u64 + at.elapsed().as_secs()) % 86_400 / 60) as u32)
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

#[cfg(test)]
mod unit {
    use super::*;

    fn room(uuid: &str, name: &str, model: &str) -> Room {
        Room {
            uuid: uuid.into(),
            name: name.into(),
            ip: format!("10.0.0.{}", uuid.len()),
            model: model.into(),
            version: String::new(),
            volume: 20,
            muted: false,
            settings: None,
            online: None,
            last_seen: None,
            problem: String::new(),
        }
    }

    fn home() -> Household {
        Household {
            id: "H1".into(),
            system: System::S2,
            rooms: vec![room("A", "Kitchen", "Port"), room("BB", "Lounge", "Beam"), room("CCC", "Study", "One")],
            groups: vec![
                Group { id: "A".into(), coordinator: "A".into(), members: vec!["A".into(), "BB".into()], ..Default::default() },
                Group { id: "CCC".into(), coordinator: "CCC".into(), members: vec!["CCC".into()], ..Default::default() },
            ],
        }
    }

    #[test]
    fn a_speaker_with_no_household_is_not_set_up() {
        let mut h = home();
        assert!(h.is_set_up());
        h.id = String::new();
        assert!(!h.is_set_up());
        h.id = format!("{UNSET_PREFIX}10.0.0.5");
        assert!(!h.is_set_up());
    }

    #[test]
    fn play_modes_round_trip() {
        for shuffle in [false, true] {
            for repeat in [Repeat::Off, Repeat::All, Repeat::One] {
                assert_eq!(parse_play_mode(play_mode(shuffle, repeat)), (shuffle, repeat));
            }
        }
        assert_eq!(parse_play_mode("SOMETHING_NEW"), (false, Repeat::Off));
        assert_eq!(play_mode(true, Repeat::All), "SHUFFLE");
    }

    #[test]
    fn repeat_cycles_off_all_one() {
        assert_eq!(Repeat::Off.next(), Repeat::All);
        assert_eq!(Repeat::All.next(), Repeat::One);
        assert_eq!(Repeat::One.next(), Repeat::Off);
    }

    #[test]
    fn alarm_days_read_naturally() {
        crate::test_support::isolate();
        assert_eq!(recurrence_label("DAILY"), "Every day");
        assert_eq!(recurrence_label("WEEKDAYS"), "Weekdays");
        assert_eq!(recurrence_label("ON_135"), "Mon, Wed, Fri");
        assert_eq!(recurrence_label("ON_06"), "Sun, Sat");
        assert_eq!(recurrence_label("ODD"), "ODD");
    }

    #[test]
    fn kinds_of_items() {
        crate::test_support::isolate();
        let it = |class: &str| Item { class: class.into(), ..Default::default() };
        assert_eq!(item_type(&it("object.container.person.musicArtist")), "Artists");
        assert_eq!(item_type(&it("object.container.album.musicAlbum")), "Albums");
        assert_eq!(item_type(&it("object.item.audioItem.musicTrack")), "Songs");
        assert_eq!(item_type(&it("object.container.playlistContainer")), "Playlists");
        assert_eq!(item_type(&it("object.item.audioItem.audioBroadcast")), "Stations");
        let show = Item { source: "podcasts".into(), is_container: true, ..Default::default() };
        assert_eq!(item_type(&show), "Podcasts");
        let episode = Item { source: "podcasts".into(), ..Default::default() };
        assert_eq!(item_type(&episode), "Episodes");
        assert_eq!(kind_label("object.item.audioItem.musicTrack"), "Song");
        assert_eq!(kind_label("object.container"), "Folder");
        assert!(ITEM_TYPES.contains(&"Other"));
    }

    #[test]
    fn streams_and_favorites() {
        let radio = Item { uri: "x-sonosapi-stream:s123?sid=254".into(), ..Default::default() };
        assert!(radio.is_stream());
        let song = Item { uri: "x-file-cifs://nas/music/a.flac".into(), class: "object.item.audioItem.musicTrack".into(), ..Default::default() };
        assert!(!song.is_stream());
        assert!(song.can_favorite());
        let queue = Item { uri: "x-rincon-queue:RINCON_1#0".into(), ..Default::default() };
        assert!(!queue.can_favorite());
        let fav = Item { id: "FV:2/7".into(), uri: "x-sonosapi-radio:x".into(), ..Default::default() };
        assert!(fav.is_favorite());
        assert!(!fav.can_favorite(), "already a favorite");
        let local_album = Item { id: "LOCAL:ALBUM/x".into(), is_container: true, ..Default::default() };
        assert!(local_album.can_keep_in_app());
        assert!(local_album.can_be_favorite());
        assert!(local_album.browsable());
    }

    #[test]
    fn subtitles() {
        crate::test_support::isolate();
        let a = Item { artist: "Nova".into(), album: "Moon".into(), title: "Song".into(), ..Default::default() };
        assert_eq!(a.subtitle(), "Nova · Moon");
        let b = Item { description: "Radio · Jazz".into(), ..Default::default() };
        assert_eq!(b.subtitle(), "Radio · Jazz");
        let c = Item { class: "object.container.album.musicAlbum".into(), ..Default::default() };
        assert_eq!(c.subtitle(), "Album");
    }

    #[test]
    fn live_position_runs_while_playing_and_stops_at_the_end() {
        let mut n = NowPlaying {
            title: "x".into(),
            position: 10.0,
            duration: 200.0,
            transport: Transport::Paused,
            stamp: Some(Instant::now() - std::time::Duration::from_secs(5)),
            ..Default::default()
        };
        assert_eq!(n.live_position(), 10.0);
        n.transport = Transport::Playing;
        assert!(n.live_position() >= 14.9);
        n.duration = 12.0;
        assert_eq!(n.live_position(), 12.0);
        assert!(!n.is_empty());
        assert!(NowPlaying::default().is_empty());
    }

    #[test]
    fn what_is_playing_as_a_favorite() {
        let stream = NowPlaying { title: "Live".into(), source: "Jazz FM".into(), is_stream: true, media_uri: "x-sonosapi-stream:s1?sid=254".into(), ..Default::default() };
        let item = stream.as_item().expect("a station can be saved");
        assert_eq!(item.title, "Jazz FM");
        assert!(item.class.contains("audioBroadcast"));
        let line_in = NowPlaying { title: "Line-In".into(), is_stream: true, media_uri: "x-rincon-stream:RINCON_1".into(), ..Default::default() };
        assert!(line_in.as_item().is_none());
        let track = NowPlaying { title: "Song".into(), artist: "A".into(), uri: "x-file-cifs://nas/a.mp3".into(), duration: 100.0, ..Default::default() };
        assert_eq!(track.as_item().map(|i| i.title), Some("Song".to_string()));
        assert!(NowPlaying::default().as_item().is_none());
    }

    #[test]
    fn group_names() {
        crate::test_support::isolate();
        let h = home();
        assert_eq!(h.group_name(&h.groups[0]), "Kitchen + 1");
        assert_eq!(h.group_name(&h.groups[1]), "Study");
        let ghost = Group { members: vec!["nobody".into()], ..Default::default() };
        assert_eq!(h.group_name(&ghost), "Unknown room");
    }

    #[test]
    fn inputs_guessed_from_model() {
        let h = home();
        assert!(h.room("A").unwrap().has_line_in());
        assert!(h.room("BB").unwrap().has_tv());
        assert!(!h.room("CCC").unwrap().has_tv());
    }

    #[test]
    fn app_state_lookups() {
        let mut s = AppState { households: vec![home()], ..Default::default() };
        assert_eq!(s.find_group("A").map(|(_, g)| g.members.len()), Some(2));
        assert_eq!(s.find_room("BB").map(|(_, r)| r.name.clone()), Some("Lounge".into()));
        assert_eq!(s.coordinator_ip("A"), Some("10.0.0.1".into()));
        assert_eq!(s.room_ip("CCC"), Some("10.0.0.3".into()));
        assert!(s.any_ip().is_some());
        s.links.push(("A".into(), "Z".into()));
        assert_eq!(s.linked_with("A"), Some("Z"));
        assert_eq!(s.linked_with("Z"), Some("A"));
        assert_eq!(s.linked_with("CCC"), None);
    }

    #[test]
    fn selection_prefers_a_playing_group() {
        let mut h = home();
        h.groups[1].now.transport = Transport::Playing;
        let mut s = AppState { households: vec![h], ..Default::default() };
        s.ensure_selection();
        assert_eq!(s.selected_group.as_deref(), Some("CCC"));
        s.selected_group = Some("gone".into());
        s.ensure_selection();
        assert_eq!(s.selected_group.as_deref(), Some("CCC"));
        let mut empty = AppState::default();
        empty.ensure_selection();
        assert!(empty.selected_group.is_none());
    }

    #[test]
    fn toasts_keep_the_latest_four() {
        let mut s = AppState::default();
        for i in 0..6 {
            s.toast(format!("note {i}"), false);
        }
        assert_eq!(s.toasts.len(), 4);
        assert_eq!(s.toasts.last().map(|t| t.text.as_str()), Some("note 5"));
    }
}

#[cfg(test)]
mod sound_tests {
    use super::*;

    #[test]
    fn every_home_theatre_setting_has_a_name_a_range_and_survives_set_and_get() {
        let mut keys = std::collections::HashSet::new();
        for k in SoundKind::ALL {
            assert!(keys.insert(k.key()), "names are unique: {}", k.key());
            let (lo, hi) = k.range();
            assert!(lo < hi);
            assert_eq!(k.eq_type().is_none(), k == SoundKind::Trueplay, "only the room tuning has its own action");
            assert_eq!(k.is_toggle(), (lo, hi) == (0, 1));
            assert!(!k.needs().is_empty());
            let mut s = RoomSettings::default();
            assert_eq!(s.get(k), None, "nothing until the room says it has it");
            s.set(k, hi + 100);
            assert_eq!(s.get(k), Some(hi), "{k:?} is kept inside its range");
            s.set(k, lo - 100);
            assert_eq!(s.get(k), Some(lo));
            assert!(s.has_home_theatre());
        }
        assert!(!RoomSettings::default().has_home_theatre());
    }

    #[test]
    fn models_with_tv_audio_and_a_height_channel() {
        assert!(model_is_soundbar("Beam") && model_is_soundbar("Arc") && model_is_soundbar("Playbar") && model_is_soundbar("Playbase"));
        assert!(!model_is_soundbar("Play:3") && !model_is_soundbar("One SL"));
        assert!(model_has_height("Arc") && model_has_height("Arc Ultra") && !model_has_height("Beam"));
    }
}
