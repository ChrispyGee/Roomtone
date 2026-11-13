//! A pretend household (one S2 and one S1 system) used when no speakers answer,
//! so every screen can be explored. All names and music here are made up.

use std::time::Instant;

use crate::backend::{scale_volume, Cmd};
use crate::model::*;
use crate::net::speaker::PlayHow;

struct Album {
    title: &'static str,
    artist: &'static str,
    genre: &'static str,
    tracks: &'static [(&'static str, u32)],
}

const ALBUMS: &[Album] = &[
    Album {
        title: "Golden Hour Drive",
        artist: "The Paper Lanterns",
        genre: "Indie",
        tracks: &[("Neon Coastline", 214), ("Slow Motion Summer", 198), ("Highway Glow", 241), ("Tidal Hearts", 187), ("After the Rain", 226), ("Last Light", 263)],
    },
    Album {
        title: "Midnight Atlas",
        artist: "Nova Reyes",
        genre: "Electronic",
        tracks: &[("Constellations", 232), ("Northbound", 205), ("Paper Moon", 189), ("Satellite Love", 244), ("Quiet Orbit", 270)],
    },
    Album {
        title: "Velvet Static",
        artist: "Juniper & The Echoes",
        genre: "Indie",
        tracks: &[("Static Bloom", 201), ("Velvet Room", 223), ("Echo Chamber", 196), ("Blue Hour", 251), ("Wildflower Radio", 210)],
    },
    Album {
        title: "Low Tide Sessions",
        artist: "Marlow Quartet",
        genre: "Jazz",
        tracks: &[("Morning Fog", 312), ("Harbor Lights", 287), ("Saltwater Waltz", 265), ("Driftwood", 298)],
    },
    Album {
        title: "Electric Orchard",
        artist: "Kites Over Kyoto",
        genre: "Electronic",
        tracks: &[("Plum Blossom Circuit", 219), ("Lantern Festival", 238), ("Bullet Train Dreams", 207), ("Moss Garden", 254), ("Orchard Lights", 229)],
    },
    Album {
        title: "Northern Soul Kitchen",
        artist: "The Copper Pots",
        genre: "Soul",
        tracks: &[("Simmer Down", 185), ("Sunday Roast", 202), ("Spice Rack Shuffle", 177), ("Late Supper", 233)],
    },
];

const PLAYLISTS: &[(&str, &[(usize, usize)])] = &[
    ("Dinner Party", &[(5, 0), (3, 1), (2, 3), (5, 3), (3, 2), (0, 4)]),
    ("Morning Coffee", &[(3, 0), (0, 1), (1, 2), (5, 1), (2, 1)]),
    ("Workout Mix", &[(4, 2), (1, 3), (0, 2), (4, 0), (2, 0), (1, 1)]),
    ("Rainy Day", &[(0, 4), (3, 3), (1, 4), (2, 3), (4, 3)]),
];

const STATIONS: &[(&str, &str, &str)] = &[
    ("Coastline FM", "Indie and alternative, all day", "Highway Glow|The Paper Lanterns"),
    ("Late Night Jazz Radio", "Smooth jazz after dark", "Harbor Lights|Marlow Quartet"),
    ("Indie Underground", "New music first", "Static Bloom|Juniper & The Echoes"),
    ("Classical Breakfast", "Gentle mornings", "Morning Fog|Marlow Quartet"),
    ("World Beats 24/7", "Global grooves", "Lantern Festival|Kites Over Kyoto"),
    ("Talk of the Town", "News and conversation", "The Morning Show|Talk of the Town"),
];

fn track(a: usize, t: usize) -> Item {
    let al = &ALBUMS[a];
    let (title, secs) = al.tracks[t];
    Item {
        id: format!("A:TRACKS/{a}/{t}"),
        parent_id: format!("A:ALBUM/{a}"),
        title: title.into(),
        artist: al.artist.into(),
        album: al.title.into(),
        class: "object.item.audioItem.musicTrack".into(),
        uri: format!("demo-track:{a}:{t}"),
        duration: secs as f32,
        ..Default::default()
    }
}

