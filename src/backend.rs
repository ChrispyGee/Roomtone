//! Background work: discovery, polling and carrying out what the person asks for,
//! so the window never waits on the network.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::demo;
use crate::model::*;
use crate::net::discovery;
use crate::net::speaker::{self, PlayHow};

#[derive(Clone, Debug)]
pub enum Cmd {
    Discover,
    /// Start with the speakers from last time, then keep looking in the background.
    QuickStart,
    AddManual(String),
    UseDemo,
    Play(String),
    Pause(String),
    Next(String),
    Previous(String),
    Seek(String, f32),
    SetPlayMode(String, bool, Repeat),
    SetCrossfade(String, bool),
    GroupVolume(String, u8),
    GroupMute(String, bool),
    RoomVolume(String, u8),
    RoomMute(String, bool),
    Browse(String),
    Search(String),
    PlayItem(String, Item, PlayHow),
    LoadQueue(String),
    PlayQueueIndex(String, usize),
    RemoveFromQueue(String, usize),
    MoveInQueue(String, usize, usize),
    ClearQueue(String),
    SaveQueue(String, String),
    /// Save to Favorites; the room decides how https addresses are played (S1 speakers need help).
    AddFavorite(Option<String>, Item),
    RemoveFavorite(String, String),
    SetGroupMembers(String, Vec<String>),
    /// Group exactly these rooms, whichever is playing leading (rooms outside are left alone).
    GroupRooms(Vec<String>),
    /// Save (or replace, by the old name) a named group; delete one by name.
    SaveGroup(Option<String>, crate::config::SavedGroup),
    DeleteGroup(String),
    LineIn(String, String),
    Tv(String, String),
    SleepTimer(String, Option<u32>),
    LoadRoomSettings(String),
    SetBass(String, i32),
    SetTreble(String, i32),
    SetLoudness(String, bool),
    SetNight(String, bool),
    SetSpeech(String, bool),
    SetStatusLight(String, bool),
    Rename(String, String),
    LoadAlarms,
    SaveAlarm(Alarm),
    DeleteAlarm(String),
    PlayItems(String, Vec<Item>, PlayHow, String),
    AddFolder(String),
    RemoveFolder(String),
    Rescan,
    AddStation(crate::config::Station),
    RemoveStation(String),
    Subscribe(crate::config::Podcast),
    Unsubscribe(String),
    SetService(String, bool),
    ConnectService(u32),
    Reconnect,
    CancelLink,
    DisconnectService(u32),
    SetAccountNumber(u32, u32),
    /// Play the same thing on a group in another system (S1 with S2), following the first.
    LinkGroups(String, String),
    /// Stop a group following (or being followed by) a group in another system.
    Unlink(String),
    /// Ask the speakers of a system (by household id) whether newer software is available.
    CheckUpdates(String),
    /// Install the offered update on every speaker of a system (asked for after a confirmation).
    InstallUpdate(String),
    /// Try a connected streaming service without playing anything.
    CheckService(u32),
    /// Switch the app's language ("auto" follows the computer).
    SetLanguage(String),
    /// Change the assistant link (MCP) settings.
    SetMcp(crate::config::McpConfig),
}

impl Cmd {
    /// Repeated commands of the same kind and target collapse into the latest one (volume drags).
    fn coalesce_key(&self) -> Option<String> {
        match self {
            Cmd::GroupVolume(g, _) => Some(format!("gv{g}")),
            Cmd::RoomVolume(r, _) => Some(format!("rv{r}")),
            Cmd::Seek(g, _) => Some(format!("sk{g}")),
            Cmd::SetBass(r, _) => Some(format!("ba{r}")),
            Cmd::SetTreble(r, _) => Some(format!("tr{r}")),
            _ => None,
        }
    }
}

/// Asks the window (if there is one) to draw again after the shared state changed.
#[derive(Clone, Default)]
pub struct Waker(Option<eframe::egui::Context>);

impl Waker {
    pub fn window(ctx: eframe::egui::Context) -> Waker {
        Waker(Some(ctx))
    }
    /// For running without a window.
    pub fn none() -> Waker {
        Waker(None)
    }
    pub fn request_repaint(&self) {
        if let Some(c) = &self.0 {
            c.request_repaint();
        }
    }
}

#[derive(Clone)]
pub struct Backend {
    pub state: Arc<Mutex<AppState>>,
    tx: Sender<(u64, Cmd)>,
    seq: Arc<AtomicU64>,
}

impl Backend {
    pub fn start(ctx: Waker, demo_only: bool) -> Backend {
        let first = AppState { config: crate::config::Config::load(), ..Default::default() };
        crate::i18n::set_language(&first.config.language);
        let state = Arc::new(Mutex::new(first));
        let (tx, rx) = channel();
        let backend = Backend { state: state.clone(), tx, seq: Arc::new(AtomicU64::new(0)) };
        {
            let b = backend.clone();
            let ctx = ctx.clone();
            thread::spawn(move || worker(b, rx, ctx));
        }
        {
            let b = backend.clone();
            thread::spawn(move || poller(b, ctx));
        }
        crate::sources::local::start_server();
        backend.send(Cmd::Rescan);
        backend.send(if demo_only { Cmd::UseDemo } else { Cmd::QuickStart });
        backend
    }

