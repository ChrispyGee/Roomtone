//! The computer's own media controls: the keyboard's play/pause, next and previous keys, headphone
//! buttons, and the system's *Now Playing* (macOS Control Center, the Windows media overlay,
//! Linux's MPRIS players list). They control the room that's selected in the app.
//!
//! The deciding parts (what to show, which key means which command) are plain functions with
//! tests; `Controls` is the thin layer over the operating system's own interface.

use std::time::Duration;

use crate::backend::{Backend, Cmd};
use crate::model::{NowPlaying, Transport};

/// A media key press, in the app's own terms.
#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    /// Jump this many seconds forward (positive) or back (negative).
    SeekBy(f32),
    /// Jump to this many seconds in.
    SeekTo(f32),
}

/// What a key does to the selected group, given what it's playing.
pub fn command_for(key: &Key, group: &str, now: &NowPlaying) -> Option<Cmd> {
    let g = group.to_string();
    Some(match key {
        Key::Play => Cmd::Play(g),
        Key::Pause | Key::Stop => Cmd::Pause(g),
        Key::Toggle => {
            if now.transport == Transport::Playing {
                Cmd::Pause(g)
            } else {
                Cmd::Play(g)
            }
        }
        Key::Next => Cmd::Next(g),
        Key::Previous => Cmd::Previous(g),
        // A radio station has no position to move.
        Key::SeekBy(_) | Key::SeekTo(_) if now.is_stream || now.duration <= 0.0 => return None,
        Key::SeekBy(by) => Cmd::Seek(g, (now.live_position() + by).clamp(0.0, now.duration)),
        Key::SeekTo(to) => Cmd::Seek(g, to.clamp(0.0, now.duration)),
    })
}

/// What the system's Now Playing should show.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<String>,
    pub duration: Option<f32>,
    pub state: State,
    pub position: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Playing,
    Paused,
    Stopped,
}

/// What to tell the system for a group's `now`, or `None` for "nothing is playing".
pub fn shown(now: &NowPlaying) -> Option<Shown> {
    if now.is_empty() {
        return None;
    }
    let state = match now.transport {
        Transport::Playing | Transport::Transitioning => State::Playing,
        Transport::Paused => State::Paused,
        Transport::Stopped => State::Stopped,
    };
    Some(Shown {
        title: now.title.clone(),
        artist: now.artist.clone(),
        album: if now.album.starts_with("http") { String::new() } else { now.album.clone() },
        art: now.art.clone().filter(|a| a.starts_with("http")),
        duration: (now.duration > 0.0 && !now.is_stream).then_some(now.duration),
        state,
        position: now.live_position(),
    })
}

/// Do two shown states differ enough to tell the system again? (The position moves on by itself
/// while playing, so it only counts when it jumps.)
pub fn needs_update(old: &Option<Shown>, new: &Option<Shown>) -> bool {
    match (old, new) {
        (None, None) => false,
        (Some(a), Some(b)) => {
            a.title != b.title
                || a.artist != b.artist
                || a.album != b.album
                || a.art != b.art
                || a.duration != b.duration
                || a.state != b.state
                || (a.position - b.position).abs() > if b.state == State::Playing { 4.0 } else { 0.5 }
        }
        _ => true,
    }
}

/// The operating system's side. `None` from `new` means this computer has no such interface
/// available (for example no D-Bus session on Linux): the app simply goes without.
pub struct Controls {
    inner: souvlaki::MediaControls,
    last: Option<Shown>,
    /// What was last told to the system, to move the playback position along.
    last_sent: Option<std::time::Instant>,
}