fn album(a: usize) -> Item {
    let al = &ALBUMS[a];
    Item {
        id: format!("A:ALBUM/{a}"),
        parent_id: "A:ALBUM".into(),
        title: al.title.into(),
        artist: al.artist.into(),
        album: al.title.into(),
        class: "object.container.album.musicAlbum".into(),
        uri: format!("demo-album:{a}"),
        is_container: true,
        ..Default::default()
    }
}

fn artists() -> Vec<&'static str> {
    let mut v: Vec<&str> = ALBUMS.iter().map(|a| a.artist).collect();
    v.sort();
    v.dedup();
    v
}

fn artist(name: &str) -> Item {
    Item {
        id: format!("A:ARTIST/{name}"),
        parent_id: "A:ARTIST".into(),
        title: name.into(),
        class: "object.container.person.musicArtist".into(),
        uri: format!("demo-artist:{name}"),
        is_container: true,
        ..Default::default()
    }
}

fn playlist(n: usize) -> Item {
    let (name, tracks) = PLAYLISTS[n];
    Item {
        id: format!("SQ:{n}"),
        parent_id: "SQ:".into(),
        title: name.into(),
        description: format!("Playlist · {} songs", tracks.len()),
        class: "object.container.playlistContainer".into(),
        uri: format!("demo-pl:{n}"),
        is_container: true,
        ..Default::default()
    }
}

fn station(n: usize) -> Item {
    let (name, desc, _) = STATIONS[n];
    Item {
        id: format!("R:0/0/{n}"),
        parent_id: "R:0/0".into(),
        title: name.into(),
        description: format!("Radio · {desc}"),
        class: "object.item.audioItem.audioBroadcast".into(),
        uri: format!("demo-radio:{n}"),
        ..Default::default()
    }
}

fn all_tracks() -> Vec<Item> {
    ALBUMS.iter().enumerate().flat_map(|(a, al)| (0..al.tracks.len()).map(move |t| track(a, t))).collect()
}

fn favorites() -> Vec<Item> {
    let mut v = vec![
        station(0),
        {
            let mut a = album(0);
            a.description = "Album · The Paper Lanterns".into();
            a
        },
        playlist(0),
        station(1),
    ];
    let mut t = track(1, 0);
    t.description = "Song · Nova Reyes".into();
    v.push(t);
    v.push(playlist(1));
    let mut a = album(4);
    a.description = "Album · Kites Over Kyoto".into();
    v.push(a);
    v.push(station(2));
    v
}

pub fn browse(id: &str) -> Vec<Item> {
    match id {
        "FV:2" => favorites()
            .into_iter()
            .enumerate()
            .map(|(i, mut f)| {
                f.id = format!("FV:2/{i}:{}", f.id);
                f
            })
            .collect(),
        "SQ:" => (0..PLAYLISTS.len()).map(playlist).collect(),
        "R:0/0" => (0..STATIONS.len()).map(station).collect(),
        "A:ARTIST" | "A:ALBUMARTIST" => artists().into_iter().map(artist).collect(),
        "A:ALBUM" => (0..ALBUMS.len()).map(album).collect(),
        "A:TRACKS" => all_tracks(),
        "A:GENRE" => {
            let mut g: Vec<&str> = ALBUMS.iter().map(|a| a.genre).collect();
            g.sort();
            g.dedup();
            g.into_iter()
                .map(|name| Item {
                    id: format!("A:GENRE/{name}"),
                    parent_id: "A:GENRE".into(),
                    title: name.into(),
                    class: "object.container.genre.musicGenre".into(),
                    is_container: true,
                    ..Default::default()
                })
                .collect()
        }
        "A:COMPOSER" | "A:PLAYLISTS" => vec![],
        "S:" => vec![Item {
            id: "S://nas/music".into(),
            parent_id: "S:".into(),
            title: "Music on network drive".into(),
            class: "object.container".into(),
            is_container: true,
            ..Default::default()
        }],
        "S://nas/music" => (0..ALBUMS.len())
            .map(|a| Item {
                id: format!("A:ALBUM/{a}"),
                title: ALBUMS[a].title.into(),
                artist: ALBUMS[a].artist.into(),
                class: "object.container".into(),
                uri: format!("demo-album:{a}"),
                is_container: true,
                ..Default::default()
            })
            .collect(),
        other => {
            if let Some(name) = other.strip_prefix("A:ARTIST/") {
                ALBUMS.iter().enumerate().filter(|(_, a)| a.artist == name).map(|(i, _)| album(i)).collect()
            } else if let Some(g) = other.strip_prefix("A:GENRE/") {
                ALBUMS.iter().enumerate().filter(|(_, a)| a.genre == g).map(|(i, _)| album(i)).collect()
            } else if let Some(a) = other.strip_prefix("A:ALBUM/").and_then(|a| a.parse::<usize>().ok()) {
                (0..ALBUMS.get(a).map(|x| x.tracks.len()).unwrap_or(0)).map(|t| track(a, t)).collect()
            } else if let Some(n) = other.strip_prefix("SQ:").and_then(|n| n.parse::<usize>().ok()) {
                PLAYLISTS.get(n).map(|p| p.1.iter().map(|(a, t)| track(*a, *t)).collect()).unwrap_or_default()
            } else {
                vec![]
            }
        }
    }
}