    /// Hand a command to the background worker; returns its ticket number.
    pub fn send(&self, cmd: Cmd) -> u64 {
        // Optimistic updates so sliders and buttons react at once.
        {
            let mut s = self.lock();
            optimistic(&mut s, &cmd);
        }
        let follow = {
            let s = self.lock();
            mirror(&s, &cmd)
        };
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.tx.send((seq, cmd));
        // Linked groups in the other system do the same, just after.
        for c in follow {
            let n = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut s = self.lock();
                optimistic(&mut s, &c);
            }
            let _ = self.tx.send((n, c));
        }
        seq
    }

    /// Send a command and wait (up to `timeout`) for the worker to finish it.
    /// Returns the error it ran into, if any.
    pub fn send_and_wait(&self, cmd: Cmd, timeout: Duration) -> Result<(), String> {
        let seq = self.send(cmd);
        let until = Instant::now() + timeout;
        loop {
            {
                let s = self.lock();
                if let Some((_, e)) = s.errors.iter().find(|(n, _)| *n == seq) {
                    return Err(e.clone());
                }
                if s.done_seq >= seq {
                    return Ok(());
                }
            }
            if Instant::now() > until {
                return Err(crate::t!("The speakers are taking a while to answer; try again in a moment.").to_string());
            }
            thread::sleep(Duration::from_millis(40));
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, AppState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What a following group in another system should do when its leading group gets `cmd`.
fn mirror(s: &AppState, cmd: &Cmd) -> Vec<Cmd> {
    let gid = match cmd {
        Cmd::Play(g)
        | Cmd::Pause(g)
        | Cmd::Next(g)
        | Cmd::Previous(g)
        | Cmd::Seek(g, _)
        | Cmd::SetPlayMode(g, ..)
        | Cmd::PlayItem(g, ..)
        | Cmd::PlayItems(g, ..)
        | Cmd::PlayQueueIndex(g, _)
        | Cmd::ClearQueue(g)
        | Cmd::SleepTimer(g, _) => g,
        _ => return vec![],
    };
    s.links
        .iter()
        .filter(|(lead, follow)| lead == gid && s.find_group(follow).is_some())
        .map(|(_, f)| {
            let f = f.clone();
            match cmd.clone() {
                Cmd::Play(_) => Cmd::Play(f),
                Cmd::Pause(_) => Cmd::Pause(f),
                Cmd::Next(_) => Cmd::Next(f),
                Cmd::Previous(_) => Cmd::Previous(f),
                Cmd::Seek(_, p) => Cmd::Seek(f, p),
                Cmd::SetPlayMode(_, a, r) => Cmd::SetPlayMode(f, a, r),
                Cmd::PlayItem(_, i, h) => Cmd::PlayItem(f, i, h),
                Cmd::PlayItems(_, i, h, t) => Cmd::PlayItems(f, i, h, t),
                Cmd::PlayQueueIndex(_, i) => Cmd::PlayQueueIndex(f, i),
                Cmd::ClearQueue(_) => Cmd::ClearQueue(f),
                Cmd::SleepTimer(_, m) => Cmd::SleepTimer(f, m),
                other => other,
            }
        })
        .collect()
}

/// Ask one speaker of the system for an update, and only accept one for the same generation:
/// an S1 system must never be offered S2 software here (that change can't be undone).
fn check_updates(b: &Backend, hid: &str) -> UpdateState {
    let (system, ip, demo_mode, demo_current) = {
        let s = b.lock();
        let Some(h) = s.households.iter().find(|h| h.id == hid) else { return UpdateState::Failed(crate::t!("That system isn't available right now").into()) };
        (h.system, h.rooms.first().map(|r| r.ip.clone()), s.phase == Phase::Demo, h.rooms.iter().all(|r| r.version == "16.5"))
    };
    if demo_mode {
        thread::sleep(Duration::from_millis(900));
        return match system {
            System::S2 if !demo_current => UpdateState::Available(speaker::UpdateOffer { version: "86.4-62010".into(), url: "demo://update".into(), size: 190_000_000 }),
            _ => UpdateState::UpToDate,
        };
    }
    let Some(ip) = ip.filter(|i| !i.is_empty()) else { return UpdateState::Failed(crate::t!("That system isn't available right now").into()) };
    match speaker::check_for_update(&ip) {
        Ok(None) => UpdateState::UpToDate,
        Ok(Some(offer)) if offer.system() != system => UpdateState::UpToDate,
        Ok(Some(offer)) if speaker::software_version(&ip).as_deref() == Some(offer.version.as_str()) => UpdateState::UpToDate,
        Ok(Some(offer)) => UpdateState::Available(offer),
        Err(e) => UpdateState::Failed(e),
    }
}

/// Tell every speaker of the system to install the update. They download it and restart.
fn install_update(b: &Backend, ctx: &Waker, hid: &str, offer: &speaker::UpdateOffer) -> UpdateState {
    let (system, rooms, demo_mode) = {
        let s = b.lock();
        let Some(h) = s.households.iter().find(|h| h.id == hid) else { return UpdateState::Failed(crate::t!("That system isn't available right now").into()) };
        (h.system, h.rooms.iter().map(|r| (r.uuid.clone(), r.ip.clone(), r.name.clone())).collect::<Vec<_>>(), s.phase == Phase::Demo)
    };
    // The same guard again, right before anything changes.
    if offer.system() != system {
        return UpdateState::Failed(crate::t!("That update is for the other kind of system, so it wasn't installed.").into());
    }
    let total = rooms.len();
    let mut failed = vec![];
    for (i, (uuid, ip, name)) in rooms.iter().enumerate() {
        b.lock().updates.insert(hid.to_string(), UpdateState::Installing(i, total));
        ctx.request_repaint();
        if demo_mode {
            thread::sleep(Duration::from_millis(500));
            if let Some(r) = b.lock().room_mut(uuid) {
                r.version = "16.5".into();
            }
        } else if let Err(e) = speaker::begin_update(ip, &offer.url) {
            failed.push(format!("{name} ({e})"));
        }
        thread::sleep(Duration::from_millis(300));
    }
    if failed.len() == total {
        return UpdateState::Failed(failed.join("; "));
    }
    let mut s = b.lock();
    if !failed.is_empty() {
        s.toast(crate::t!("Some speakers didn't take the update: {rooms}", rooms = failed.join("; ")), true);
    }
    s.toast(crate::t!("Update started. Speakers download it and restart one by one; they're back in about 10 minutes."), false);
    if demo_mode {
        UpdateState::UpToDate
    } else {
        UpdateState::Started(offer.version.clone())
    }
}

/// Start on the following group whatever the leading group plays now.
fn sync_link(b: &Backend, lead: &str, follow: &str) {
    let demo_mode = b.lock().phase == Phase::Demo;
    if !demo_mode {
        let _ = load_queue(b, lead);
    }
    let (now, queue) = {
        let s = b.lock();
        match s.find_group(lead) {
            Some((_, g)) => (g.now.clone(), g.queue.clone().unwrap_or_default()),
            None => return,
        }
    };
    if now.is_empty() {
        return;
    }
    let paused = now.transport != Transport::Playing;
    if now.is_stream || queue.is_empty() {
        if let Some(item) = now.as_item() {
            b.send(Cmd::PlayItem(follow.to_string(), item, PlayHow::Now));
        }
    } else {
        let at = (now.track_no.max(1) - 1) as usize;
        b.send(Cmd::PlayItems(follow.to_string(), queue, PlayHow::Replace, now.title.clone()));
        b.send(Cmd::PlayQueueIndex(follow.to_string(), at));
        // Starting takes a moment; aim a little ahead.
        b.send(Cmd::Seek(follow.to_string(), now.live_position() + 1.0));
    }
    if paused {
        b.send(Cmd::Pause(follow.to_string()));
    }
}

fn optimistic(s: &mut AppState, cmd: &Cmd) {
    match cmd {
        Cmd::GroupVolume(g, v) => {
            let ids = s.find_group(g).map(|(_, g)| g.members.clone()).unwrap_or_default();
            let old = s.find_group(g).map(|(_, g)| g.volume).unwrap_or(0);
            if let Some(gr) = s.group_mut(g) {
                gr.volume = *v;
            }
            // Members follow proportionally, as the speakers do.
            for id in ids {
                if let Some(r) = s.room_mut(&id) {
                    r.volume = scale_volume(r.volume, old, *v);
                }
            }
        }
        Cmd::GroupMute(g, m) => {
            if let Some(gr) = s.group_mut(g) {
                gr.muted = *m;
            }
        }
        Cmd::RoomVolume(r, v) => {
            if let Some(room) = s.room_mut(r) {
                room.volume = *v;
            }
            recompute_group_volume(s, r);
        }
        Cmd::RoomMute(r, m) => {
            if let Some(room) = s.room_mut(r) {
                room.muted = *m;
            }
        }
        Cmd::Play(g) => set_transport(s, g, Transport::Playing),
        Cmd::Pause(g) => set_transport(s, g, Transport::Paused),
        Cmd::Seek(g, pos) => {
            if let Some(gr) = s.group_mut(g) {
                gr.now.position = *pos;
                gr.now.stamp = Some(Instant::now());
            }
        }
        Cmd::SetPlayMode(g, sh, rp) => {
            if let Some(gr) = s.group_mut(g) {
                gr.shuffle = *sh;
                gr.repeat = *rp;
            }
        }
        Cmd::SetCrossfade(g, on) => {
            if let Some(gr) = s.group_mut(g) {
                gr.crossfade = *on;
            }
        }
        Cmd::Browse(id) => {
            s.browse.entry(id.clone()).or_insert(Browse::Loading);
        }
        Cmd::Search(t) => {
            for src in crate::sources::SEARCH_SOURCES {
                s.browse.insert(format!("search:{src}:{t}"), Browse::Loading);
            }
        }
        Cmd::PlayItem(_, item, _) => {
            s.recent.retain(|r| r.uri != item.uri || r.title != item.title);
            s.recent.insert(0, item.clone());
            s.recent.truncate(12);
        }
        Cmd::SetBass(r, v) => edit_settings(s, r, |st| st.bass = *v),
        Cmd::SetTreble(r, v) => edit_settings(s, r, |st| st.treble = *v),
        Cmd::SetLoudness(r, v) => edit_settings(s, r, |st| st.loudness = *v),
        Cmd::SetNight(r, v) => edit_settings(s, r, |st| st.night = Some(*v)),
        Cmd::SetSpeech(r, v) => edit_settings(s, r, |st| st.speech = Some(*v)),
        Cmd::SetStatusLight(r, v) => edit_settings(s, r, |st| st.status_light = *v),
        Cmd::SleepTimer(g, m) => {
            if let Some(gr) = s.group_mut(g) {
                gr.sleep_left = m.map(|m| m * 60);
            }
        }
        _ => {}
    }
}

fn edit_settings(s: &mut AppState, uuid: &str, f: impl FnOnce(&mut RoomSettings)) {
    if let Some(r) = s.room_mut(uuid) {
        f(r.settings.get_or_insert_with(RoomSettings::default));
    }
}

fn set_transport(s: &mut AppState, gid: &str, t: Transport) {
    if let Some(g) = s.group_mut(gid) {
        g.now.position = g.now.live_position();
        g.now.stamp = Some(Instant::now());
        g.now.transport = t;
    }
}

pub fn scale_volume(room: u8, old_group: u8, new_group: u8) -> u8 {
    if old_group == 0 {
        return new_group;
    }
    let v = room as f32 * new_group as f32 / old_group as f32;
    v.round().clamp(0.0, 100.0) as u8
}

fn recompute_group_volume(s: &mut AppState, room: &str) {
    for h in s.households.iter_mut() {
        let vols: HashMap<String, u8> = h.rooms.iter().map(|r| (r.uuid.clone(), r.volume)).collect();
        for g in h.groups.iter_mut().filter(|g| g.members.iter().any(|m| m == room)) {
            let sum: u32 = g.members.iter().filter_map(|m| vols.get(m)).map(|v| *v as u32).sum();
            g.volume = (sum / g.members.len().max(1) as u32) as u8;
        }
    }
}

fn worker(b: Backend, rx: Receiver<(u64, Cmd)>, ctx: Waker) {
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while let Ok(c) = rx.try_recv() {
            batch.push(c);
        }
        let mut seen = HashSet::new();
        let mut keep = vec![];
        let last_seq = batch.last().map(|(n, _)| *n).unwrap_or(0);
        for (seq, cmd) in batch.into_iter().rev() {
            if let Some(k) = cmd.coalesce_key() {
                if !seen.insert(k) {
                    continue;
                }
            }
            keep.push((seq, cmd));
        }
        keep.reverse();
        for (seq, cmd) in keep {
            let failed = |b: &Backend, e: String| {
                let mut s = b.lock();
                s.errors.push((seq, e.clone()));
                if s.errors.len() > 32 {
                    s.errors.remove(0);
                }
                s.toast(e, true);
            };
            let cmd = match run_common(&b, &ctx, cmd) {
                Ok(Some(c)) => c,
                Ok(None) => {
                    b.lock().done_seq = seq;
                    ctx.request_repaint();
                    continue;
                }
                Err(e) => {
                    failed(&b, e);
                    b.lock().done_seq = seq;
                    ctx.request_repaint();
                    continue;
                }
            };
            let demo_mode = matches!(b.lock().phase, Phase::Demo);
            if demo_mode && !matches!(cmd, Cmd::Discover | Cmd::QuickStart | Cmd::AddManual(_)) {
                let mut s = b.lock();
                demo::apply(&mut s, &cmd);
            } else if let Err(e) = run_live(&b, &ctx, cmd) {
                let lost = looks_like_lost_contact(&e);
                failed(&b, if lost { crate::t!("Lost contact with your speakers. Reconnecting…").to_string() } else { e });
                if lost {
                    let (b2, ctx2) = (b.clone(), ctx.clone());
                    thread::spawn(move || reconnect(&b2, &ctx2, false));
                }
            }
            b.lock().done_seq = seq;
            ctx.request_repaint();
        }
        // Commands folded into a later one count as done too.
        let mut s = b.lock();
        s.done_seq = s.done_seq.max(last_seq);
    }
}

/// The service number in an address such as `x-sonos-spotify:…?sid=9&…`.
fn service_of(uri: &str) -> Option<u32> {
    let rest = uri.split("sid=").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn with_account_number(uri: &str, sn: u32) -> String {
    match uri.find("sn=") {
        Some(at) => {
            let end = uri[at + 3..].find(|c: char| !c.is_ascii_digit()).map(|e| at + 3 + e).unwrap_or(uri.len());
            format!("{}sn={sn}{}", &uri[..at], &uri[end..])
        }
        None => format!("{uri}&sn={sn}"),
    }
}

/// Play something from a streaming service. If the speakers refuse it, try the system's
/// other accounts for that service and keep the one that works. After starting, check
/// that it really plays and say what to try if it doesn't.
fn play_from_service(b: &Backend, ip: &str, gid: &str, item: Item, how: PlayHow, sid: u32) -> Result<(), String> {
    let (name, current) = {
        let s = b.lock();
        let a = s.config.account(sid);
        (a.map(|a| a.name.clone()).unwrap_or_default(), a.map(|a| a.sn.max(1)).unwrap_or(1))
    };
    let mut tries = vec![current];
    tries.extend((1..=6).filter(|n| *n != current));
    let mut first_error = None;
    for sn in tries {
        let mut attempt = item.clone();
        attempt.uri = with_account_number(&item.uri, sn);
        match speaker::play_item(ip, gid, &attempt, how) {
            Ok(()) => {
                if sn != current {
                    {
                        let mut s = b.lock();
                        if let Some(a) = s.config.accounts.iter_mut().find(|a| a.sid == sid) {
                            a.sn = sn;
                        }
                        s.toast(crate::t!("{service} plays with account {n} on this system; saved.", service = name, n = sn), false);
                    }
                    let cfg = b.lock().config.clone();
                    let _ = cfg.save();
                }
                if matches!(how, PlayHow::Now | PlayHow::Replace) {
                    let (b2, ip2, name2) = (b.clone(), ip.to_string(), name.clone());
                    thread::spawn(move || {
                        thread::sleep(Duration::from_secs(5));
                        if let Ok(p) = speaker::poll_group(&ip2, false) {
                            let ours = service_of(&p.now.uri).or_else(|| service_of(&p.now.media_uri)) == Some(sid);
                            if ours && p.now.transport == Transport::Stopped {
                                b2.lock().toast(crate::t!("{service} didn't start. In Music Services, try another account number (now {n}).", service = name2, n = sn), true);
                            }
                        }
                    });
                }
                return Ok(());
            }
            Err(e) => {
                if looks_like_lost_contact(&e) {
                    return Err(e);
                }
                first_error.get_or_insert(e);
            }
        }
    }
    Err(crate::t!(
        "{service} wouldn't play this ({error}). Check the account is added to your system in the speaker maker's app.",
        service = name,
        error = first_error.unwrap_or_default()
    ))
}

/// Try a connected service end to end without playing anything: home, a search and the account number.
fn check_service(b: &Backend, sid: u32) -> String {
    let (acc, ips) = {
        let s = b.lock();
        (s.config.account(sid).cloned(), s.household().map(|h| h.rooms.iter().map(|r| r.ip.clone()).collect::<Vec<_>>()).unwrap_or_default())
    };
    let Some(acc) = acc else { return crate::t!("Not connected").to_string() };
    let mut parts = vec![];
    let mut failed = false;
    match crate::sources::smapi::browse(&acc, "root") {
        Ok(v) => parts.push(crate::tn!(v.len(), "Browsing works (1 section)", "Browsing works ({n} sections)")),
        Err(e) => {
            failed = true;
            parts.push(crate::t!("Browsing failed: {error}", error = e));
        }
    }
    match crate::sources::smapi::search(&acc, "love") {
        Ok(v) => parts.push(crate::tn!(v.len(), "search works (1 result)", "search works ({n} results)")),
        Err(e) => {
            failed = true;
            parts.push(crate::t!("search failed: {error}", error = e));
        }
    }
    match ips.iter().find_map(|ip| speaker::account_number(ip, sid)) {
        Some(sn) => {
            parts.push(crate::t!("plays with account {n} (found on your system)", n = sn));
            let mut s = b.lock();
            if let Some(a) = s.config.accounts.iter_mut().find(|a| a.sid == sid) {
                a.sn = sn;
            }
        }
        None => parts.push(crate::t!("account {n} (a guess: save one of its songs or playlists to Favorites in the speaker maker's app to confirm)", n = acc.sn.max(1))),
    }
    save_renewed(b);
    let cfg = b.lock().config.clone();
    let _ = cfg.save();
    // A leading "!" tells the page to show it as a problem.
    format!("{}{}", if failed { "!" } else { "" }, parts.join(" · "))
}

/// The note shown after starting something.
pub fn played(how: PlayHow, title: &str) -> String {
    match how {
        PlayHow::Now | PlayHow::Replace => crate::t!("Playing {title}", title = title),
        PlayHow::Next => crate::t!("Playing next: {title}", title = title),
        PlayHow::End => crate::t!("Added to queue: {title}", title = title),
    }
}

fn need(o: Option<String>, what: &str) -> Result<String, String> {
    o.ok_or_else(|| crate::t!("{what} isn't available right now", what = crate::i18n::tr(what)))
}

fn run_live(b: &Backend, ctx: &Waker, cmd: Cmd) -> Result<(), String> {
    let coord = |g: &str| need(b.lock().coordinator_ip(g), n_!("That room"));
    let room_ip = |r: &str| need(b.lock().room_ip(r), n_!("That speaker"));
    match cmd {
        // Finding speakers can take a while; do it beside the worker so browsing isn't held up.
        Cmd::Discover => {
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                if let Err(e) = discover(&b2, &ctx2) {
                    b2.lock().toast(e, true);
                }
                ctx2.request_repaint();
            });
            Ok(())
        }
        Cmd::QuickStart => {
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                if let Err(e) = quick_start(&b2, &ctx2) {
                    b2.lock().toast(e, true);
                }
                ctx2.request_repaint();
            });
            Ok(())
        }
        Cmd::AddManual(ip) => {
            {
                let mut s = b.lock();
                if !s.manual_ips.contains(&ip) {
                    s.manual_ips.push(ip);
                }
            }
            discover(b, ctx)
        }
        Cmd::UseDemo => {
            let mut s = b.lock();
            demo::load(&mut s);
            Ok(())
        }
        Cmd::Play(g) => speaker::play(&coord(&g)?).and_then(|_| refresh_group(b, &g)),
        Cmd::Pause(g) => speaker::pause(&coord(&g)?).and_then(|_| refresh_group(b, &g)),
        Cmd::Next(g) => speaker::next(&coord(&g)?).and_then(|_| refresh_group(b, &g)),
        Cmd::Previous(g) => speaker::previous(&coord(&g)?).and_then(|_| refresh_group(b, &g)),
        Cmd::Seek(g, p) => speaker::seek(&coord(&g)?, p),
        Cmd::SetPlayMode(g, sh, rp) => speaker::set_play_mode(&coord(&g)?, play_mode(sh, rp)),
        Cmd::SetCrossfade(g, on) => speaker::set_crossfade(&coord(&g)?, on),
        Cmd::GroupVolume(g, v) => speaker::set_group_volume(&coord(&g)?, v),
        Cmd::GroupMute(g, m) => speaker::set_group_mute(&coord(&g)?, m),
        Cmd::RoomVolume(r, v) => speaker::set_volume(&room_ip(&r)?, v),
        Cmd::RoomMute(r, m) => speaker::set_mute(&room_ip(&r)?, m),
        Cmd::Browse(id) => {
            let ip = need(b.lock().any_ip(), n_!("Your music library"))?;
            let b2 = b.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let result = match speaker::browse(&ip, &id) {
                    Ok(mut items) => {
                        crate::sources::memory::enrich(&mut items);
                        Browse::Ready(items)
                    }
                    Err(e) => Browse::Failed(e),
                };
                b2.lock().browse.insert(id, result);
                ctx.request_repaint();
            });
            Ok(())
        }
        Cmd::Search(_) => Ok(()),
        Cmd::PlayItems(g, items, how, title) => {
            let ip = coord(&g)?;
            let items: Vec<Item> = items.into_iter().map(|i| adapt_for_system(b, &g, i)).collect();
            crate::sources::memory::remember(&items);
            speaker::enqueue_many(&ip, &g, &items, how)?;
            b.lock().toast(played(how, &title), false);
            refresh_group(b, &g)?;
            load_queue(b, &g)
        }
        Cmd::AddFolder(_)
        | Cmd::RemoveFolder(_)
        | Cmd::Rescan
        | Cmd::AddStation(_)
        | Cmd::RemoveStation(_)
        | Cmd::Subscribe(_)
        | Cmd::Unsubscribe(_)
        | Cmd::SetService(..)
        | Cmd::ConnectService(_)
        | Cmd::Reconnect
        | Cmd::GroupRooms(_)
        | Cmd::SaveGroup(..)
        | Cmd::DeleteGroup(_)
        | Cmd::CancelLink
        | Cmd::DisconnectService(_)
        | Cmd::SetAccountNumber(..)
        | Cmd::SetLanguage(_)
        | Cmd::SetMcp(_)
        | Cmd::CheckService(_)
        | Cmd::LinkGroups(..)
        | Cmd::Unlink(_)
        | Cmd::CheckUpdates(_)
        | Cmd::InstallUpdate(_) => Ok(()),
        Cmd::PlayItem(g, item, how) => {
            let ip = coord(&g)?;
            let item = adapt_for_system(b, &g, item);
            crate::sources::memory::remember(std::slice::from_ref(&item));
            match service_of(&item.uri) {
                Some(sid) if b.lock().config.account(sid).is_some() => play_from_service(b, &ip, &g, item.clone(), how, sid)?,
                _ => speaker::play_item(&ip, &g, &item, how)?,
            }
            b.lock().toast(played(how, &item.title), false);
            refresh_group(b, &g)?;
            load_queue(b, &g)
        }
        Cmd::LoadQueue(g) => load_queue(b, &g),
        Cmd::PlayQueueIndex(g, i) => {
            speaker::play_queue_index(&coord(&g)?, &g, i)?;
            refresh_group(b, &g)
        }
        Cmd::RemoveFromQueue(g, i) => {
            speaker::remove_from_queue(&coord(&g)?, i)?;
            load_queue(b, &g)
        }
        Cmd::MoveInQueue(g, from, to) => {
            speaker::move_in_queue(&coord(&g)?, from, to)?;
            load_queue(b, &g)
        }
        Cmd::ClearQueue(g) => {
            speaker::clear_queue(&coord(&g)?)?;
            load_queue(b, &g)?;
            refresh_group(b, &g)
        }
        Cmd::AddFavorite(g, item) => {
            let ip = need(b.lock().any_ip(), n_!("Favorites"))?;
            let item = match &g {
                Some(g) => adapt_for_system(b, g, item),
                None => item,
            };
            crate::sources::memory::remember(std::slice::from_ref(&item));
            speaker::add_favorite(&ip, &item)?;
            {
                let mut s = b.lock();
                s.browse.remove("FV:2");
                s.toast(crate::t!("Added “{title}” to Favorites", title = item.title), false);
            }
            Ok(())
        }
        Cmd::RemoveFavorite(id, title) => {
            let ip = need(b.lock().any_ip(), n_!("Favorites"))?;
            speaker::remove_favorite(&ip, &id)?;
            let mut s = b.lock();
            s.browse.remove("FV:2");
            s.toast(crate::t!("Removed “{title}” from Favorites", title = title), false);
            Ok(())
        }
        Cmd::SaveQueue(g, title) => {
            speaker::save_queue(&coord(&g)?, &title)?;
            b.lock().browse.remove("SQ:");
            b.lock().toast(crate::t!("Saved playlist “{title}”", title = title), false);
            Ok(())
        }
        Cmd::SetGroupMembers(g, wanted) => {
            let current = b.lock().find_group(&g).map(|(_, gr)| gr.members.clone()).unwrap_or_default();
            for m in wanted.iter().filter(|m| !current.contains(m)) {
                speaker::join(&room_ip(m)?, &g)?;
            }
            for m in current.iter().filter(|m| !wanted.contains(m) && **m != g) {
                speaker::leave(&room_ip(m)?)?;
            }
            thread::sleep(Duration::from_millis(600));
            refresh_topology(b)
        }
        Cmd::LineIn(g, src) => speaker::play_line_in(&coord(&g)?, &src).and_then(|_| refresh_group(b, &g)),
        Cmd::Tv(g, src) => speaker::play_tv(&coord(&g)?, &src).and_then(|_| refresh_group(b, &g)),
        Cmd::SleepTimer(g, m) => speaker::sleep_timer(&coord(&g)?, m),
        Cmd::LoadRoomSettings(r) => {
            let st = speaker::room_settings(&room_ip(&r)?)?;
            if let Some(room) = b.lock().room_mut(&r) {
                room.settings = Some(st);
            }
            Ok(())
        }
        Cmd::SetBass(r, v) => speaker::set_bass(&room_ip(&r)?, v),
        Cmd::SetTreble(r, v) => speaker::set_treble(&room_ip(&r)?, v),
        Cmd::SetLoudness(r, v) => speaker::set_loudness(&room_ip(&r)?, v),
        Cmd::SetNight(r, v) => speaker::set_eq(&room_ip(&r)?, "NightMode", v),
        Cmd::SetSpeech(r, v) => speaker::set_eq(&room_ip(&r)?, "DialogLevel", v),
        Cmd::SetStatusLight(r, v) => speaker::set_led(&room_ip(&r)?, v),
        Cmd::Rename(r, name) => {
            speaker::rename(&room_ip(&r)?, &name)?;
            if let Some(room) = b.lock().room_mut(&r) {
                room.name = name;
            }
            Ok(())
        }
        Cmd::LoadAlarms => {
            let list = speaker::alarms(&need(b.lock().any_ip(), n_!("Alarms"))?)?;
            b.lock().alarms = Some(list);
            Ok(())
        }
        Cmd::SaveAlarm(a) => {
            let ip = need(b.lock().any_ip(), n_!("Alarms"))?;
            speaker::save_alarm(&ip, &a)?;
            b.lock().alarms = Some(speaker::alarms(&ip)?);
            Ok(())
        }
        Cmd::DeleteAlarm(id) => {
            let ip = need(b.lock().any_ip(), n_!("Alarms"))?;
            speaker::delete_alarm(&ip, &id)?;
            b.lock().alarms = Some(speaker::alarms(&ip)?);
            Ok(())
        }
    }
}

