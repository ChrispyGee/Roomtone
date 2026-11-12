//! The actions this app performs on a speaker. Same calls for S1 and S2 systems.

use std::time::Instant;

use super::soap::{call, ALARM_CLOCK, AV_TRANSPORT, CONTENT_DIRECTORY, DEVICE_PROPS, GROUP_RENDERING, RENDERING};
use super::{didl, soap};
use crate::model::{parse_play_mode, Alarm, Item, NowPlaying, Repeat, RoomSettings, System, Transport};

type R<T> = Result<T, String>;

fn base(ip: &str) -> String {
    format!("http://{ip}:1400")
}

fn avt(ip: &str, action: &str, extra: &[(&str, &str)]) -> R<std::collections::HashMap<String, String>> {
    let mut args = vec![("InstanceID", "0")];
    args.extend_from_slice(extra);
    call(ip, AV_TRANSPORT, action, &args)
}

pub struct GroupPoll {
    pub now: NowPlaying,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub crossfade: Option<bool>,
    pub volume: Option<u8>,
    pub muted: Option<bool>,
    pub sleep_left: Option<u32>,
}

pub fn poll_group(ip: &str, full: bool) -> R<GroupPoll> {
    let t = avt(ip, "GetTransportInfo", &[])?;
    let transport = match t.get("CurrentTransportState").map(String::as_str) {
        Some("PLAYING") => Transport::Playing,
        Some("PAUSED_PLAYBACK") => Transport::Paused,
        Some("TRANSITIONING") => Transport::Transitioning,
        _ => Transport::Stopped,
    };
    let p = avt(ip, "GetPositionInfo", &[])?;
    let m = avt(ip, "GetMediaInfo", &[]).unwrap_or_default();
    let b = base(ip);
    let track = p.get("TrackMetaData").and_then(|x| didl::parse_one(x, &b));
    let media = m.get("CurrentURIMetaData").and_then(|x| didl::parse_one(x, &b));
    let current_uri = m.get("CurrentURI").cloned().unwrap_or_default();
    let track_uri = p.get("TrackURI").cloned().unwrap_or_default();

    let mut now = NowPlaying {
        transport,
        duration: p.get("TrackDuration").map(|d| didl::parse_time(d)).unwrap_or(0.0),
        position: p.get("RelTime").map(|d| didl::parse_time(d)).unwrap_or(0.0),
        stamp: Some(Instant::now()),
        track_no: p.get("Track").and_then(|x| x.parse().ok()).unwrap_or(0),
        ..Default::default()
    };
    if let Some(tr) = &track {
        now.title = tr.title.clone();
        now.artist = tr.artist.clone();
        now.album = tr.album.clone();
        now.art = tr.art.clone();
    }
    let uri = if current_uri.is_empty() { track_uri.clone() } else { current_uri.clone() };
    now.source = if uri.starts_with("x-rincon-queue:") {
        "Queue".into()
    } else if uri.starts_with("x-rincon-stream:") {
        "Line-In".into()
    } else if uri.starts_with("x-sonos-htastream:") {
        "TV".into()
    } else if uri.is_empty() {
        String::new()
    } else {
        media.as_ref().map(|x| x.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Radio".into())
    };
    now.is_stream = !uri.starts_with("x-rincon-queue:") && !uri.is_empty();
    now.uri = track_uri.clone();
    now.media_uri = current_uri.clone();
    now.media_meta = m.get("CurrentURIMetaData").cloned().filter(|x| x != "NOT_IMPLEMENTED").unwrap_or_default();
    now.track_meta = p.get("TrackMetaData").cloned().filter(|x| x != "NOT_IMPLEMENTED").unwrap_or_default();
    if now.is_stream && now.title.is_empty() {
        now.title = now.source.clone();
    }
    if now.art.is_none() {
        now.art = media.as_ref().and_then(|x| x.art.clone());
    }
    if now.title.starts_with("x-sonosapi") || now.title.starts_with("ZPSTR_") {
        now.title = now.source.clone();
    }
    // Stations sometimes report their internal address as the album; show the station instead.
    let looks_like_address = |s: &str| s.starts_with("x-") || s.contains("://") || s.contains("~~") || (s.contains(':') && !s.contains(' '));
    if looks_like_address(&now.album) {
        now.album = media.as_ref().map(|m| m.title.clone()).filter(|t| !t.is_empty() && !looks_like_address(t)).unwrap_or_default();
    }

    let s = avt(ip, "GetTransportSettings", &[]).unwrap_or_default();
    let (shuffle, repeat) = parse_play_mode(s.get("PlayMode").map(String::as_str).unwrap_or("NORMAL"));
    let mut out = GroupPoll { now, shuffle, repeat, crossfade: None, volume: None, muted: None, sleep_left: None };
    if full {
        out.crossfade = avt(ip, "GetCrossfadeMode", &[]).ok().and_then(|c| c.get("CrossfadeMode").map(|v| v == "1"));
        out.volume = call(ip, GROUP_RENDERING, "GetGroupVolume", &[("InstanceID", "0")]).ok().and_then(|v| v.get("CurrentVolume").and_then(|x| x.parse().ok()));
        out.muted = call(ip, GROUP_RENDERING, "GetGroupMute", &[("InstanceID", "0")]).ok().and_then(|v| v.get("CurrentMute").map(|x| x == "1"));
        out.sleep_left = avt(ip, "GetRemainingSleepTimerDuration", &[])
            .ok()
            .and_then(|v| v.get("RemainingSleepTimerDuration").cloned())
            .filter(|v| !v.is_empty())
            .map(|v| didl::parse_time(&v) as u32);
    }
    Ok(out)
}

pub fn volume(ip: &str) -> R<(u8, bool)> {
    let v = call(ip, RENDERING, "GetVolume", &[("InstanceID", "0"), ("Channel", "Master")])?;
    let m = call(ip, RENDERING, "GetMute", &[("InstanceID", "0"), ("Channel", "Master")])?;
    Ok((v.get("CurrentVolume").and_then(|x| x.parse().ok()).unwrap_or(0), m.get("CurrentMute").is_some_and(|x| x == "1")))
}

pub fn play(ip: &str) -> R<()> {
    avt(ip, "Play", &[("Speed", "1")]).map(|_| ())
}
pub fn pause(ip: &str) -> R<()> {
    avt(ip, "Pause", &[]).or_else(|_| avt(ip, "Stop", &[])).map(|_| ())
}
pub fn next(ip: &str) -> R<()> {
    avt(ip, "Next", &[]).map(|_| ())
}
pub fn previous(ip: &str) -> R<()> {
    avt(ip, "Previous", &[]).map(|_| ())
}
pub fn seek(ip: &str, secs: f32) -> R<()> {
    avt(ip, "Seek", &[("Unit", "REL_TIME"), ("Target", &didl::format_hms(secs))]).map(|_| ())
}
pub fn set_play_mode(ip: &str, mode: &str) -> R<()> {
    avt(ip, "SetPlayMode", &[("NewPlayMode", mode)]).map(|_| ())
}
pub fn set_crossfade(ip: &str, on: bool) -> R<()> {
    avt(ip, "SetCrossfadeMode", &[("CrossfadeMode", if on { "1" } else { "0" })]).map(|_| ())
}
pub fn set_group_volume(ip: &str, v: u8) -> R<()> {
    let _ = call(ip, GROUP_RENDERING, "SnapshotGroupVolume", &[("InstanceID", "0")]);
    call(ip, GROUP_RENDERING, "SetGroupVolume", &[("InstanceID", "0"), ("DesiredVolume", &v.to_string())]).map(|_| ())
}
pub fn set_group_mute(ip: &str, m: bool) -> R<()> {
    call(ip, GROUP_RENDERING, "SetGroupMute", &[("InstanceID", "0"), ("DesiredMute", if m { "1" } else { "0" })]).map(|_| ())
}
pub fn set_volume(ip: &str, v: u8) -> R<()> {
    call(ip, RENDERING, "SetVolume", &[("InstanceID", "0"), ("Channel", "Master"), ("DesiredVolume", &v.to_string())]).map(|_| ())
}
pub fn set_mute(ip: &str, m: bool) -> R<()> {
    call(ip, RENDERING, "SetMute", &[("InstanceID", "0"), ("Channel", "Master"), ("DesiredMute", if m { "1" } else { "0" })]).map(|_| ())
}

pub fn browse(ip: &str, object_id: &str) -> R<Vec<Item>> {
    let b = base(ip);
    let mut items = vec![];
    let mut start = 0usize;
    loop {
        let reply = call(
            ip,
            CONTENT_DIRECTORY,
            "Browse",
            &[
                ("ObjectID", object_id),
                ("BrowseFlag", "BrowseDirectChildren"),
                ("Filter", "*"),
                ("StartingIndex", &start.to_string()),
                ("RequestedCount", "200"),
                ("SortCriteria", ""),
            ],
        )?;
        let page = didl::parse(reply.get("Result").map(String::as_str).unwrap_or(""), &b);
        let returned: usize = reply.get("NumberReturned").and_then(|x| x.parse().ok()).unwrap_or(page.len());
        let total: usize = reply.get("TotalMatches").and_then(|x| x.parse().ok()).unwrap_or(0);
        items.extend(page);
        start += returned;
        if returned == 0 || start >= total || start >= 1000 {
            break;
        }
    }
    Ok(items)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayHow {
    Now,
    Next,
    End,
    Replace,
}

fn queue_uri(coordinator: &str) -> String {
    format!("x-rincon-queue:{coordinator}#0")
}

pub fn play_item(ip: &str, coordinator: &str, item: &Item, how: PlayHow) -> R<()> {
    let meta = didl::metadata_for(item);
    if item.is_stream() {
        avt(ip, "SetAVTransportURI", &[("CurrentURI", &item.uri), ("CurrentURIMetaData", &meta)])?;
        return play(ip);
    }
    if item.uri.is_empty() {
        return Err(format!("“{}” can't be played directly", item.title));
    }
    let current_track: u32 = avt(ip, "GetPositionInfo", &[]).ok().and_then(|p| p.get("Track").and_then(|x| x.parse().ok())).unwrap_or(0);
    if how == PlayHow::Replace {
        avt(ip, "RemoveAllTracksFromQueue", &[])?;
    }
    let (position, as_next) = match how {
        PlayHow::Now | PlayHow::Next => ((current_track + 1).to_string(), "1"),
        PlayHow::End | PlayHow::Replace => ("0".to_string(), "0"),
    };
    let reply =
        avt(ip, "AddURIToQueue", &[("EnqueuedURI", &item.uri), ("EnqueuedURIMetaData", &meta), ("DesiredFirstTrackNumberEnqueued", &position), ("EnqueueAsNext", as_next)])?;
    if matches!(how, PlayHow::Now | PlayHow::Replace) {
        let first: u32 = reply.get("FirstTrackNumberEnqueued").and_then(|x| x.parse().ok()).unwrap_or(1);
        avt(ip, "SetAVTransportURI", &[("CurrentURI", &queue_uri(coordinator)), ("CurrentURIMetaData", "")])?;
        avt(ip, "Seek", &[("Unit", "TRACK_NR"), ("Target", &first.max(1).to_string())])?;
        play(ip)?;
    }
    Ok(())
}

/// Put several songs from another source into the queue in one go.
pub fn enqueue_many(ip: &str, coordinator: &str, items: &[Item], how: PlayHow) -> R<()> {
    if items.is_empty() {
        return Err("Nothing to play".into());
    }
    if items.len() == 1 && items[0].is_stream() {
        return play_item(ip, coordinator, &items[0], PlayHow::Now);
    }
    let current_track: u32 = avt(ip, "GetPositionInfo", &[]).ok().and_then(|p| p.get("Track").and_then(|x| x.parse().ok())).unwrap_or(0);
    if how == PlayHow::Replace {
        avt(ip, "RemoveAllTracksFromQueue", &[])?;
    }
    let mut position = match how {
        PlayHow::Now | PlayHow::Next => current_track + 1,
        PlayHow::End | PlayHow::Replace => 0,
    };
    let mut first: Option<u32> = None;
    for item in items.iter().filter(|i| !i.uri.is_empty() && !i.is_stream()) {
        let meta = didl::metadata_for(item);
        let reply = avt(
            ip,
            "AddURIToQueue",
            &[
                ("EnqueuedURI", &item.uri),
                ("EnqueuedURIMetaData", &meta),
                ("DesiredFirstTrackNumberEnqueued", &position.to_string()),
                ("EnqueueAsNext", if position > 0 { "1" } else { "0" }),
            ],
        )?;
        let n: u32 = reply.get("FirstTrackNumberEnqueued").and_then(|x| x.parse().ok()).unwrap_or(1);
        first.get_or_insert(n);
        if position > 0 {
            position += 1;
        }
    }
    if matches!(how, PlayHow::Now | PlayHow::Replace) {
        avt(ip, "SetAVTransportURI", &[("CurrentURI", &queue_uri(coordinator)), ("CurrentURIMetaData", "")])?;
        avt(ip, "Seek", &[("Unit", "TRACK_NR"), ("Target", &first.unwrap_or(1).max(1).to_string())])?;
        play(ip)?;
    }
    Ok(())
}

pub fn queue(ip: &str) -> R<Vec<Item>> {
    browse(ip, "Q:0")
}

pub fn play_queue_index(ip: &str, coordinator: &str, index: usize) -> R<()> {
    avt(ip, "SetAVTransportURI", &[("CurrentURI", &queue_uri(coordinator)), ("CurrentURIMetaData", "")])?;
    avt(ip, "Seek", &[("Unit", "TRACK_NR"), ("Target", &(index + 1).to_string())])?;
    play(ip)
}

pub fn remove_from_queue(ip: &str, index: usize) -> R<()> {
    avt(ip, "RemoveTrackFromQueue", &[("ObjectID", &format!("Q:0/{}", index + 1)), ("UpdateID", "0")]).map(|_| ())
}

pub fn move_in_queue(ip: &str, from: usize, to: usize) -> R<()> {
    let insert_before = if to > from { to + 2 } else { to + 1 };
    avt(ip, "ReorderTracksInQueue", &[("StartingIndex", &(from + 1).to_string()), ("NumberOfTracks", "1"), ("InsertBefore", &insert_before.to_string()), ("UpdateID", "0")])
        .map(|_| ())
}

pub fn clear_queue(ip: &str) -> R<()> {
    avt(ip, "RemoveAllTracksFromQueue", &[]).map(|_| ())
}

/// Save an item to the system's Favorites (kept on the speakers, shared with every controller).
pub fn add_favorite(ip: &str, item: &Item) -> R<()> {
    use super::soap::escape;
    if item.uri.is_empty() {
        return Err(format!("“{}” can't be a favorite", item.title));
    }
    let scheme = item.uri.split(':').next().unwrap_or("http");
    let protocol = if item.uri.starts_with("http") { "http-get:*:*:*".to_string() } else { format!("{scheme}:*:*:*") };
    let description = if item.description.is_empty() { item.subtitle() } else { item.description.clone() };
    let art = item.art.as_ref().filter(|a| a.starts_with("http")).map(|a| format!("<upnp:albumArtURI>{}</upnp:albumArtURI>", escape(a))).unwrap_or_default();
    let elements = format!(
        "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" \
xmlns:r=\"urn:schemas-rinconnetworks-com:metadata-1-0/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\">\
<item id=\"\" parentID=\"FV:2\" restricted=\"false\"><dc:title>{}</dc:title><upnp:class>object.itemobject.item.sonos-favorite</upnp:class>\
<r:ordinal>0</r:ordinal><res protocolInfo=\"{protocol}\">{}</res>{art}<r:type>instantPlay</r:type><r:description>{}</r:description>\
<r:resMD>{}</r:resMD></item></DIDL-Lite>",
        escape(&item.title),
        escape(&item.uri),
        escape(&description),
        escape(&didl::metadata_for(item))
    );
    call(ip, CONTENT_DIRECTORY, "CreateObject", &[("ContainerID", "FV:2"), ("Elements", &elements)]).map(|_| ())
}

pub fn remove_favorite(ip: &str, id: &str) -> R<()> {
    call(ip, CONTENT_DIRECTORY, "DestroyObject", &[("ObjectID", id)]).map(|_| ())
}

pub fn save_queue(ip: &str, title: &str) -> R<()> {
    avt(ip, "SaveQueue", &[("Title", title), ("ObjectID", "")]).map(|_| ())
}

pub fn join(member_ip: &str, coordinator: &str) -> R<()> {
    avt(member_ip, "SetAVTransportURI", &[("CurrentURI", &format!("x-rincon:{coordinator}")), ("CurrentURIMetaData", "")]).map(|_| ())
}

pub fn leave(member_ip: &str) -> R<()> {
    avt(member_ip, "BecomeCoordinatorOfStandaloneGroup", &[]).map(|_| ())
}

pub fn play_line_in(ip: &str, source_uuid: &str) -> R<()> {
    avt(ip, "SetAVTransportURI", &[("CurrentURI", &format!("x-rincon-stream:{source_uuid}")), ("CurrentURIMetaData", "")])?;
    play(ip)
}

pub fn play_tv(ip: &str, source_uuid: &str) -> R<()> {
    avt(ip, "SetAVTransportURI", &[("CurrentURI", &format!("x-sonos-htastream:{source_uuid}:spdif")), ("CurrentURIMetaData", "")])?;
    play(ip)
}

pub fn sleep_timer(ip: &str, minutes: Option<u32>) -> R<()> {
    let value = minutes.map(|m| didl::format_hms(m as f32 * 60.0)).unwrap_or_default();
    avt(ip, "ConfigureSleepTimer", &[("NewSleepTimerDuration", &value)]).map(|_| ())
}

pub fn room_settings(ip: &str) -> R<RoomSettings> {
    let num = |action: &str, key: &str| -> i32 { call(ip, RENDERING, action, &[("InstanceID", "0")]).ok().and_then(|m| m.get(key).and_then(|x| x.parse().ok())).unwrap_or(0) };
    let eq = |kind: &str| -> Option<bool> { call(ip, RENDERING, "GetEQ", &[("InstanceID", "0"), ("EQType", kind)]).ok().and_then(|m| m.get("CurrentValue").map(|x| x == "1")) };
    let loudness = call(ip, RENDERING, "GetLoudness", &[("InstanceID", "0"), ("Channel", "Master")]).ok().and_then(|m| m.get("CurrentLoudness").map(|x| x == "1")).unwrap_or(false);
    let led = call(ip, DEVICE_PROPS, "GetLEDState", &[]).ok().and_then(|m| m.get("CurrentLEDState").map(|x| x == "On")).unwrap_or(true);
    Ok(RoomSettings {
        bass: num("GetBass", "CurrentBass"),
        treble: num("GetTreble", "CurrentTreble"),
        loudness,
        night: eq("NightMode"),
        speech: eq("DialogLevel"),
        status_light: led,
    })
}

pub fn set_bass(ip: &str, v: i32) -> R<()> {
    call(ip, RENDERING, "SetBass", &[("InstanceID", "0"), ("DesiredBass", &v.to_string())]).map(|_| ())
}
pub fn set_treble(ip: &str, v: i32) -> R<()> {
    call(ip, RENDERING, "SetTreble", &[("InstanceID", "0"), ("DesiredTreble", &v.to_string())]).map(|_| ())
}
pub fn set_loudness(ip: &str, on: bool) -> R<()> {
    call(ip, RENDERING, "SetLoudness", &[("InstanceID", "0"), ("Channel", "Master"), ("DesiredLoudness", if on { "1" } else { "0" })]).map(|_| ())
}
pub fn set_eq(ip: &str, kind: &str, on: bool) -> R<()> {
    call(ip, RENDERING, "SetEQ", &[("InstanceID", "0"), ("EQType", kind), ("DesiredValue", if on { "1" } else { "0" })]).map(|_| ())
}
pub fn set_led(ip: &str, on: bool) -> R<()> {
    call(ip, DEVICE_PROPS, "SetLEDState", &[("DesiredLEDState", if on { "On" } else { "Off" })]).map(|_| ())
}
pub fn rename(ip: &str, name: &str) -> R<()> {
    let cur = call(ip, DEVICE_PROPS, "GetZoneAttributes", &[])?;
    call(
        ip,
        DEVICE_PROPS,
        "SetZoneAttributes",
        &[
            ("DesiredZoneName", name),
            ("DesiredIcon", cur.get("CurrentIcon").map(String::as_str).unwrap_or("")),
            ("DesiredConfiguration", cur.get("CurrentConfiguration").map(String::as_str).unwrap_or("")),
        ],
    )
    .map(|_| ())
}

pub fn alarms(ip: &str) -> R<Vec<Alarm>> {
    let reply = call(ip, ALARM_CLOCK, "ListAlarms", &[])?;
    let list = reply.get("CurrentAlarmList").cloned().unwrap_or_default();
    let doc = roxmltree::Document::parse(&list).map_err(|e| format!("alarms: {e}"))?;
    let b = base(ip);
    let mut out = vec![];
    for a in doc.descendants().filter(|n| n.tag_name().name() == "Alarm") {
        let attr = |k: &str| a.attribute(k).unwrap_or_default().to_string();
        let start = attr("StartTime");
        let mut hm = start.split(':').map(|x| x.parse::<u32>().unwrap_or(0));
        let meta = attr("ProgramMetaData");
        let uri = attr("ProgramURI");
        let name = if uri.starts_with("x-rincon-buzzer") { "Chime".to_string() } else { didl::parse_one(&meta, &b).map(|i| i.title).unwrap_or_else(|| "Music".into()) };
        out.push(Alarm {
            id: attr("ID"),
            hour: hm.next().unwrap_or(7),
            minute: hm.next().unwrap_or(0),
            duration_min: (didl::parse_time(&attr("Duration")) / 60.0) as u32,
            recurrence: attr("Recurrence"),
            enabled: attr("Enabled") == "1",
            room_uuid: attr("RoomUUID"),
            program_uri: uri,
            program_meta: meta,
            program_name: name,
            play_mode: attr("PlayMode"),
            volume: attr("Volume").parse().unwrap_or(20),
            include_linked: attr("IncludeLinkedZones") == "1",
        });
    }
    out.sort_by_key(|a| a.hour * 60 + a.minute);
    Ok(out)
}

pub fn save_alarm(ip: &str, a: &Alarm) -> R<()> {
    let start = format!("{:02}:{:02}:00", a.hour, a.minute);
    let duration = didl::format_hms(a.duration_min.max(1) as f32 * 60.0);
    let duration = format!("{:0>8}", duration);
    let uri = if a.program_uri.is_empty() { "x-rincon-buzzer:0".to_string() } else { a.program_uri.clone() };
    let volume = a.volume.to_string();
    let mode = if a.play_mode.is_empty() { "NORMAL".to_string() } else { a.play_mode.clone() };
    let mut args: Vec<(&str, &str)> = vec![];
    if !a.id.is_empty() {
        args.push(("ID", &a.id));
    }
    args.extend_from_slice(&[
        ("StartLocalTime", start.as_str()),
        ("Duration", duration.as_str()),
        ("Recurrence", a.recurrence.as_str()),
        ("Enabled", if a.enabled { "1" } else { "0" }),
        ("RoomUUID", a.room_uuid.as_str()),
        ("ProgramURI", uri.as_str()),
        ("ProgramMetaData", a.program_meta.as_str()),
        ("PlayMode", mode.as_str()),
        ("Volume", volume.as_str()),
        ("IncludeLinkedZones", if a.include_linked { "1" } else { "0" }),
    ]);
    let action = if a.id.is_empty() { "CreateAlarm" } else { "UpdateAlarm" };
    call(ip, ALARM_CLOCK, action, &args).map(|_| ())
}

pub fn delete_alarm(ip: &str, id: &str) -> R<()> {
    call(ip, ALARM_CLOCK, "DestroyAlarm", &[("ID", id)]).map(|_| ())
}

/// The streaming services this system can play (same list on S1 and S2).
pub fn music_services(ip: &str) -> R<Vec<crate::sources::smapi::MusicService>> {
    let reply = call(ip, ("/MusicServices/Control", "urn:schemas-upnp-org:service:MusicServices:1"), "ListAvailableServices", &[])?;
    let list = reply.get("AvailableServiceDescriptorList").cloned().unwrap_or_default();
    let doc = roxmltree::Document::parse(&list).map_err(|e| format!("services: {e}"))?;
    let mut out = vec![];
    for svc in doc.descendants().filter(|n| n.tag_name().name() == "Service") {
        let sid: u32 = svc.attribute("Id").and_then(|x| x.parse().ok()).unwrap_or(0);
        let auth = svc.descendants().find(|n| n.tag_name().name() == "Policy").and_then(|n| n.attribute("Auth")).unwrap_or("").to_string();
        let manifest = svc.descendants().find(|n| n.tag_name().name() == "Manifest").and_then(|n| n.attribute("Uri")).unwrap_or("").to_string();
        out.push(crate::sources::smapi::MusicService {
            sid,
            name: svc.attribute("Name").unwrap_or("").to_string(),
            uri: svc.attribute("SecureUri").or(svc.attribute("Uri")).unwrap_or("").to_string(),
            auth,
            manifest,
        });
    }
    Ok(out)
}

/// The identity services expect from a device on this system.
pub fn device_id(ip: &str) -> Option<String> {
    call(ip, ("/SystemProperties/Control", "urn:schemas-upnp-org:service:SystemProperties:1"), "GetString", &[("VariableName", "R_TrialZPSerial")])
        .ok()
        .and_then(|m| m.get("StringValue").cloned())
        .filter(|v| !v.is_empty())
}

/// Which of the speakers' accounts plays a service, guessed from saved favorites and the queue.
/// The number in `sn=` of an address from this service, if there is one.
fn sn_in(text: &str, sid: u32) -> Option<u32> {
    let needle = format!("sid={sid}&");
    let at = text.find(&needle)?;
    text[at..].split("sn=").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok())
}

/// Which of the system's accounts for a service plays its music: read from what this speaker
/// is playing now, then from favorites, the queue and saved playlists.
pub fn account_number(ip: &str, sid: u32) -> Option<u32> {
    let now = [avt(ip, "GetMediaInfo", &[]).ok().and_then(|m| m.get("CurrentURI").cloned()), avt(ip, "GetPositionInfo", &[]).ok().and_then(|p| p.get("TrackURI").cloned())];
    if let Some(sn) = now.iter().flatten().find_map(|u| sn_in(&u.replace("&amp;", "&"), sid)) {
        return Some(sn);
    }
    let needle = format!("sid={sid}&");
    for list in ["FV:2", "Q:0", "SQ:"] {
        let Ok(items) = browse(ip, list) else { continue };
        for it in items {
            let sources = [it.uri.clone(), it.meta.clone()];
            for u in sources.iter().filter(|u| u.contains(&needle)) {
                if let Some(sn) = u.split("sn=").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()) {
                    return Some(sn);
                }
            }
        }
    }
    None
}