pub fn tracks_for(item: &Item) -> Vec<Item> {
    let u = item.uri.as_str();
    if let Some(rest) = u.strip_prefix("demo-track:") {
        let mut p = rest.split(':').filter_map(|x| x.parse::<usize>().ok());
        if let (Some(a), Some(t)) = (p.next(), p.next()) {
            return vec![track(a, t)];
        }
    }
    if let Some(a) = u.strip_prefix("demo-album:").and_then(|a| a.parse::<usize>().ok()) {
        return browse(&format!("A:ALBUM/{a}"));
    }
    if let Some(n) = u.strip_prefix("demo-pl:").and_then(|a| a.parse::<usize>().ok()) {
        return browse(&format!("SQ:{n}"));
    }
    if let Some(name) = u.strip_prefix("demo-artist:") {
        return ALBUMS.iter().enumerate().filter(|(_, a)| a.artist == name).flat_map(|(i, a)| (0..a.tracks.len()).map(move |t| track(i, t))).collect();
    }
    if !item.uri.is_empty() && !item.is_container && !u.starts_with("demo-") {
        let mut t = item.clone();
        t.art = item.art.clone();
        return vec![t];
    }
    if item.is_container {
        return browse(&item.id).into_iter().flat_map(|i| if i.is_container { tracks_for(&i) } else { vec![i] }).collect();
    }
    vec![]
}

fn room(uuid: &str, name: &str, model: &str, version: &str, volume: u8, tv: bool) -> Room {
    Room {
        uuid: uuid.into(),
        name: name.into(),
        ip: String::new(),
        model: model.into(),
        version: version.into(),
        volume,
        muted: false,
        online: Some(true),
        last_seen: None,
        problem: String::new(),
        settings: Some(RoomSettings { bass: 0, treble: 0, loudness: true, night: tv.then_some(false), speech: tv.then_some(false), status_light: true }),
    }
}

fn queue_group(id: &str, members: &[&str], queue: Vec<Item>, idx: usize, transport: Transport, pos: f32, volume: u8) -> Group {
    let mut g = Group { id: id.into(), coordinator: id.into(), members: members.iter().map(|m| m.to_string()).collect(), volume, crossfade: false, ..Default::default() };
    g.queue = Some(queue);
    set_from_queue(&mut g, idx);
    g.now.transport = transport;
    g.now.position = pos;
    g
}

fn set_from_queue(g: &mut Group, idx: usize) {
    let Some(q) = &g.queue else { return };
    let Some(it) = q.get(idx) else { return };
    g.now = NowPlaying {
        title: it.title.clone(),
        artist: it.artist.clone(),
        album: it.album.clone(),
        art: it.art.clone(),
        duration: it.duration,
        position: 0.0,
        stamp: Some(Instant::now()),
        track_no: idx as u32 + 1,
        transport: Transport::Playing,
        source: "Queue".into(),
        is_stream: false,
        uri: it.uri.clone(),
        ..Default::default()
    };
}