/// Work that doesn't depend on the speakers: settings, other music sources and search.
/// Returns the command to pass on to the speakers (or demo), if any.
fn run_common(b: &Backend, ctx: &Waker, cmd: Cmd) -> Result<Option<Cmd>, String> {
    let save = |b: &Backend| {
        let cfg = b.lock().config.clone();
        if let Err(e) = cfg.save() {
            b.lock().toast(crate::t!("Couldn't save settings: {error}", error = e), true);
        }
    };
    match cmd {
        Cmd::Browse(id) if id.starts_with("ARTIST:") => {
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                let name = id.trim_start_matches("ARTIST:").to_string();
                let items = artist_everywhere(&b2, &name);
                let state = if items.is_empty() { Browse::Ready(vec![]) } else { Browse::Ready(items) };
                b2.lock().browse.insert(id, state);
                ctx2.request_repaint();
            });
            Ok(None)
        }
        Cmd::PlayItem(g, item, how) if item.id.starts_with("ARTIST:") => {
            let name = item.id.trim_start_matches("ARTIST:").to_string();
            let (cfg, demo_mode) = {
                let s = b.lock();
                (s.config.clone(), s.phase == Phase::Demo)
            };
            let mut tracks = vec![];
            for it in artist_everywhere(b, &name) {
                if demo_mode && it.uri.starts_with("demo-") && it.is_container {
                    tracks.extend(demo::tracks_for(&it));
                } else if !it.uri.is_empty() {
                    tracks.push(it);
                } else if let Ok(more) = crate::sources::tracks_for(&it, &cfg) {
                    tracks.extend(more);
                }
            }
            if tracks.is_empty() {
                return Err(crate::t!("Nothing to play by {artist}", artist = name));
            }
            Ok(Some(Cmd::PlayItems(g, tracks, how, item.title)))
        }
        Cmd::Browse(id) if id.starts_with("FVSVC:") => {
            let sid: u32 = id.trim_start_matches("FVSVC:").parse().unwrap_or(0);
            let (demo_mode, ip) = {
                let s = b.lock();
                (s.phase == Phase::Demo, s.any_ip())
            };
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                let all = if demo_mode {
                    Ok(demo::browse("FV:2"))
                } else {
                    match &ip {
                        Some(ip) => speaker::browse(ip, "FV:2"),
                        None => Ok(vec![]),
                    }
                };
                let result = match all {
                    Ok(v) => Browse::Ready(v.into_iter().filter(|i| crate::sources::from_service(i, sid)).collect()),
                    Err(e) => Browse::Failed(e),
                };
                b2.lock().browse.insert(id, result);
                ctx2.request_repaint();
            });
            Ok(None)
        }
        Cmd::Browse(id) if crate::sources::is_source_id(&id) => {
            let cfg = b.lock().config.clone();
            let b2 = b.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let result = match crate::sources::browse(&id, &cfg) {
                    Ok(items) => Browse::Ready(items),
                    Err(e) => Browse::Failed(e),
                };
                save_renewed(&b2);
                b2.lock().browse.insert(id, result);
                ctx.request_repaint();
            });
            Ok(None)
        }
        Cmd::Search(term) => {
            search_everywhere(b, ctx, term);
            Ok(None)
        }
        Cmd::PlayItem(g, item, how) if crate::sources::is_source_id(&item.id) && item.is_container && !(item.id.starts_with("SVC:") && !item.uri.is_empty()) => {
            let cfg = b.lock().config.clone();
            let tracks = crate::sources::tracks_for(&item, &cfg)?;
            if tracks.is_empty() {
                return Err(crate::t!("“{title}” has nothing playable", title = item.title));
            }
            Ok(Some(Cmd::PlayItems(g, tracks, how, item.title)))
        }
        Cmd::AddFolder(path) => {
            {
                let mut s = b.lock();
                if s.config.folders.contains(&path) {
                    return Ok(None);
                }
                s.config.folders.push(path);
                if !s.config.service_on("local") {
                    s.config.services.push("local".into());
                }
            }
            save(b);
            rescan(b, ctx);
            Ok(None)
        }
        Cmd::RemoveFolder(path) => {
            b.lock().config.folders.retain(|f| f != &path);
            save(b);
            rescan(b, ctx);
            Ok(None)
        }
        Cmd::Rescan => {
            rescan(b, ctx);
            Ok(None)
        }
        Cmd::AddStation(mut st) => {
            st.url = crate::sources::radio::resolve_stream(st.url.trim());
            if st.name.trim().is_empty() {
                st.name = st.url.trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or("Station").to_string();
            }
            {
                let mut s = b.lock();
                s.config.stations.retain(|x| x.url != st.url);
                let name = st.name.clone();
                s.config.stations.push(st);
                s.browse.remove("RADIO:MINE");
                s.toast(crate::t!("Added “{title}” to My stations", title = name), false);
            }
            save(b);
            Ok(None)
        }
        Cmd::RemoveStation(url) => {
            {
                let mut s = b.lock();
                s.config.stations.retain(|x| x.url != url);
                s.browse.remove("RADIO:MINE");
            }
            save(b);
            Ok(None)
        }
        Cmd::Subscribe(p) => {
            {
                let mut s = b.lock();
                if !s.config.podcasts.iter().any(|x| x.feed == p.feed) {
                    s.toast(crate::t!("Subscribed to “{title}”", title = p.title), false);
                    s.config.podcasts.push(p);
                }
                s.browse.remove("POD:SUBS");
            }
            save(b);
            Ok(None)
        }
        Cmd::Unsubscribe(feed) => {
            {
                let mut s = b.lock();
                s.config.podcasts.retain(|x| x.feed != feed);
                s.browse.remove("POD:SUBS");
            }
            save(b);
            Ok(None)
        }
        Cmd::ConnectService(sid) => {
            connect_service(b, ctx, sid)?;
            Ok(None)
        }
        Cmd::AddFavorite(_, item) if !item.can_favorite() => {
            if !item.can_keep_in_app() {
                return Err(crate::t!("“{title}” can't be a favorite", title = item.title));
            }
            {
                let mut s = b.lock();
                if !s.config.favorites.iter().any(|f| f.id == item.id) {
                    let mut fav = item.clone();
                    if fav.description.is_empty() {
                        fav.description = crate::t!("{kind} · kept in this app", kind = kind_label(&fav.class));
                    }
                    s.config.favorites.push(fav);
                }
                s.toast(crate::t!("Added “{title}” to Favorites", title = item.title), false);
            }
            save(b);
            Ok(None)
        }
        Cmd::RemoveFavorite(id, title) if !id.starts_with("FV:2/") => {
            {
                let mut s = b.lock();
                s.config.favorites.retain(|f| f.id != id);
                s.toast(crate::t!("Removed “{title}” from Favorites", title = title), false);
            }
            save(b);
            Ok(None)
        }
        Cmd::SaveGroup(old, group) => {
            {
                let mut s = b.lock();
                let replace = old.unwrap_or_else(|| group.name.clone());
                s.config.groups.retain(|g| g.name != replace && g.name != group.name);
                s.toast(crate::t!("Saved group “{name}”", name = group.name), false);
                s.config.groups.push(group);
                s.config.groups.sort_by_key(|g| g.name.to_lowercase());
            }
            save(b);
            Ok(None)
        }
        Cmd::DeleteGroup(name) => {
            {
                let mut s = b.lock();
                s.config.groups.retain(|g| g.name != name);
                s.toast(crate::t!("Deleted group “{name}”", name = name), false);
            }
            save(b);
            Ok(None)
        }
        Cmd::GroupRooms(rooms) => {
            // Lead with a room that already leads a group, preferring one that's playing.
            let lead = {
                let s = b.lock();
                let leads: Vec<&Group> = s.households.iter().flat_map(|h| h.groups.iter()).filter(|g| rooms.contains(&g.coordinator)).collect();
                leads.iter().find(|g| g.now.transport == Transport::Playing).or(leads.first()).map(|g| g.coordinator.clone())
            };
            let lead = match lead {
                Some(l) => l,
                None => {
                    // None of these rooms leads a group yet: take the first out of its group.
                    let first = rooms.first().cloned().ok_or(crate::t!("No rooms in that group"))?;
                    let mut out = Ok(Some(Cmd::SetGroupMembers(first.clone(), rooms.clone())));
                    let current = b.lock().households.iter().flat_map(|h| h.groups.iter()).find(|g| g.members.contains(&first)).map(|g| (g.id.clone(), g.members.clone()));
                    if let Some((gid, members)) = current {
                        let rest: Vec<String> = members.into_iter().filter(|m| m != &first).collect();
                        let demo_mode = b.lock().phase == Phase::Demo;
                        // Take the address first; the shared state mustn't stay locked while we wait.
                        let first_ip = b.lock().room_ip(&first);
                        if demo_mode {
                            let mut s = b.lock();
                            demo::apply(&mut s, &Cmd::SetGroupMembers(gid, rest));
                        } else if let Some(ip) = first_ip {
                            if let Err(e) = speaker::leave(&ip) {
                                out = Err(e);
                            } else {
                                thread::sleep(Duration::from_millis(600));
                                let _ = refresh_topology(b);
                            }
                        }
                    }
                    return out;
                }
            };
            Ok(Some(Cmd::SetGroupMembers(lead, rooms)))
        }
        Cmd::Reconnect => {
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || reconnect(&b2, &ctx2, true));
            Ok(None)
        }
        Cmd::CancelLink => {
            b.lock().link = None;
            Ok(None)
        }
        Cmd::DisconnectService(sid) => {
            {
                let mut s = b.lock();
                s.config.accounts.retain(|a| a.sid != sid);
                s.browse.retain(|k, _| !k.starts_with(&format!("SVC:{sid}/")) && !k.starts_with(&format!("search:svc:{sid}:")));
            }
            save(b);
            Ok(None)
        }
        Cmd::SetAccountNumber(sid, sn) => {
            {
                let mut s = b.lock();
                if let Some(a) = s.config.accounts.iter_mut().find(|a| a.sid == sid) {
                    a.sn = sn.max(1);
                }
                s.browse.retain(|k, _| !k.starts_with(&format!("SVC:{sid}/")) && !k.starts_with(&format!("search:svc:{sid}:")));
            }
            save(b);
            Ok(None)
        }
        Cmd::CheckUpdates(hid) => {
            {
                let mut s = b.lock();
                // Not while an update is being sent out.
                if matches!(s.updates.get(&hid), Some(UpdateState::Installing(..))) {
                    return Ok(None);
                }
                s.updates.insert(hid.clone(), UpdateState::Checking);
            }
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                let state = check_updates(&b2, &hid);
                b2.lock().updates.insert(hid, state);
                ctx2.request_repaint();
            });
            Ok(None)
        }
        Cmd::InstallUpdate(hid) => {
            let offer = match b.lock().updates.get(&hid) {
                Some(UpdateState::Available(o)) => o.clone(),
                _ => return Err(crate::t!("Check for updates first.").into()),
            };
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                let state = install_update(&b2, &ctx2, &hid, &offer);
                b2.lock().updates.insert(hid, state);
                ctx2.request_repaint();
            });
            Ok(None)
        }
        Cmd::LinkGroups(lead, follow) => {
            {
                let mut s = b.lock();
                let home = |g: &str| s.find_group(g).map(|(h, _)| h.id.clone());
                let (hl, hf) = (home(&lead), home(&follow));
                if hl.is_none() || hf.is_none() {
                    return Err(crate::t!("That room isn't available right now").into());
                }
                if hl == hf {
                    return Err(crate::t!("Those rooms are in the same system; group them instead.").into());
                }
                // One link per group, and no chains.
                s.links.retain(|(l, f)| l != &lead && f != &lead && l != &follow && f != &follow);
                s.links.push((lead.clone(), follow.clone()));
                let names = (s.find_group(&lead).map(|(h, g)| h.group_name(g)).unwrap_or_default(), s.find_group(&follow).map(|(h, g)| h.group_name(g)).unwrap_or_default());
                s.toast(crate::t!("{room} now plays along with {lead}", room = names.1, lead = names.0), false);
            }
            let (b2, l2, f2) = (b.clone(), lead.clone(), follow.clone());
            thread::spawn(move || sync_link(&b2, &l2, &f2));
            Ok(None)
        }
        Cmd::Unlink(gid) => {
            b.lock().links.retain(|(l, f)| l != &gid && f != &gid);
            Ok(None)
        }
        Cmd::CheckService(sid) => {
            b.lock().service_checks.insert(sid, String::new());
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || {
                let result = check_service(&b2, sid);
                b2.lock().service_checks.insert(sid, result);
                ctx2.request_repaint();
            });
            Ok(None)
        }
        Cmd::SetLanguage(code) => {
            crate::i18n::set_language(&code);
            b.lock().config.language = code;
            save(b);
            Ok(None)
        }
        Cmd::SetMcp(m) => {
            b.lock().config.mcp = m;
            save(b);
            Ok(None)
        }
        Cmd::SetService(id, on) => {
            {
                let mut s = b.lock();
                s.config.services.retain(|x| x != &id);
                if on {
                    s.config.services.push(id);
                }
            }
            save(b);
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

/// Services sometimes renew a sign-in mid-request; keep the new one.
fn save_renewed(b: &Backend) {
    let renewed = crate::sources::smapi::take_renewed();
    if renewed.is_empty() {
        return;
    }
    let cfg = {
        let mut s = b.lock();
        for (sid, t, k) in renewed {
            if let Some(a) = s.config.accounts.iter_mut().find(|a| a.sid == sid) {
                a.token = t;
                a.key = k;
            }
        }
        s.config.clone()
    };
    let _ = cfg.save();
}

/// Sign in to a streaming service: show its web page and code, then wait for approval.
fn connect_service(b: &Backend, ctx: &Waker, sid: u32) -> Result<(), String> {
    let (svc, ip, household) = {
        let s = b.lock();
        if s.phase != Phase::Live {
            return Err(crate::t!("Connecting a service needs your speakers on the network").into());
        }
        let svc = s.music_services.iter().find(|m| m.sid == sid).cloned().ok_or(crate::t!("Your speakers don't offer that service"))?;
        let h = s.household().ok_or("No system")?;
        let ip = h.rooms.first().map(|r| r.ip.clone()).ok_or("No speaker")?;
        (svc, ip, h.id.clone())
    };
    b.lock().link = Some(LinkState {
        sid,
        name: svc.name.clone(),
        url: String::new(),
        code: String::new(),
        status: crate::t!("Asking the service for a sign-in page…").into(),
        done: false,
        failed: false,
    });
    ctx.request_repaint();
    let device_id = speaker::device_id(&ip).unwrap_or_else(|| format!("Roomtone-{:x}", crate::sources::hash(&household)));
    let link = match crate::sources::smapi::start_link(&svc, &household, &device_id) {
        Ok(l) => l,
        Err(e) => {
            if let Some(l) = b.lock().link.as_mut() {
                l.status = crate::t!("{service} couldn't start a sign-in: {error}", service = svc.name, error = e);
                l.failed = true;
            }
            return Ok(());
        }
    };
    if let Some(l) = b.lock().link.as_mut() {
        l.url = link.url.clone();
        l.code = link.code.clone();
        l.status = crate::t!("Waiting for you to approve on the service's page…").into();
    }
    ctx.request_repaint();
    let (b2, ctx2) = (b.clone(), ctx.clone());
    thread::spawn(move || {
        for _ in 0..72 {
            thread::sleep(Duration::from_secs(5));
            if b2.lock().link.as_ref().is_none_or(|l| l.sid != sid) {
                return;
            }
            match crate::sources::smapi::finish_link(&svc, &household, &device_id, &link) {
                Ok(None) => continue,
                Ok(Some((token, key))) => {
                    let ips: Vec<String> = b2.lock().household().map(|h| h.rooms.iter().map(|r| r.ip.clone()).collect()).unwrap_or_else(|| vec![ip.clone()]);
                    let sn = ips.iter().find_map(|i| speaker::account_number(i, sid)).unwrap_or(1);
                    let account = crate::config::Account {
                        sid,
                        name: svc.name.clone(),
                        uri: svc.uri.clone(),
                        manifest: svc.manifest.clone(),
                        household: household.clone(),
                        device_id: device_id.clone(),
                        token,
                        key,
                        sn,
                    };
                    let cfg = {
                        let mut s = b2.lock();
                        s.config.accounts.retain(|a| a.sid != sid);
                        s.config.accounts.push(account);
                        if let Some(l) = s.link.as_mut() {
                            l.status = crate::t!("{service} is connected. Find it in Music Services and in Search.", service = svc.name);
                            l.done = true;
                        }
                        s.toast(crate::t!("{service} connected", service = svc.name), false);
                        s.config.clone()
                    };
                    let _ = cfg.save();
                    ctx2.request_repaint();
                    return;
                }
                Err(e) => {
                    if let Some(l) = b2.lock().link.as_mut() {
                        l.status = crate::t!("Sign-in didn't complete: {error}", error = e);
                        l.failed = true;
                    }
                    ctx2.request_repaint();
                    return;
                }
            }
        }
        if let Some(l) = b2.lock().link.as_mut() {
            l.status = crate::t!("Sign-in timed out. Try again when you're ready.").into();
            l.failed = true;
        }
        ctx2.request_repaint();
    });
    Ok(())
}

fn rescan(b: &Backend, ctx: &Waker) {
    let folders = b.lock().config.folders.clone();
    b.lock().local_status = if folders.is_empty() { String::new() } else { crate::t!("Scanning your folders…").into() };
    let b2 = b.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let b3 = b2.clone();
        let ctx2 = ctx.clone();
        crate::sources::local::rescan(&folders, move |msg| {
            b3.lock().local_status = msg;
            ctx2.request_repaint();
        });
        let mut s = b2.lock();
        s.browse.retain(|k, _| !k.starts_with("LOCAL:") && !k.starts_with("search:local:"));
        drop(s);
        ctx.request_repaint();
    });
}

