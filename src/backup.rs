//! Back up and restore: save the things people have built up (Sonos favorites, playlists,
//! alarms, and Roomtone's own lists and settings) to one file, and load them back, on the same
//! system or a new one.
//!
//! The file is plain JSON (`format` 1, see docs/SETTINGS.md). It never holds sign-ins or the
//! assistant access key, so it's safe to keep in a normal backup. Restoring only **adds**:
//! nothing that's already there is changed or removed, and duplicates are skipped.
//!
//! The speakers' side is reached through [`SystemAccess`], so the same logic runs on real
//! speakers, on the demo system and (in the tests) on a pretend system.

use crate::config::{Config, Podcast, SavedGroup, Station};
use crate::model::{Alarm, Item};
use serde::{Deserialize, Serialize};

/// The version of the file layout. Bump it only for changes older versions can't read.
pub const FORMAT: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Backup {
    /// Always `FORMAT`; also how a Roomtone backup is recognised.
    pub roomtone_backup: u32,
    /// Seconds since 1970, when it was made.
    pub created: u64,
    pub app_version: String,
    pub system: SystemInfo,
    pub sonos_favorites: Vec<Item>,
    pub sonos_playlists: Vec<PlaylistBackup>,
    pub alarms: Vec<AlarmBackup>,
    pub app: AppBackup,
    /// Things skipped while reading a playlist file (M3U); not part of the saved format.
    #[serde(skip)]
    pub import_notes: Vec<String>,
}

/// Where it came from (for the person's information; restoring doesn't depend on it).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SystemInfo {
    /// "S1" or "S2", or empty if no system was connected.
    pub generation: String,
    pub rooms: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlaylistBackup {
    pub title: String,
    pub tracks: Vec<Item>,
}

/// An alarm, with its room by **name** (room ids differ between systems).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AlarmBackup {
    pub room: String,
    pub hour: u32,
    pub minute: u32,
    pub duration_min: u32,
    pub recurrence: String,
    pub enabled: bool,
    pub program_uri: String,
    pub program_meta: String,
    pub program_name: String,
    pub play_mode: String,
    pub volume: u8,
    pub include_linked: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GroupBackup {
    pub name: String,
    pub rooms: Vec<String>,
}

/// Roomtone's own lists and settings. No sign-ins, no access key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppBackup {
    pub folders: Vec<String>,
    pub stations: Vec<Station>,
    pub podcasts: Vec<Podcast>,
    pub services: Vec<String>,
    /// Favorites kept in the app (albums, artists, folders, shows the speakers can't store).
    pub favorites: Vec<Item>,
    pub groups: Vec<GroupBackup>,
    pub language: String,
    pub theme: String,
    pub speaker_events: bool,
}

/// What to include (when saving) or restore (when loading).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sections {
    pub favorites: bool,
    pub playlists: bool,
    pub alarms: bool,
    pub app: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Sections { favorites: true, playlists: true, alarms: true, app: true }
    }
}

/// How many of each kind there are (or were added, skipped, failed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub favorites: usize,
    pub playlists: usize,
    pub alarms: usize,
    /// Roomtone's own: stations, podcasts, folders, saved groups and favorites kept in the app.
    pub app: usize,
}

impl Counts {
    pub fn total(&self) -> usize {
        self.favorites + self.playlists + self.alarms + self.app
    }
}

/// What restoring did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub added: Counts,
    /// Already there, so left alone.
    pub skipped: Counts,
    pub failed: Counts,
    /// One plain sentence per thing that failed, and anything else worth saying.
    pub notes: Vec<String>,
}

/// What's inside a backup file, for the person to look over before restoring.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub created: u64,
    pub app_version: String,
    pub generation: String,
    pub counts: Counts,
}

/// The speakers' side of backing up and restoring.
pub trait SystemAccess {
    fn generation(&self) -> String;
    /// Every room: (id, name).
    fn rooms(&self) -> Vec<(String, String)>;
    fn favorites(&self) -> Result<Vec<Item>, String>;
    fn playlists(&self) -> Result<Vec<PlaylistBackup>, String>;
    fn alarms(&self) -> Result<Vec<Alarm>, String>;
    fn add_favorite(&self, item: &Item) -> Result<(), String>;
    fn create_playlist(&self, title: &str, tracks: &[Item]) -> Result<(), String>;
    fn save_alarm(&self, alarm: &Alarm) -> Result<(), String>;
}