fn station_now(n: usize) -> NowPlaying {
    let (name, _, song) = STATIONS[n % STATIONS.len()];
    let (title, artist) = song.split_once('|').unwrap_or((song, ""));
    NowPlaying {
        title: title.into(),
        artist: artist.into(),
        album: name.into(),
        transport: Transport::Playing,
        source: name.into(),
        is_stream: true,
        stamp: Some(Instant::now()),
        media_uri: format!("demo-radio:{}", n % STATIONS.len()),
        ..Default::default()
    }
}

pub fn load(s: &mut AppState) {
    let s2 = Household {
        id: "demo-s2".into(),
        system: System::S2,
        rooms: vec![
            room("D-LIVING", "Living Room", "Soundbar", "16.4", 32, true),
            room("D-KITCHEN", "Kitchen", "Smart Speaker", "16.4", 28, false),
            room("D-BEDROOM", "Bedroom", "Smart Speaker Mini", "16.4", 18, false),
            room("D-PATIO", "Patio", "Outdoor Amp", "16.4", 40, true),
            room("D-STUDY", "Study", "Bookshelf Speaker", "16.4", 22, false),
        ],
        groups: vec![
            queue_group("D-LIVING", &["D-LIVING", "D-KITCHEN"], browse("A:ALBUM/0"), 1, Transport::Playing, 74.0, 30),
            queue_group("D-BEDROOM", &["D-BEDROOM"], browse("SQ:1"), 2, Transport::Paused, 41.0, 18),
            {
                let mut g = Group { id: "D-PATIO".into(), coordinator: "D-PATIO".into(), members: vec!["D-PATIO".into()], volume: 40, ..Default::default() };
                g.queue = Some(vec![]);
                g
            },
            queue_group("D-STUDY", &["D-STUDY"], browse("A:ALBUM/3"), 0, Transport::Stopped, 0.0, 22),
        ],
    };
    let mut office = Group { id: "D-OFFICE".into(), coordinator: "D-OFFICE".into(), members: vec!["D-OFFICE".into()], volume: 24, ..Default::default() };
    office.now = station_now(1);
    office.queue = Some(vec![]);
    let s1 = Household {
        id: "demo-s1".into(),
        system: System::S1,
        rooms: vec![
            room("D-OFFICE", "Office", "Legacy Speaker 5", "11.15", 24, false),
            room("D-GARAGE", "Garage", "Legacy Speaker 1", "11.15", 35, false),
            room("D-DEN", "Den", "Legacy Connect", "11.15", 30, false),
        ],
        groups: vec![office, queue_group("D-GARAGE", &["D-GARAGE", "D-DEN"], browse("SQ:2"), 0, Transport::Paused, 12.0, 32)],
    };
    s.households = vec![s2, s1];
    s.active = 0;
    s.phase = Phase::Demo;
    s.browse.clear();
    s.selected_group = Some("D-LIVING".into());
    s.alarms = Some(vec![
        Alarm {
            id: "1".into(),
            hour: 7,
            minute: 0,
            duration_min: 60,
            recurrence: "WEEKDAYS".into(),
            enabled: true,
            room_uuid: "D-BEDROOM".into(),
            program_uri: "demo-pl:1".into(),
            program_name: "Morning Coffee".into(),
            play_mode: "NORMAL".into(),
            volume: 15,
            ..Default::default()
        },
        Alarm {
            id: "2".into(),
            hour: 8,
            minute: 30,
            duration_min: 120,
            recurrence: "WEEKENDS".into(),
            enabled: false,
            room_uuid: "D-KITCHEN".into(),
            program_uri: "demo-radio:0".into(),
            program_name: "Coastline FM".into(),
            play_mode: "NORMAL".into(),
            volume: 25,
            ..Default::default()
        },
    ]);
    s.recent = vec![album(0), station(1), playlist(1), album(2), station(0), playlist(2)];
}