/// Search every switched-on source at once; each fills in its own results as it finishes.
fn search_everywhere(b: &Backend, ctx: &Waker, term: String) {
    let (cfg, demo, ip) = {
        let s = b.lock();
        (s.config.clone(), s.phase == Phase::Demo, s.any_ip())
    };
    for src in crate::sources::all_search_sources(&cfg) {
        let src = src.as_str();
        let key = format!("search:{src}:{term}");
        if !crate::sources::enabled(&cfg, src) {
            b.lock().browse.insert(key, Browse::Ready(vec![]));
            continue;
        }
        let (b2, ctx2, cfg2, ip2, term2) = (b.clone(), ctx.clone(), cfg.clone(), ip.clone(), term.clone());
        let src = src.to_string();
        thread::spawn(move || {
            let lower = term2.to_lowercase();
            let speaker_list = |list: &str| -> Result<Vec<Item>, String> {
                if demo {
                    Ok(demo::browse(list))
                } else {
                    match &ip2 {
                        Some(ip) => speaker::browse(ip, list),
                        None => Ok(vec![]),
                    }
                }
            };
            let result: Result<Vec<Item>, String> = match src.as_str() {
                "favorites" => speaker_list("FV:2").map(|v| {
                    v.into_iter()
                        .chain(cfg2.favorites.iter().cloned())
                        .filter(|i| {
                            let service = crate::sources::service_of_item(i).map(|s| s.name.to_lowercase()).unwrap_or_default();
                            [i.title.to_lowercase(), i.artist.to_lowercase(), i.album.to_lowercase(), i.description.to_lowercase(), service].iter().any(|f| f.contains(&lower))
                        })
                        .map(|mut i| {
                            // Say which service it's from, so Apple Music and the like are easy to spot.
                            if let Some(svc) = crate::sources::service_of_item(&i) {
                                if !i.description.to_lowercase().contains(&svc.name.to_lowercase()) {
                                    let kind = kind_label(&i.class);
                                    i.description = format!("{} · {}", svc.name, if i.description.is_empty() { kind.to_string() } else { i.description.clone() });
                                }
                            }
                            i
                        })
                        .collect()
                }),
                "playlists" => speaker_list("SQ:").map(|v| v.into_iter().filter(|i| i.title.to_lowercase().contains(&lower)).collect()),
                "library" => {
                    if demo {
                        let mut s = AppState::default();
                        demo::apply(&mut s, &Cmd::Search(term2.clone()));
                        match s.browse.remove(&format!("search:{term2}")) {
                            Some(Browse::Ready(v)) => Ok(v.into_iter().filter(|i| !i.is_stream() && !i.class.contains("playlist")).collect()),
                            _ => Ok(vec![]),
                        }
                    } else {
                        let mut items = vec![];
                        if let Some(ip) = &ip2 {
                            for kind in ["A:ARTIST", "A:ALBUM", "A:TRACKS"] {
                                if let Ok(found) = speaker::browse(ip, &format!("{kind}:{term2}")) {
                                    items.extend(found.into_iter().take(50));
                                }
                            }
                        }
                        Ok(items)
                    }
                }
                other => crate::sources::search(other, &term2, &cfg2),
            };
            let state = match result {
                Ok(mut v) => {
                    for i in v.iter_mut() {
                        if i.source.is_empty() || matches!(src.as_str(), "favorites" | "playlists" | "library") {
                            i.source = src.clone();
                        }
                    }
                    Browse::Ready(v)
                }
                Err(e) => Browse::Failed(e),
            };
            save_renewed(&b2);
            b2.lock().browse.insert(format!("search:{src}:{term2}"), state);
            ctx2.request_repaint();
        });
    }
}