impl Backup {
    /// Gather everything wanted. `system` is `None` when no speakers are connected: the
    /// speaker-side sections are then left out (and say so in the notes).
    pub fn build(system: Option<&dyn SystemAccess>, cfg: &Config, sections: Sections, now: u64) -> (Backup, Vec<String>) {
        let mut notes = vec![];
        let mut b = Backup { roomtone_backup: FORMAT, created: now, app_version: env!("CARGO_PKG_VERSION").to_string(), ..Default::default() };
        match system {
            Some(sys) => {
                b.system = SystemInfo { generation: sys.generation(), rooms: sys.rooms().into_iter().map(|(_, n)| n).collect() };
                if sections.favorites {
                    match sys.favorites() {
                        Ok(v) => b.sonos_favorites = v.into_iter().filter(|i| !i.uri.is_empty()).collect(),
                        Err(e) => notes.push(format!("Couldn't read the speakers' favorites: {e}")),
                    }
                }
                if sections.playlists {
                    match sys.playlists() {
                        Ok(v) => b.sonos_playlists = v,
                        Err(e) => notes.push(format!("Couldn't read the speakers' playlists: {e}")),
                    }
                }
                if sections.alarms {
                    match sys.alarms() {
                        Ok(list) => {
                            let rooms = sys.rooms();
                            for a in list {
                                let room = rooms.iter().find(|(id, _)| *id == a.room_uuid).map(|(_, n)| n.clone());
                                match room {
                                    Some(room) => b.alarms.push(AlarmBackup {
                                        room,
                                        hour: a.hour,
                                        minute: a.minute,
                                        duration_min: a.duration_min,
                                        recurrence: a.recurrence,
                                        enabled: a.enabled,
                                        program_uri: a.program_uri,
                                        program_meta: a.program_meta,
                                        program_name: a.program_name,
                                        play_mode: a.play_mode,
                                        volume: a.volume,
                                        include_linked: a.include_linked,
                                    }),
                                    None => notes.push(format!("Skipped an alarm at {:02}:{:02} for a room that wasn't found.", a.hour, a.minute)),
                                }
                            }
                        }
                        Err(e) => notes.push(format!("Couldn't read the alarms: {e}")),
                    }
                }
            }
            None if sections.favorites || sections.playlists || sections.alarms => {
                notes.push("No speakers are connected, so favorites, playlists and alarms from the speakers weren't included.".to_string());
            }
            None => {}
        }
        if sections.app {
            b.app = AppBackup {
                folders: cfg.folders.clone(),
                stations: cfg.stations.clone(),
                podcasts: cfg.podcasts.clone(),
                services: cfg.services.clone(),
                favorites: cfg.favorites.clone(),
                groups: cfg.groups.iter().map(|g| GroupBackup { name: g.name.clone(), rooms: g.rooms.clone() }).collect(),
                language: cfg.language.clone(),
                theme: cfg.theme.clone(),
                speaker_events: cfg.speaker_events,
            };
        }
        (b, notes)
    }