impl Controls {
    /// `hwnd` is the window's handle on Windows (the system needs it), `None` elsewhere.
    pub fn new(backend: Backend, hwnd: Option<*mut std::ffi::c_void>) -> Option<Controls> {
        let config = souvlaki::PlatformConfig { display_name: "Roomtone", dbus_name: "roomtone", hwnd };
        let mut inner = match souvlaki::MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Media keys aren't available here: {e:?}");
                return None;
            }
        };
        let attached = inner.attach(move |event| {
            use souvlaki::{MediaControlEvent as E, SeekDirection as D};
            let key = match event {
                E::Play => Key::Play,
                E::Pause => Key::Pause,
                E::Toggle => Key::Toggle,
                E::Next => Key::Next,
                E::Previous => Key::Previous,
                E::Stop => Key::Stop,
                E::Seek(D::Forward) => Key::SeekBy(10.0),
                E::Seek(D::Backward) => Key::SeekBy(-10.0),
                E::SeekBy(D::Forward, by) => Key::SeekBy(by.as_secs_f32()),
                E::SeekBy(D::Backward, by) => Key::SeekBy(-by.as_secs_f32()),
                E::SetPosition(p) => Key::SeekTo(p.0.as_secs_f32()),
                _ => return,
            };
            let (group, now) = {
                let s = backend.lock();
                let Some((_, g)) = s.selected_group.as_ref().and_then(|g| s.find_group(g)) else { return };
                (g.id.clone(), g.now.clone())
            };
            if let Some(cmd) = command_for(&key, &group, &now) {
                backend.send(cmd);
            }
        });
        if let Err(e) = attached {
            eprintln!("Media keys couldn't be attached: {e:?}");
            return None;
        }
        Some(Controls { inner, last: None, last_sent: None })
    }

    /// Tell the system what's playing in the selected room, when it has changed.
    pub fn update(&mut self, now: Option<&NowPlaying>) {
        let new = now.and_then(shown);
        let due = self.last_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(1));
        if !due || !needs_update(&self.last, &new) {
            return;
        }
        self.last_sent = Some(std::time::Instant::now());
        match &new {
            Some(s) => {
                let _ = self.inner.set_metadata(souvlaki::MediaMetadata {
                    title: Some(&s.title),
                    artist: (!s.artist.is_empty()).then_some(s.artist.as_str()),
                    album: (!s.album.is_empty()).then_some(s.album.as_str()),
                    cover_url: s.art.as_deref(),
                    duration: s.duration.map(Duration::from_secs_f32),
                });
                let progress = Some(souvlaki::MediaPosition(Duration::from_secs_f32(s.position.max(0.0))));
                let _ = self.inner.set_playback(match s.state {
                    State::Playing => souvlaki::MediaPlayback::Playing { progress },
                    State::Paused => souvlaki::MediaPlayback::Paused { progress },
                    State::Stopped => souvlaki::MediaPlayback::Stopped,
                });
            }
            None => {
                let _ = self.inner.set_playback(souvlaki::MediaPlayback::Stopped);
            }
        }
        self.last = new;
    }

    /// Stop listening to the keys (the setting was switched off).
    pub fn detach(&mut self) {
        let _ = self.inner.set_playback(souvlaki::MediaPlayback::Stopped);
        let _ = self.inner.detach();
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::time::Instant;

    fn now(transport: Transport, secs: f32, duration: f32) -> NowPlaying {
        NowPlaying { title: "Song".into(), artist: "Band".into(), album: "Album".into(), position: secs, duration, transport, stamp: Some(Instant::now()), ..Default::default() }
    }

    #[test]
    fn keys_do_what_they_say() {
        let playing = now(Transport::Playing, 30.0, 200.0);
        let paused = now(Transport::Paused, 30.0, 200.0);
        assert!(matches!(command_for(&Key::Play, "g", &paused), Some(Cmd::Play(g)) if g == "g"));
        assert!(matches!(command_for(&Key::Pause, "g", &playing), Some(Cmd::Pause(_))));
        assert!(matches!(command_for(&Key::Stop, "g", &playing), Some(Cmd::Pause(_))), "stop pauses: the queue stays");
        assert!(matches!(command_for(&Key::Toggle, "g", &playing), Some(Cmd::Pause(_))));
        assert!(matches!(command_for(&Key::Toggle, "g", &paused), Some(Cmd::Play(_))));
        assert!(matches!(command_for(&Key::Next, "g", &playing), Some(Cmd::Next(_))));
        assert!(matches!(command_for(&Key::Previous, "g", &playing), Some(Cmd::Previous(_))));
    }

    #[test]
    fn seeking_stays_inside_the_song_and_skips_radio() {
        let p = now(Transport::Paused, 30.0, 200.0);
        let Some(Cmd::Seek(_, to)) = command_for(&Key::SeekBy(-100.0), "g", &p) else { panic!("seek expected") };
        assert_eq!(to, 0.0, "not before the start");
        let Some(Cmd::Seek(_, to)) = command_for(&Key::SeekBy(1000.0), "g", &p) else { panic!("seek expected") };
        assert_eq!(to, 200.0, "not after the end");
        let Some(Cmd::Seek(_, to)) = command_for(&Key::SeekTo(95.0), "g", &p) else { panic!("seek expected") };
        assert_eq!(to, 95.0);
        let mut radio = now(Transport::Playing, 0.0, 0.0);
        radio.is_stream = true;
        assert!(command_for(&Key::SeekBy(10.0), "g", &radio).is_none(), "a station has nowhere to seek");
        assert!(command_for(&Key::Next, "g", &radio).is_some());
    }

    #[test]
    fn what_the_system_is_told() {
        assert!(shown(&NowPlaying::default()).is_none(), "nothing playing");
        let mut n = now(Transport::Playing, 12.0, 215.0);
        n.art = Some("http://192.168.1.5:1400/getaa?x=1".into());
        let s = shown(&n).unwrap();
        assert_eq!((s.title.as_str(), s.artist.as_str(), s.album.as_str(), s.state), ("Song", "Band", "Album", State::Playing));
        assert_eq!(s.duration, Some(215.0));
        assert!(s.art.is_some());
        n.art = Some("not-a-web-address".into());
        assert!(shown(&n).unwrap().art.is_none(), "only web artwork is passed on");
        n.album = "http://stream.example/x".into();
        assert!(shown(&n).unwrap().album.is_empty(), "stations that put an address in the album show none");
        let mut radio = now(Transport::Transitioning, 0.0, 0.0);
        radio.is_stream = true;
        let s = shown(&radio).unwrap();
        assert_eq!((s.state, s.duration), (State::Playing, None), "starting counts as playing; a station has no length");
        assert_eq!(shown(&now(Transport::Stopped, 0.0, 100.0)).unwrap().state, State::Stopped);
    }

    #[test]
    fn the_system_is_only_told_about_real_changes() {
        let a = shown(&now(Transport::Playing, 10.0, 200.0));
        let mut moved_on = shown(&now(Transport::Playing, 10.0, 200.0)).unwrap();
        moved_on.position = 12.0;
        assert!(!needs_update(&a, &Some(moved_on.clone())), "time passing on its own isn't a change");
        moved_on.position = 90.0;
        assert!(needs_update(&a, &Some(moved_on)), "a jump is");
        let paused = shown(&now(Transport::Paused, 10.0, 200.0));
        assert!(needs_update(&a, &paused));
        assert!(needs_update(&a, &None) && needs_update(&None, &a));
        assert!(!needs_update(&None, &None));
        let mut other = now(Transport::Playing, 10.0, 200.0);
        other.title = "Another".into();
        assert!(needs_update(&a, &shown(&other)));
    }
}