/// S1 speakers get https addresses relayed through this computer as plain http.
fn adapt_for_system(b: &Backend, gid: &str, mut item: Item) -> Item {
    let is_s1 = b.lock().find_group(gid).is_some_and(|(h, _)| h.system == System::S1);
    if !is_s1 || !item.uri.starts_with("https://") {
        return item;
    }
    let relayed = crate::sources::local::proxied(&item.uri);
    item.uri = if item.is_stream() { format!("x-rincon-mp3radio://{}", relayed.trim_start_matches("http://")) } else { relayed };
    item
}

/// Show last session's speakers straight away and check them at their known addresses;
/// a full network search runs in the background for new or moved speakers.
fn quick_start(b: &Backend, ctx: &Waker) -> Result<(), String> {
    let remembered = crate::known::load();
    if remembered.is_empty() {
        return discover(b, ctx);
    }
    {
        let mut s = b.lock();
        s.households = remembered;
        s.active = 0;
        s.phase = Phase::Live;
        s.ensure_selection();
    }
    ctx.request_repaint();
    let ok = try_reconnect(b);
    if ok {
        let n: usize = b.lock().households.iter().map(|h| h.rooms.len()).sum();
        b.lock().toast(crate::tn!(n, "Connected to 1 speaker", "Connected to {n} speakers"), false);
        remember_speakers(b);
        fetch_music_services(b);
        poll_everything(b);
        // Rooms' playing state is only known now; start on one that's playing.
        let mut s = b.lock();
        let selected_playing = s.selected_group.as_ref().and_then(|g| s.find_group(g)).is_some_and(|(_, g)| g.now.transport == Transport::Playing);
        if !selected_playing {
            s.selected_group = None;
            s.ensure_selection();
        }
    } else {
        let mut s = b.lock();
        for h in s.households.iter_mut() {
            for r in h.rooms.iter_mut().filter(|r| r.online.is_none()) {
                r.online = Some(false);
                r.problem = crate::t!("Not answering at its last known address").into();
            }
        }
        s.toast(crate::t!("Some speakers from last time aren't answering yet. Still looking…"), true);
    }
    ctx.request_repaint();
    let (b2, ctx2) = (b.clone(), ctx.clone());
    thread::spawn(move || background_scan(&b2, &ctx2));
    Ok(())
}