    /// Read a backup file's text. Gives a plain reason if it isn't one of ours.
    pub fn parse(text: &str) -> Result<Backup, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("this isn't a readable backup file ({e})"))?;
        let format = value.get("roomtone_backup").and_then(|v| v.as_u64()).ok_or("this isn't a Roomtone backup file")?;
        if format as u32 > FORMAT {
            return Err(format!("this backup was made by a newer Roomtone (format {format}); update Roomtone to restore it"));
        }
        serde_json::from_value(value).map_err(|e| format!("this backup is damaged ({e})"))
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn summary(&self) -> Summary {
        let a = &self.app;
        Summary {
            created: self.created,
            app_version: self.app_version.clone(),
            generation: self.system.generation.clone(),
            counts: Counts {
                favorites: self.sonos_favorites.len(),
                playlists: self.sonos_playlists.len(),
                alarms: self.alarms.len(),
                app: a.folders.len() + a.stations.len() + a.podcasts.len() + a.favorites.len() + a.groups.len(),
            },
        }
    }

    /// Add what's in the backup to the system and to `cfg`. Existing things are never changed
    /// or removed. `household` is the system the saved groups should belong to.
    pub fn restore(&self, system: Option<&dyn SystemAccess>, cfg: &mut Config, sections: Sections, household: &str) -> Report {
        let mut r = Report { notes: self.import_notes.clone(), ..Default::default() };
        let on_speakers = sections.favorites || sections.playlists || sections.alarms;
        if on_speakers && system.is_none() && (!self.sonos_favorites.is_empty() || !self.sonos_playlists.is_empty() || !self.alarms.is_empty()) {
            r.notes.push("No speakers are connected, so favorites, playlists and alarms weren't restored.".to_string());
        }
        if let Some(sys) = system {
            if sections.favorites {
                self.restore_favorites(sys, &mut r);
            }
            if sections.playlists {
                self.restore_playlists(sys, &mut r);
            }
            if sections.alarms {
                self.restore_alarms(sys, &mut r);
            }
        }
        if sections.app {
            self.restore_app(cfg, household, &mut r);
        }
        r
    }

    fn restore_favorites(&self, sys: &dyn SystemAccess, r: &mut Report) {
        let have: Vec<(String, String)> = match sys.favorites() {
            Ok(v) => v.into_iter().map(|i| (i.title.to_lowercase(), i.uri)).collect(),
            Err(e) => {
                r.notes.push(format!("Couldn't check the existing favorites, so none were restored: {e}"));
                r.failed.favorites += self.sonos_favorites.len();
                return;
            }
        };
        for f in &self.sonos_favorites {
            if have.iter().any(|(t, u)| *t == f.title.to_lowercase() && *u == f.uri) {
                r.skipped.favorites += 1;
                continue;
            }
            let mut item = f.clone();
            item.id.clear();
            match sys.add_favorite(&item) {
                Ok(()) => r.added.favorites += 1,
                Err(e) => {
                    r.failed.favorites += 1;
                    r.notes.push(format!("Favorite “{}”: {e}", f.title));
                }
            }
        }
    }

    fn restore_playlists(&self, sys: &dyn SystemAccess, r: &mut Report) {
        let have: Vec<String> = match sys.playlists() {
            Ok(v) => v.into_iter().map(|p| p.title.to_lowercase()).collect(),
            Err(e) => {
                r.notes.push(format!("Couldn't check the existing playlists, so none were restored: {e}"));
                r.failed.playlists += self.sonos_playlists.len();
                return;
            }
        };
        for p in &self.sonos_playlists {
            if have.contains(&p.title.to_lowercase()) {
                r.skipped.playlists += 1;
                continue;
            }
            if p.tracks.is_empty() {
                r.skipped.playlists += 1;
                r.notes.push(format!("Playlist “{}” has no songs, so it wasn't created.", p.title));
                continue;
            }
            match sys.create_playlist(&p.title, &p.tracks) {
                Ok(()) => r.added.playlists += 1,
                Err(e) => {
                    r.failed.playlists += 1;
                    r.notes.push(format!("Playlist “{}”: {e}", p.title));
                }
            }
        }
    }

    fn restore_alarms(&self, sys: &dyn SystemAccess, r: &mut Report) {
        let rooms = sys.rooms();
        let have = match sys.alarms() {
            Ok(v) => v,
            Err(e) => {
                r.notes.push(format!("Couldn't check the existing alarms, so none were restored: {e}"));
                r.failed.alarms += self.alarms.len();
                return;
            }
        };
        for a in &self.alarms {
            let Some((uuid, _)) = rooms.iter().find(|(_, n)| n.eq_ignore_ascii_case(&a.room)) else {
                r.failed.alarms += 1;
                r.notes.push(format!("Alarm at {:02}:{:02}: there's no room called “{}” on this system.", a.hour, a.minute, a.room));
                continue;
            };
            if have.iter().any(|h| h.room_uuid == *uuid && h.hour == a.hour && h.minute == a.minute && h.recurrence == a.recurrence && h.program_uri == a.program_uri) {
                r.skipped.alarms += 1;
                continue;
            }
            let alarm = Alarm {
                id: String::new(),
                hour: a.hour,
                minute: a.minute,
                duration_min: a.duration_min,
                recurrence: a.recurrence.clone(),
                enabled: a.enabled,
                room_uuid: uuid.clone(),
                program_uri: a.program_uri.clone(),
                program_meta: a.program_meta.clone(),
                program_name: a.program_name.clone(),
                play_mode: a.play_mode.clone(),
                volume: a.volume,
                include_linked: a.include_linked,
            };
            match sys.save_alarm(&alarm) {
                Ok(()) => r.added.alarms += 1,
                Err(e) => {
                    r.failed.alarms += 1;
                    r.notes.push(format!("Alarm at {:02}:{:02} in {}: {e}", a.hour, a.minute, a.room));
                }
            }
        }
    }

    fn restore_app(&self, cfg: &mut Config, household: &str, r: &mut Report) {
        let a = &self.app;
        for f in &a.folders {
            if cfg.folders.iter().any(|x| x == f) {
                r.skipped.app += 1;
            } else {
                cfg.folders.push(f.clone());
                r.added.app += 1;
            }
        }
        for s in &a.stations {
            if cfg.stations.iter().any(|x| x.url == s.url) {
                r.skipped.app += 1;
            } else {
                cfg.stations.push(s.clone());
                r.added.app += 1;
            }
        }
        for p in &a.podcasts {
            if cfg.podcasts.iter().any(|x| x.feed == p.feed) {
                r.skipped.app += 1;
            } else {
                cfg.podcasts.push(p.clone());
                r.added.app += 1;
            }
        }
        for f in &a.favorites {
            if cfg.favorites.iter().any(|x| x.id == f.id) {
                r.skipped.app += 1;
            } else {
                cfg.favorites.push(f.clone());
                r.added.app += 1;
            }
        }
        for g in &a.groups {
            if cfg.groups.iter().any(|x| x.name.eq_ignore_ascii_case(&g.name) && x.household == household) {
                r.skipped.app += 1;
            } else {
                cfg.groups.push(SavedGroup { name: g.name.clone(), household: household.to_string(), rooms: g.rooms.clone() });
                r.added.app += 1;
            }
        }
        // Plain preferences come across only if they're ones this version understands.
        if ["auto", "en", "de", "fr", "es", "it", "nl"].contains(&a.language.as_str()) {
            cfg.language = a.language.clone();
        }
        if ["auto", "light", "dark"].contains(&a.theme.as_str()) {
            cfg.theme = a.theme.clone();
        }
        for s in &a.services {
            if ["local", "radio", "podcasts", "archive"].contains(&s.as_str()) && !cfg.services.contains(s) {
                cfg.services.push(s.clone());
            }
        }
        // Only a backup that has an app part at all (not one saved without it) sets this.
        if !a.language.is_empty() || !a.theme.is_empty() {
            cfg.speaker_events = a.speaker_events;
        }
    }
}