/// A newer software version the speakers' update service offers.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateOffer {
    /// The build number, e.g. "57.19-41110" (S1) or "86.3-61160" (S2).
    pub version: String,
    pub url: String,
    pub size: u64,
}

impl UpdateOffer {
    /// Which generation the update is for: S1 builds stop at 57, S2 builds start at 58.
    pub fn system(&self) -> System {
        crate::net::discovery::system_from("", &self.version)
    }
}

/// Ask a speaker whether its system has newer software (`None`: up to date).
pub fn check_for_update(ip: &str) -> R<Option<UpdateOffer>> {
    let reply = soap::call(ip, soap::ZONE_TOPOLOGY, "CheckForUpdate", &[("UpdateType", "All"), ("CachedOnly", "0"), ("Version", "")])?;
    let xml = reply.get("UpdateItem").cloned().unwrap_or_default();
    if xml.trim().is_empty() {
        return Ok(None);
    }
    let doc = roxmltree::Document::parse(&xml).map_err(|e| format!("CheckForUpdate: unexpected reply ({e})"))?;
    let n = doc.root_element();
    let version = n.attribute("Version").unwrap_or("").to_string();
    let url = n.attribute("UpdateURL").unwrap_or("").to_string();
    if version.is_empty() || url.is_empty() {
        return Ok(None);
    }
    Ok(Some(UpdateOffer { version, url, size: n.attribute("DownloadSize").and_then(|x| x.parse().ok()).unwrap_or(0) }))
}

/// The speaker's build number ("softwareVersion" in its description).
pub fn software_version(ip: &str) -> Option<String> {
    let text = soap::get_text(&format!("http://{ip}:1400/xml/device_description.xml")).ok()?;
    let doc = roxmltree::Document::parse(&text).ok()?;
    doc.descendants().find(|n| n.tag_name().name() == "softwareVersion").and_then(|n| n.text()).map(|t| t.trim().to_string())
}

/// Tell a speaker to download and install an update. It restarts when done.
pub fn begin_update(ip: &str, url: &str) -> R<()> {
    soap::call(ip, soap::ZONE_TOPOLOGY, "BeginSoftwareUpdate", &[("UpdateURL", url), ("Flags", "0"), ("ExtraOptions", "")]).map(|_| ())
}