/// Everything by one artist: albums on this computer, in the system's library and on
/// connected streaming services, each marked with where it came from.
fn artist_everywhere(b: &Backend, name: &str) -> Vec<Item> {
    let (cfg, demo_mode, ip) = {
        let s = b.lock();
        (s.config.clone(), s.phase == Phase::Demo, s.any_ip())
    };
    let mut out: Vec<Item> = vec![];
    if !cfg.folders.is_empty() {
        out.extend(crate::sources::local::browse(&format!("LOCAL:ARTIST/{name}"), &cfg.folders));
    }
    let library: Vec<Item> = if demo_mode {
        demo::browse(&format!("A:ARTIST/{name}"))
    } else if let Some(ip) = &ip {
        // The system's library finds the artist by name, then lists what it has.
        speaker::browse(ip, &format!("A:ARTIST:{name}"))
            .ok()
            .and_then(|found| found.into_iter().find(|a| a.title.eq_ignore_ascii_case(name)))
            .and_then(|a| speaker::browse(ip, &a.id).ok())
            .unwrap_or_default()
    } else {
        vec![]
    };
    out.extend(library.into_iter().map(|mut i| {
        i.source = "library".into();
        i
    }));
    for acc in &cfg.accounts {
        if let Ok(items) = crate::sources::smapi::artist_content(acc, name) {
            out.extend(items);
        }
    }
    save_renewed(b);
    out
}