fn household_of(s: &mut AppState, gid: &str) -> Option<usize> {
    s.households.iter().position(|h| h.groups.iter().any(|g| g.id == gid))
}

pub fn apply(s: &mut AppState, cmd: &Cmd) {
    match cmd {
        Cmd::Discover | Cmd::QuickStart | Cmd::AddManual(_) => {}
        Cmd::UseDemo => load(s),
        Cmd::Play(g) => {
            if let Some(g) = s.group_mut(g) {
                if g.now.is_empty() {
                    if g.queue.as_ref().is_some_and(|q| !q.is_empty()) {
                        set_from_queue(g, 0);
                    } else {
                        g.now.transport = Transport::Stopped;
                    }
                }
            }
        }
        Cmd::Pause(_) | Cmd::Seek(..) | Cmd::SetPlayMode(..) | Cmd::SetCrossfade(..) => {}
        Cmd::Next(g) | Cmd::Previous(g) => {
            let fwd = matches!(cmd, Cmd::Next(_));
            if let Some(g) = s.group_mut(g) {
                if g.now.is_stream {
                    return;
                }
                let len = g.queue.as_ref().map(|q| q.len()).unwrap_or(0);
                if len == 0 {
                    return;
                }
                let cur = g.now.track_no.saturating_sub(1) as usize;
                let next = if fwd {
                    (cur + 1) % len
                } else if g.now.live_position() > 3.0 {
                    cur
                } else {
                    (cur + len - 1) % len
                };
                let state = g.now.transport;
                set_from_queue(g, next);
                if state != Transport::Playing {
                    g.now.transport = state;
                }
            }
        }
        Cmd::GroupVolume(..) | Cmd::GroupMute(..) | Cmd::RoomVolume(..) | Cmd::RoomMute(..) => {}
        Cmd::Browse(id) => {
            let items = browse(id);
            s.browse.insert(id.clone(), Browse::Ready(items));
        }
        Cmd::Search(term) => {
            let t = term.to_lowercase();
            let mut items: Vec<Item> = vec![];
            items.extend(artists().into_iter().filter(|a| a.to_lowercase().contains(&t)).map(artist));
            items.extend((0..ALBUMS.len()).map(album).filter(|a| a.title.to_lowercase().contains(&t) || a.artist.to_lowercase().contains(&t)));
            items.extend(all_tracks().into_iter().filter(|i| i.title.to_lowercase().contains(&t) || i.artist.to_lowercase().contains(&t)));
            items.extend((0..PLAYLISTS.len()).map(playlist).filter(|p| p.title.to_lowercase().contains(&t)));
            items.extend((0..STATIONS.len()).map(station).filter(|p| p.title.to_lowercase().contains(&t)));
            s.browse.insert(format!("search:{term}"), Browse::Ready(items));
        }
        Cmd::PlayItems(gid, tracks, how, title) => enqueue(s, gid, tracks.clone(), *how, title),
        Cmd::ConnectService(_)
        | Cmd::Reconnect
        | Cmd::GroupRooms(_)
        | Cmd::SaveGroup(..)
        | Cmd::DeleteGroup(_)
        | Cmd::CancelLink
        | Cmd::DisconnectService(_)
        | Cmd::SetAccountNumber(..)
        | Cmd::SetLanguage(_)
        | Cmd::SetMcp(_)
        | Cmd::SetEvents(_)
        | Cmd::MarkEpisode(..)
        | Cmd::CheckService(_)
        | Cmd::LinkGroups(..)
        | Cmd::Unlink(_)
        | Cmd::CheckUpdates(_)
        | Cmd::InstallUpdate(_)
        | Cmd::SetTheme(_) => {}
        Cmd::AddFolder(_) | Cmd::RemoveFolder(_) | Cmd::Rescan | Cmd::AddStation(_) | Cmd::RemoveStation(_) | Cmd::Subscribe(_) | Cmd::Unsubscribe(_) | Cmd::SetService(..) => {}
        Cmd::PlayItem(gid, item, how) => {
            if item.is_stream() {
                let now = match item.uri.strip_prefix("demo-radio:") {
                    Some(n) => station_now(n.parse().unwrap_or(0)),
                    None => NowPlaying {
                        title: item.title.clone(),
                        artist: item.description.clone(),
                        art: item.art.clone(),
                        transport: Transport::Playing,
                        source: item.title.clone(),
                        is_stream: true,
                        stamp: Some(Instant::now()),
                        ..Default::default()
                    },
                };
                if let Some(g) = s.group_mut(gid) {
                    g.now = now;
                }
                s.toast(crate::backend::played(PlayHow::Now, &item.title), false);
                return;
            }
            let tracks = tracks_for(item);
            enqueue(s, gid, tracks, *how, &item.title);
            if crate::sources::progress::is_episode(item) {
                let resume = crate::sources::progress::start(item);
                if let Some(g) = s.group_mut(gid) {
                    g.episode = Some((item.uri.clone(), Instant::now()));
                    if let Some(pos) = resume {
                        g.now.position = pos;
                        g.now.stamp = Some(Instant::now());
                    }
                }
            }
        }
        Cmd::LoadQueue(_) => {}
        Cmd::PlayQueueIndex(g, i) => {
            if let Some(g) = s.group_mut(g) {
                set_from_queue(g, *i);
            }
        }
        Cmd::RemoveFromQueue(g, i) => {
            if let Some(g) = s.group_mut(g) {
                let cur = g.now.track_no as usize;
                if let Some(q) = g.queue.as_mut() {
                    if *i < q.len() {
                        q.remove(*i);
                    }
                }
                if !g.now.is_stream && *i + 1 < cur {
                    g.now.track_no -= 1;
                }
            }
        }
        Cmd::MoveInQueue(g, from, to) => {
            if let Some(g) = s.group_mut(g) {
                let cur = g.now.track_no as usize;
                if let Some(q) = g.queue.as_mut() {
                    if *from < q.len() && *to < q.len() {
                        let it = q.remove(*from);
                        q.insert(*to, it);
                        if !g.now.is_stream && cur > 0 {
                            let c = cur - 1;
                            let new_c = if c == *from {
                                *to
                            } else if *from < c && *to >= c {
                                c - 1
                            } else if *from > c && *to <= c {
                                c + 1
                            } else {
                                c
                            };
                            g.now.track_no = new_c as u32 + 1;
                        }
                    }
                }
            }
        }
        Cmd::ClearQueue(g) => {
            if let Some(g) = s.group_mut(g) {
                g.queue = Some(vec![]);
                if !g.now.is_stream {
                    g.now = NowPlaying::default();
                }
            }
        }
        Cmd::SaveQueue(_, title) => s.toast(t!("Saved playlist “{title}”", title = title), false),
        Cmd::AddFavorite(_, item) => {
            let mut list = match s.browse.get("FV:2") {
                Some(Browse::Ready(v)) => v.clone(),
                _ => favorites(),
            };
            let mut fav = item.clone();
            fav.id = format!("FV:2/demo{}", list.len() + 1);
            list.push(fav);
            s.browse.insert("FV:2".into(), Browse::Ready(list));
            s.toast(t!("Added “{title}” to Favorites", title = item.title), false);
        }
        Cmd::RemoveFavorite(id, title) => {
            let mut list = match s.browse.get("FV:2") {
                Some(Browse::Ready(v)) => v.clone(),
                _ => favorites(),
            };
            list.retain(|i| &i.id != id);
            s.browse.insert("FV:2".into(), Browse::Ready(list));
            s.toast(t!("Removed “{title}” from Favorites", title = title), false);
        }
        Cmd::SetGroupMembers(gid, wanted) => set_members(s, gid, wanted),
        Cmd::LineIn(g, src) | Cmd::Tv(g, src) => {
            let is_tv = matches!(cmd, Cmd::Tv(..));
            let name = s.find_room(src).map(|(_, r)| r.name.clone()).unwrap_or_default();
            if let Some(g) = s.group_mut(g) {
                g.now = NowPlaying {
                    title: if is_tv { "TV".into() } else { "Line-In".into() },
                    artist: name,
                    transport: Transport::Playing,
                    source: if is_tv { "TV".into() } else { "Line-In".into() },
                    is_stream: true,
                    stamp: Some(Instant::now()),
                    ..Default::default()
                };
            }
        }
        Cmd::SleepTimer(_, m) => match m {
            Some(m) => s.toast(t!("Sleep timer set for {n} minutes", n = m), false),
            None => s.toast(t!("Sleep timer off"), false),
        },
        Cmd::LoadRoomSettings(_) => {}
        Cmd::SetBass(..) | Cmd::SetTreble(..) | Cmd::SetLoudness(..) | Cmd::SetNight(..) | Cmd::SetSpeech(..) | Cmd::SetStatusLight(..) => {}
        Cmd::Rename(r, name) => {
            if let Some(r) = s.room_mut(r) {
                r.name = name.clone();
            }
        }
        Cmd::LoadAlarms => {}
        Cmd::SaveAlarm(a) => {
            let list = s.alarms.get_or_insert_with(Vec::new);
            let mut a = a.clone();
            if a.program_name.is_empty() {
                a.program_name = "Chime".into();
            }
            if a.id.is_empty() {
                a.id = (list.iter().filter_map(|x| x.id.parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string();
                list.push(a);
            } else if let Some(x) = list.iter_mut().find(|x| x.id == a.id) {
                *x = a;
            }
            list.sort_by_key(|a| a.hour * 60 + a.minute);
        }
        Cmd::DeleteAlarm(id) => {
            if let Some(list) = s.alarms.as_mut() {
                list.retain(|a| &a.id != id);
            }
        }
    }
}

fn enqueue(s: &mut AppState, gid: &str, tracks: Vec<Item>, how: PlayHow, title: &str) {
    if tracks.is_empty() {
        s.toast(t!("“{title}” has nothing playable", title = title), true);
        return;
    }
    let item_title = title;
    if let Some(g) = s.group_mut(gid) {
        let q = g.queue.get_or_insert_with(Vec::new);
        let cur = if g.now.is_stream { q.len() } else { g.now.track_no as usize };
        let at = match how {
            PlayHow::Replace => {
                q.clear();
                0
            }
            PlayHow::Now | PlayHow::Next => cur.min(q.len()),
            PlayHow::End => q.len(),
        };
        for (i, t) in tracks.into_iter().enumerate() {
            q.insert(at + i, t);
        }
        if matches!(how, PlayHow::Now | PlayHow::Replace) {
            set_from_queue(g, at);
        }
    }
    s.toast(crate::backend::played(how, item_title), false);
}

fn set_members(s: &mut AppState, gid: &str, wanted: &[String]) {
    let Some(hi) = household_of(s, gid) else { return };
    let h = &mut s.households[hi];
    let coord_volume = h.groups.iter().find(|g| g.id == gid).map(|g| g.volume).unwrap_or(20);
    // Take every wanted room out of its current group.
    for m in wanted.iter().filter(|m| m.as_str() != gid) {
        let mut orphaned: Vec<String> = vec![];
        for g in h.groups.iter_mut() {
            if g.id != gid && g.members.contains(m) {
                g.members.retain(|x| x != m);
                if g.coordinator == *m {
                    orphaned = g.members.clone();
                    g.members.clear();
                }
            }
        }
        h.groups.retain(|g| !g.members.is_empty());
        if let Some(first) = orphaned.first().cloned() {
            h.groups.push(Group { id: first.clone(), coordinator: first, members: orphaned, volume: coord_volume, queue: Some(vec![]), ..Default::default() });
        }
    }
    let mut released = vec![];
    if let Some(g) = h.groups.iter_mut().find(|g| g.id == gid) {
        for m in g.members.iter().filter(|m| !wanted.contains(m) && m.as_str() != gid) {
            released.push(m.clone());
        }
        g.members.retain(|m| wanted.contains(m) || m == gid);
        for m in wanted {
            if !g.members.contains(m) {
                g.members.push(m.clone());
            }
        }
    }
    for m in released {
        let v = h.room(&m).map(|r| r.volume).unwrap_or(20);
        h.groups.push(Group { id: m.clone(), coordinator: m.clone(), members: vec![m], volume: v, queue: Some(vec![]), ..Default::default() });
    }
    let order: Vec<String> = h.rooms.iter().map(|r| r.uuid.clone()).collect();
    h.groups.sort_by_key(|g| order.iter().position(|o| *o == g.coordinator).unwrap_or(99));
    let vols: Vec<(String, u8)> = h.rooms.iter().map(|r| (r.uuid.clone(), r.volume)).collect();
    for g in h.groups.iter_mut() {
        let sum: u32 = g.members.iter().filter_map(|m| vols.iter().find(|v| &v.0 == m)).map(|v| v.1 as u32).sum();
        g.volume = (sum / g.members.len().max(1) as u32) as u8;
    }
    let _ = scale_volume;
}

pub fn tick(s: &mut AppState) {
    let mut sleepers = vec![];
    for h in s.households.iter_mut() {
        for g in h.groups.iter_mut() {
            if let Some(left) = g.sleep_left.as_mut() {
                *left = left.saturating_sub(1);
                if *left == 0 {
                    sleepers.push(g.id.clone());
                }
            }
            if g.now.transport != Transport::Playing || g.now.is_stream || g.now.duration <= 0.0 {
                continue;
            }
            if g.now.live_position() + 0.05 >= g.now.duration {
                let len = g.queue.as_ref().map(|q| q.len()).unwrap_or(0);
                let cur = g.now.track_no.saturating_sub(1) as usize;
                if g.repeat == Repeat::One {
                    set_from_queue(g, cur);
                } else if cur + 1 < len {
                    set_from_queue(g, cur + 1);
                } else if g.repeat == Repeat::All && len > 0 {
                    set_from_queue(g, 0);
                } else {
                    g.now.transport = Transport::Stopped;
                    g.now.position = 0.0;
                }
            }
        }
    }
    for id in sleepers {
        if let Some(g) = s.group_mut(&id) {
            g.sleep_left = None;
            g.now.position = g.now.live_position();
            g.now.transport = Transport::Paused;
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn state() -> AppState {
        let mut s = AppState::default();
        load(&mut s);
        s
    }

    #[test]
    fn one_s2_and_one_s1_system() {
        let s = state();
        assert_eq!(s.households.len(), 2);
        assert_eq!(s.households[0].system, System::S2);
        assert_eq!(s.households[1].system, System::S1);
        assert_eq!(s.households[0].rooms.len(), 5);
        assert_eq!(s.households[1].rooms.len(), 3);
        assert_eq!(s.phase, Phase::Demo);
        for h in &s.households {
            for g in &h.groups {
                assert!(g.members.contains(&g.coordinator), "every group contains its lead");
            }
        }
    }

    #[test]
    fn next_and_previous_move_through_the_queue() {
        let mut s = state();
        let g = s.households[0].groups.iter().find(|g| g.queue.as_ref().is_some_and(|q| q.len() > 2)).unwrap().id.clone();
        let first = s.find_group(&g).unwrap().1.now.track_no;
        apply(&mut s, &Cmd::Next(g.clone()));
        assert_eq!(s.find_group(&g).unwrap().1.now.track_no, first + 1);
        apply(&mut s, &Cmd::PlayQueueIndex(g.clone(), 0));
        assert_eq!(s.find_group(&g).unwrap().1.now.track_no, 1);
    }

    #[test]
    fn browsing_and_tracks() {
        assert!(!browse("FV:2").is_empty());
        assert!(!browse("SQ:").is_empty());
        let albums = browse("A:ALBUM");
        assert!(!albums.is_empty());
        assert!(!tracks_for(&albums[0]).is_empty(), "an album has songs");
    }

    #[test]
    fn time_moves_on() {
        let mut s = state();
        tick(&mut s);
        assert!(s.households.iter().all(|h| h.groups.iter().all(|g| g.now.live_position() >= 0.0)));
    }
}