/// The path a playlist is exported to, with a file type Roomtone can write. The type is the one
/// chosen in the Save dialog (or typed): `.m3u8`/`.m3u`, `.csv` or `.json`. Anything else, or
/// no extension (some systems' dialogs don't add one), becomes an M3U8 file by adding `.m3u8`.
pub fn playlist_export_path(path: std::path::PathBuf) -> std::path::PathBuf {
    let known = path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["m3u8", "m3u", "csv", "json"].contains(&e.to_lowercase().as_str()));
    if known {
        return path;
    }
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".m3u8");
    path.with_file_name(name)
}

/// Today's date as `2026-11-13`, for a file name (UTC).
pub fn date_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::cell::RefCell;

    /// A pretend system that remembers what's added to it.
    #[derive(Default)]
    struct Fake {
        favorites: RefCell<Vec<Item>>,
        playlists: RefCell<Vec<PlaylistBackup>>,
        alarms: RefCell<Vec<Alarm>>,
        refuse: bool,
        /// Pretend the system has other rooms (so alarms can't find theirs).
        other_rooms: bool,
    }

    fn track(title: &str) -> Item {
        Item { title: title.into(), uri: format!("x-file-cifs://nas/{title}.mp3"), ..Default::default() }
    }

    impl SystemAccess for Fake {
        fn generation(&self) -> String {
            "S2".into()
        }
        fn rooms(&self) -> Vec<(String, String)> {
            if self.other_rooms {
                return vec![("RINCON_X".into(), "Garage".into())];
            }
            vec![("RINCON_A".into(), "Kitchen".into()), ("RINCON_B".into(), "Bedroom".into())]
        }
        fn favorites(&self) -> Result<Vec<Item>, String> {
            Ok(self.favorites.borrow().clone())
        }
        fn playlists(&self) -> Result<Vec<PlaylistBackup>, String> {
            Ok(self.playlists.borrow().clone())
        }
        fn alarms(&self) -> Result<Vec<Alarm>, String> {
            Ok(self.alarms.borrow().clone())
        }
        fn add_favorite(&self, item: &Item) -> Result<(), String> {
            if self.refuse {
                return Err("the speaker said no".into());
            }
            self.favorites.borrow_mut().push(item.clone());
            Ok(())
        }
        fn create_playlist(&self, title: &str, tracks: &[Item]) -> Result<(), String> {
            self.playlists.borrow_mut().push(PlaylistBackup { title: title.into(), tracks: tracks.to_vec() });
            Ok(())
        }
        fn save_alarm(&self, alarm: &Alarm) -> Result<(), String> {
            self.alarms.borrow_mut().push(alarm.clone());
            Ok(())
        }
    }

    fn stocked() -> Fake {
        let f = Fake::default();
        f.favorites.borrow_mut().push(Item { title: "Jazz FM".into(), uri: "x-rincon-mp3radio://jazz".into(), ..Default::default() });
        f.playlists.borrow_mut().push(PlaylistBackup { title: "Road trip".into(), tracks: vec![track("one"), track("two")] });
        f.alarms.borrow_mut().push(Alarm {
            id: "7".into(),
            hour: 7,
            minute: 30,
            recurrence: "WEEKDAYS".into(),
            enabled: true,
            room_uuid: "RINCON_B".into(),
            volume: 15,
            ..Default::default()
        });
        f
    }

    fn config() -> Config {
        let mut c = Config::default();
        c.folders.push("/music".into());
        c.stations.push(Station { name: "Jazz".into(), url: "http://x/jazz".into(), genre: String::new() });
        c.groups.push(SavedGroup { name: "Downstairs".into(), household: "OLD".into(), rooms: vec!["Kitchen".into()] });
        c.accounts.push(crate::config::Account { sid: 9, name: "Spotify".into(), token: "SECRET".into(), key: "SECRETKEY".into(), ..Default::default() });
        c.mcp.token = "ACCESSKEY".into();
        c
    }

    #[test]
    fn a_backup_holds_everything_but_secrets() {
        let (b, notes) = Backup::build(Some(&stocked()), &config(), Sections::default(), 1_700_000_000);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(b.system.generation, "S2");
        assert_eq!(b.sonos_favorites.len(), 1);
        assert_eq!(b.sonos_playlists[0].tracks.len(), 2);
        assert_eq!(b.alarms[0].room, "Bedroom", "alarms are kept by room name");
        assert_eq!(b.app.groups[0].name, "Downstairs");
        let json = b.to_json().unwrap();
        assert!(!json.contains("SECRET") && !json.contains("ACCESSKEY"), "no sign-ins or access key in the file");
        assert_eq!(Backup::parse(&json).unwrap(), b, "it reads back exactly");
    }

    #[test]
    fn sections_can_be_left_out() {
        let only_app = Sections { favorites: false, playlists: false, alarms: false, app: true };
        let (b, notes) = Backup::build(Some(&stocked()), &config(), only_app, 0);
        assert!(notes.is_empty());
        assert!(b.sonos_favorites.is_empty() && b.sonos_playlists.is_empty() && b.alarms.is_empty());
        assert_eq!(b.app.folders, ["/music"]);
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections { app: false, ..Sections::default() }, 0);
        assert_eq!(b.app, AppBackup::default());
    }

    #[test]
    fn without_speakers_only_the_app_part_is_saved() {
        let (b, notes) = Backup::build(None, &config(), Sections::default(), 0);
        assert!(b.sonos_favorites.is_empty());
        assert_eq!(b.app.stations.len(), 1);
        assert_eq!(notes.len(), 1, "{notes:?}");
    }

    #[test]
    fn restoring_onto_an_empty_system_adds_everything() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let new_system = Fake::default();
        let mut cfg = Config::default();
        let r = b.restore(Some(&new_system), &mut cfg, Sections::default(), "NEWHOUSE");
        assert_eq!(r.added, Counts { favorites: 1, playlists: 1, alarms: 1, app: 3 }, "{r:?}");
        assert_eq!(r.failed.total(), 0);
        assert_eq!(new_system.playlists.borrow()[0].tracks.len(), 2, "the playlist's songs came with it");
    }

    #[test]
    fn an_alarm_for_a_missing_room_is_reported_not_guessed() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let garage_only = Fake { other_rooms: true, ..Default::default() };
        let mut cfg = Config::default();
        let r = b.restore(Some(&garage_only), &mut cfg, Sections::default(), "H");
        assert_eq!((r.added.alarms, r.failed.alarms), (0, 1));
        assert!(r.notes.iter().any(|n| n.contains("no room called “Bedroom”")), "{:?}", r.notes);
        assert!(garage_only.alarms.borrow().is_empty());
    }

    #[test]
    fn alarms_find_their_room_by_name_on_the_new_system() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let target = Fake::default(); // has a Bedroom too, under a different id in real life
        let mut cfg = Config::default();
        let r = b.restore(Some(&target), &mut cfg, Sections::default(), "H");
        assert_eq!(r.added.alarms, 1, "{r:?}");
        let made = target.alarms.borrow();
        assert_eq!(made[0].room_uuid, "RINCON_B");
        assert!(made[0].id.is_empty(), "a new alarm, not an update of someone else's");
        assert_eq!((made[0].hour, made[0].minute, made[0].volume), (7, 30, 15));
    }

    #[test]
    fn restoring_twice_adds_nothing_the_second_time() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let target = Fake::default();
        let mut cfg = Config::default();
        let first = b.restore(Some(&target), &mut cfg, Sections::default(), "H");
        let again = b.restore(Some(&target), &mut cfg, Sections::default(), "H");
        assert_eq!(again.added.total(), 0, "{again:?}");
        assert_eq!(again.skipped.total(), first.added.total(), "everything is recognised as already there");
        assert_eq!(target.favorites.borrow().len(), 1);
        assert_eq!(cfg.stations.len(), 1);
    }

    #[test]
    fn restoring_never_touches_what_is_already_there() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let target = Fake::default();
        target.favorites.borrow_mut().push(Item { title: "Mine".into(), uri: "x://mine".into(), ..Default::default() });
        let mut cfg = Config::default();
        cfg.stations.push(Station { name: "Mine".into(), url: "http://mine".into(), genre: String::new() });
        cfg.language = "de".into();
        let r = b.restore(Some(&target), &mut cfg, Sections { app: false, ..Sections::default() }, "H");
        assert!(r.added.favorites == 1 && target.favorites.borrow().len() == 2, "added next to the existing one");
        assert_eq!(cfg.stations.len(), 1, "the app part was switched off");
        assert_eq!(cfg.language, "de");
    }

    #[test]
    fn saved_groups_move_to_the_new_system() {
        let (b, _) = Backup::build(None, &config(), Sections::default(), 0);
        let mut cfg = Config::default();
        b.restore(None, &mut cfg, Sections::default(), "NEWHOUSE");
        assert_eq!(cfg.groups[0].household, "NEWHOUSE");
        assert_eq!(cfg.groups[0].rooms, ["Kitchen"]);
    }

    #[test]
    fn failures_are_counted_and_explained_without_stopping() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let target = Fake { refuse: true, ..Default::default() };
        let mut cfg = Config::default();
        let r = b.restore(Some(&target), &mut cfg, Sections::default(), "H");
        assert_eq!(r.failed.favorites, 1);
        assert_eq!(r.added.playlists, 1, "the rest carries on");
        assert!(r.notes.iter().any(|n| n.contains("the speaker said no")), "{:?}", r.notes);
    }

    #[test]
    fn restoring_without_speakers_still_restores_the_app_part() {
        let (b, _) = Backup::build(Some(&stocked()), &config(), Sections::default(), 0);
        let mut cfg = Config::default();
        let r = b.restore(None, &mut cfg, Sections::default(), "H");
        assert_eq!(r.added.favorites + r.added.playlists + r.added.alarms, 0);
        assert!(r.added.app > 0 && !cfg.stations.is_empty());
        assert_eq!(r.notes.len(), 1);
    }

    #[test]
    fn files_that_are_not_ours_are_refused_kindly() {
        assert!(Backup::parse("not json").unwrap_err().contains("readable"));
        assert!(Backup::parse(r#"{"hello": 1}"#).unwrap_err().contains("isn't a Roomtone backup"));
        let newer = format!(r#"{{"roomtone_backup": {}}}"#, FORMAT + 1);
        assert!(Backup::parse(&newer).unwrap_err().contains("newer Roomtone"));
        let minimal = Backup::parse(r#"{"roomtone_backup": 1}"#).unwrap();
        assert_eq!(minimal.summary().counts.total(), 0, "missing sections are just empty");
    }

    #[test]
    fn bad_preferences_in_a_backup_are_ignored() {
        let mut b = Backup { roomtone_backup: FORMAT, ..Default::default() };
        b.app.language = "klingon".into();
        b.app.theme = "neon".into();
        let mut cfg = Config::default();
        b.restore(None, &mut cfg, Sections::default(), "H");
        assert_eq!((cfg.language.as_str(), cfg.theme.as_str()), ("auto", "auto"));
    }

    #[test]
    fn exported_playlists_always_get_a_type_we_can_write() {
        use std::path::PathBuf;
        let p = |s: &str| playlist_export_path(PathBuf::from(s));
        assert_eq!(p("/a/Road trip.m3u8"), PathBuf::from("/a/Road trip.m3u8"));
        assert_eq!(p("/a/Road trip.CSV"), PathBuf::from("/a/Road trip.CSV"));
        assert_eq!(p("/a/list.json"), PathBuf::from("/a/list.json"));
        assert_eq!(p("/a/Road trip"), PathBuf::from("/a/Road trip.m3u8"), "no type typed: M3U8");
        assert_eq!(p("/a/mix.v2"), PathBuf::from("/a/mix.v2.m3u8"), "an unknown ending is kept and the type added");
        assert_eq!(p("/a/mix.txt"), PathBuf::from("/a/mix.txt.m3u8"));
    }

    #[test]
    fn dates_for_file_names() {
        assert_eq!(date_stamp(0), "1970-01-01");
        assert_eq!(date_stamp(1_794_570_436), "2026-11-13");
        assert_eq!(date_stamp(1_709_164_800), "2024-02-29", "leap day");
    }

    /// A real speaker, read only: writes are refused, so nothing on the system can change.
    struct ReadOnly(String);

    impl SystemAccess for ReadOnly {
        fn generation(&self) -> String {
            "?".into()
        }
        fn rooms(&self) -> Vec<(String, String)> {
            vec![]
        }
        fn favorites(&self) -> Result<Vec<Item>, String> {
            crate::net::speaker::browse(&self.0, "FV:2")
        }
        fn playlists(&self) -> Result<Vec<PlaylistBackup>, String> {
            crate::net::speaker::playlists_with_tracks(&self.0)
        }
        fn alarms(&self) -> Result<Vec<Alarm>, String> {
            crate::net::speaker::alarms(&self.0)
        }
        fn add_favorite(&self, _: &Item) -> Result<(), String> {
            Err("read-only".into())
        }
        fn create_playlist(&self, _: &str, _: &[Item]) -> Result<(), String> {
            Err("read-only".into())
        }
        fn save_alarm(&self, _: &Alarm) -> Result<(), String> {
            Err("read-only".into())
        }
    }

    /// Silent, read-only check against a real speaker (prints counts only):
    /// `ROOMTONE_SPEAKER=<ip> cargo test real_speaker_backup -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_speaker_backup_export() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else {
            eprintln!("Set ROOMTONE_SPEAKER=<ip> to run this.");
            return;
        };
        let (b, notes) = Backup::build(Some(&ReadOnly(ip)), &Config::default(), Sections::default(), 0);
        let s = b.summary();
        eprintln!("favorites={} playlists={} alarms={} notes={:?}", s.counts.favorites, s.counts.playlists, s.counts.alarms, notes);
        let songs: usize = b.sonos_playlists.iter().map(|p| p.tracks.len()).sum();
        let with_meta = b.sonos_favorites.iter().filter(|f| !f.meta.is_empty()).count();
        eprintln!("songs in playlists={songs} favorites with details={with_meta}");
        let json = b.to_json().unwrap();
        assert_eq!(Backup::parse(&json).unwrap(), b, "what was read comes back the same");
    }
}