/// Save the speakers currently known, for a fast start next time.
fn remember_speakers(b: &Backend) {
    let (live, households) = {
        let s = b.lock();
        (s.phase == Phase::Live, s.households.clone())
    };
    if live {
        crate::known::save(&households);
    }
}

fn fetch_music_services(b: &Backend) {
    let ip = b.lock().any_ip();
    if let Some(ip) = ip {
        let b2 = b.clone();
        thread::spawn(move || {
            if let Ok(list) = speaker::music_services(&ip) {
                b2.lock().music_services = list;
            }
        });
    }
}

/// Look for speakers that are new, moved or weren't answering, without disturbing the rest.
fn background_scan(b: &Backend, ctx: &Waker) {
    if b.lock().phase != Phase::Live {
        return;
    }
    let found = discovery::search(Duration::from_millis(3000));
    let mut added = 0usize;
    let mut changed = false;
    for f in found {
        let (known_ip, needs_refresh, index) = {
            let s = b.lock();
            let known_ip = s.households.iter().any(|h| h.rooms.iter().any(|r| r.ip == f.ip && r.online == Some(true)));
            let index = f.household.as_ref().and_then(|id| s.households.iter().position(|h| &h.id == id));
            let needs_refresh = index.is_some_and(|i| s.households[i].rooms.iter().any(|r| r.online != Some(true)));
            (known_ip, needs_refresh, index)
        };
        if known_ip && !needs_refresh {
            continue;
        }
        if index.is_some() && !needs_refresh {
            continue;
        }
        let Ok(fresh) = discovery::household_from(&f.ip, f.household.clone()) else { continue };
        let mut s = b.lock();
        match s.households.iter().position(|h| h.id == fresh.id) {
            Some(i) => {
                let before = s.households[i].rooms.len();
                let merged = merge_household(&s.households[i], fresh);
                added += merged.rooms.len().saturating_sub(before);
                s.households[i] = merged;
            }
            None => {
                added += fresh.rooms.len();
                s.households.push(fresh);
                s.households.sort_by_key(|h| (h.system != System::S2, h.id.clone()));
            }
        }
        s.ensure_selection();
        changed = true;
    }
    if changed {
        if added > 0 {
            b.lock().toast(crate::tn!(added, "Found 1 more speaker", "Found {n} more speakers"), false);
        }
        if b.lock().music_services.is_empty() {
            fetch_music_services(b);
        }
        remember_speakers(b);
        poll_everything(b);
    }
    ctx.request_repaint();
}

fn discover(b: &Backend, ctx: &Waker) -> Result<(), String> {
    let (was_live, previous) = {
        let s = b.lock();
        (s.phase == Phase::Live, s.households.clone())
    };
    b.lock().phase = Phase::Searching;
    ctx.request_repaint();
    let mut found = discovery::search(Duration::from_millis(2500));
    for ip in b.lock().manual_ips.clone() {
        if !found.iter().any(|f| f.ip == ip) {
            found.push(discovery::Found { ip, household: None });
        }
    }
    let mut households: Vec<Household> = vec![];
    let mut seen_hh = HashSet::new();
    let mut seen_ips = HashSet::new();
    let mut last_err = None;
    for f in found {
        if seen_ips.contains(&f.ip) || f.household.as_ref().is_some_and(|h| seen_hh.contains(h)) {
            continue;
        }
        match discovery::household_from(&f.ip, f.household.clone()) {
            Ok(h) => {
                for r in &h.rooms {
                    seen_ips.insert(r.ip.clone());
                }
                if seen_hh.insert(h.id.clone()) {
                    households.push(h);
                }
            }
            Err(e) => last_err = Some(e),
        }
    }
    // S2 first: it's where new features live.
    households.sort_by_key(|h| (h.system != System::S2, h.id.clone()));
    if households.is_empty() && was_live && !previous.is_empty() {
        // Keep the speakers we already have rather than switching to the demo.
        let mut s = b.lock();
        s.phase = Phase::Live;
        s.toast(crate::t!("No other speakers answered the search. Keeping the ones you have."), true);
        return Ok(());
    }
    if households.is_empty() {
        let mut s = b.lock();
        demo::load(&mut s);
        s.toast(
            match last_err {
                Some(e) => crate::t!("Couldn't read your speakers ({error}). Showing a demo system.", error = e),
                None => crate::t!("No speakers found on this network. Showing a demo system.").into(),
            },
            true,
        );
        return Ok(());
    }
    let count: usize = households.iter().map(|h| h.rooms.len()).sum();
    {
        let mut s = b.lock();
        s.households = households;
        s.active = 0;
        s.phase = Phase::Live;
        s.browse.clear();
        s.alarms = None;
        s.ensure_selection();
        s.toast(crate::tn!(count, "Connected to 1 speaker", "Connected to {n} speakers"), false);
    }
    remember_speakers(b);
    fetch_music_services(b);
    poll_everything(b);
    Ok(())
}

fn merge_household(old: &Household, mut new: Household) -> Household {
    new.system = old.system;
    for r in new.rooms.iter_mut() {
        if let Some(o) = old.room(&r.uuid) {
            if r.model.is_empty() {
                r.model = o.model.clone();
            }
            if r.version.is_empty() {
                r.version = o.version.clone();
            }
            r.volume = o.volume;
            r.muted = o.muted;
            if r.online.is_none() {
                r.online = o.online;
                r.last_seen = o.last_seen;
            }
            r.settings = o.settings.clone();
        }
    }
    for g in new.groups.iter_mut() {
        if let Some(o) = old.groups.iter().find(|o| o.id == g.id) {
            let members = std::mem::take(&mut g.members);
            *g = o.clone();
            g.members = members;
        }
    }
    new
}

fn refresh_topology(b: &Backend) -> Result<(), String> {
    let targets: Vec<(usize, String, String)> = {
        let s = b.lock();
        s.households.iter().enumerate().filter_map(|(i, h)| h.rooms.first().map(|r| (i, r.ip.clone(), h.id.clone()))).collect()
    };
    for (i, ip, id) in targets {
        let new = discovery::household_from(&ip, Some(id))?;
        let mut s = b.lock();
        if let Some(old) = s.households.get(i) {
            let merged = merge_household(old, new);
            s.households[i] = merged;
        }
        s.ensure_selection();
    }
    poll_everything(b);
    Ok(())
}

fn refresh_group(b: &Backend, gid: &str) -> Result<(), String> {
    thread::sleep(Duration::from_millis(250));
    poll_one(b, gid, true);
    Ok(())
}

fn load_queue(b: &Backend, gid: &str) -> Result<(), String> {
    let ip = need(b.lock().coordinator_ip(gid), n_!("That room"))?;
    let mut q = speaker::queue(&ip)?;
    crate::sources::memory::enrich(&mut q);
    if let Some(g) = b.lock().group_mut(gid) {
        g.queue = Some(q);
    }
    Ok(())
}

fn poll_one(b: &Backend, gid: &str, full: bool) {
    let (ip, members) = {
        let s = b.lock();
        let Some(ip) = s.coordinator_ip(gid) else { return };
        let members: Vec<(String, String)> =
            s.find_group(gid).map(|(h, g)| g.members.iter().filter_map(|m| h.room(m).map(|r| (m.clone(), r.ip.clone()))).collect()).unwrap_or_default();
        (ip, members)
    };
    let poll = match speaker::poll_group(&ip, full) {
        Ok(mut p) => {
            // Songs from this computer can reach the speaker without their details; fill them in.
            let mut probe = Item {
                uri: p.now.uri.clone(),
                title: p.now.title.clone(),
                artist: p.now.artist.clone(),
                album: p.now.album.clone(),
                art: p.now.art.clone(),
                duration: p.now.duration,
                ..Default::default()
            };
            crate::sources::memory::enrich(std::slice::from_mut(&mut probe));
            if probe.title != p.now.title || p.now.artist.is_empty() && !probe.artist.is_empty() {
                p.now.title = probe.title;
                p.now.artist = probe.artist;
                p.now.album = probe.album;
                if probe.art.is_some() {
                    p.now.art = probe.art;
                }
                if p.now.duration <= 0.0 {
                    p.now.duration = probe.duration;
                }
                if p.now.is_stream && p.now.uri.starts_with("http") && p.now.duration > 0.0 {
                    p.now.is_stream = false;
                }
            }
            p
        }
        Err(e) => {
            mark(b, &ip, Err(e));
            let mut s = b.lock();
            if s.selected_group.as_deref() == Some(gid) {
                s.failures += 1;
            }
            return;
        }
    };
    mark(b, &ip, Ok(()));
    if b.lock().selected_group.as_deref() == Some(gid) {
        b.lock().failures = 0;
    }
    let mut vols = vec![];
    if full {
        for (uuid, rip) in &members {
            match speaker::volume(rip) {
                Ok(v) => {
                    mark(b, rip, Ok(()));
                    vols.push((uuid.clone(), v));
                }
                Err(e) => mark(b, rip, Err(e)),
            }
        }
    }
    let mut s = b.lock();
    let track_changed;
    if let Some(g) = s.group_mut(gid) {
        track_changed = g.now.title != poll.now.title || g.now.track_no != poll.now.track_no;
        g.now = poll.now;
        g.shuffle = poll.shuffle;
        g.repeat = poll.repeat;
        if let Some(c) = poll.crossfade {
            g.crossfade = c;
        }
        if let Some(v) = poll.volume {
            g.volume = v;
        }
        if let Some(m) = poll.muted {
            g.muted = m;
        }
        if full {
            g.sleep_left = poll.sleep_left;
        }
        if track_changed && g.queue.is_some() && full {
            // keep the queue; position highlight follows track_no
        }
    }
    for (uuid, (v, m)) in vols {
        if let Some(r) = s.room_mut(&uuid) {
            r.volume = v;
            r.muted = m;
        }
    }
}

/// Record whether the speaker at `ip` answered.
fn mark(b: &Backend, ip: &str, result: Result<(), String>) {
    let mut s = b.lock();
    for h in s.households.iter_mut() {
        for r in h.rooms.iter_mut().filter(|r| r.ip == ip) {
            match &result {
                Ok(()) => {
                    r.online = Some(true);
                    r.last_seen = Some(Instant::now());
                    r.problem.clear();
                }
                Err(e) => {
                    r.online = Some(false);
                    r.problem = e.clone();
                }
            }
        }
    }
}

fn looks_like_lost_contact(e: &str) -> bool {
    let l = e.to_lowercase();
    ["connection", "timed out", "timeout", "unreachable", "network is down", "broken pipe", "reset by peer", "no route"].iter().any(|k| l.contains(k))
}

/// Find the speakers again after sleep or a dropped connection: their last known
/// addresses first, then a fresh search in case they moved. Never switches to the demo.
pub fn reconnect(b: &Backend, ctx: &Waker, asked: bool) {
    {
        let mut s = b.lock();
        if s.reconnecting || s.phase != Phase::Live {
            return;
        }
        s.reconnecting = true;
    }
    ctx.request_repaint();
    crate::sources::local::refresh_address();
    let attempts = if asked { 3 } else { 8 };
    let mut ok = false;
    for attempt in 0..attempts {
        if try_reconnect(b) {
            ok = true;
            break;
        }
        ctx.request_repaint();
        thread::sleep(Duration::from_secs((2 + attempt as u64 * 2).min(10)));
        crate::sources::local::refresh_address();
    }
    {
        let mut s = b.lock();
        s.reconnecting = false;
        s.failures = 0;
        if ok {
            s.toast(crate::t!("Reconnected to your speakers"), false);
            crate::known::save(&s.households);
        } else {
            for h in s.households.iter_mut() {
                for r in h.rooms.iter_mut() {
                    r.online = Some(false);
                    if r.problem.is_empty() {
                        r.problem = crate::t!("Not answering").into();
                    }
                }
            }
            s.toast(crate::t!("Your speakers aren't answering. Check they're on and on the same network, then choose Reconnect."), true);
        }
    }
    if ok {
        poll_everything(b);
        let gid = b.lock().selected_group.clone();
        if let Some(g) = gid {
            let _ = load_queue(b, &g);
        }
    }
    ctx.request_repaint();
}

fn try_reconnect(b: &Backend) -> bool {
    let targets: Vec<(usize, String, Vec<String>)> = {
        let s = b.lock();
        s.households.iter().enumerate().map(|(i, h)| (i, h.id.clone(), h.rooms.iter().map(|r| r.ip.clone()).collect())).collect()
    };
    let mut all_ok = true;
    let mut searched: Option<Vec<discovery::Found>> = None;
    for (i, id, ips) in targets {
        let mut fresh = ips.iter().find_map(|ip| discovery::household_from(ip, Some(id.clone())).ok());
        if fresh.is_none() {
            let found = searched.get_or_insert_with(|| discovery::search(Duration::from_millis(2500)));
            for f in found.iter() {
                if f.household.as_ref().is_some_and(|h| h != &id) {
                    continue;
                }
                if let Ok(h) = discovery::household_from(&f.ip, f.household.clone()) {
                    if h.id == id {
                        fresh = Some(h);
                        break;
                    }
                }
            }
        }
        match fresh {
            Some(new) => {
                let mut s = b.lock();
                if let Some(old) = s.households.get(i) {
                    let merged = merge_household(old, new);
                    s.households[i] = merged;
                }
                s.ensure_selection();
            }
            None => all_ok = false,
        }
    }
    all_ok
}

/// Ask every room for its state at once, so one slow speaker doesn't hold up the rest.
fn poll_everything(b: &Backend) {
    let gids: Vec<String> = b.lock().households.iter().flat_map(|h| h.groups.iter().map(|g| g.id.clone())).collect();
    thread::scope(|scope| {
        for g in &gids {
            scope.spawn(move || poll_one(b, g, true));
        }
    });
}

fn poller(b: Backend, ctx: Waker) {
    let mut tick: u64 = 0;
    // Wall-clock time keeps running while the computer sleeps; a big jump means it just woke.
    let mut last_wall = std::time::SystemTime::now();
    loop {
        thread::sleep(Duration::from_millis(1000));
        tick += 1;
        let now = std::time::SystemTime::now();
        let gap = now.duration_since(last_wall).unwrap_or_default();
        last_wall = now;
        let woke = gap > Duration::from_secs(15);
        let failing = b.lock().failures >= 3;
        if woke || failing {
            if woke {
                // Give the network a moment to come back after waking.
                thread::sleep(Duration::from_secs(2));
            }
            let (b2, ctx2) = (b.clone(), ctx.clone());
            thread::spawn(move || reconnect(&b2, &ctx2, false));
        }
        let (phase, selected, others) = {
            let s = b.lock();
            let selected = s.selected_group.clone();
            let others: Vec<String> = s.household().map(|h| h.groups.iter().map(|g| g.id.clone()).filter(|g| Some(g) != selected.as_ref()).collect()).unwrap_or_default();
            (s.phase, selected, others)
        };
        match phase {
            Phase::Live => {
                if let Some(g) = &selected {
                    poll_one(&b, g, tick.is_multiple_of(2));
                }
                if tick.is_multiple_of(4) {
                    for g in &others {
                        poll_one(&b, g, false);
                    }
                }
                if tick.is_multiple_of(20) {
                    let _ = refresh_topology(&b);
                }
                if tick.is_multiple_of(60) {
                    remember_speakers(&b);
                }
                if tick.is_multiple_of(120) && !b.lock().reconnecting {
                    let (b2, ctx2) = (b.clone(), ctx.clone());
                    thread::spawn(move || background_scan(&b2, &ctx2));
                }
            }
            Phase::Demo => {
                let mut s = b.lock();
                demo::tick(&mut s);
            }
            _ => {}
        }
        {
            let mut s = b.lock();
            s.toasts.retain(|t| t.at.elapsed() < Duration::from_secs(6));
        }
        ctx.request_repaint();
    }
}
