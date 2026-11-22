//! The now-playing display service: what is playing, as a picture and a few words, for a picture
//! frame, a tablet, a small screen or a script on the home network.
//!
//! Off until switched on (Settings › Playback › Now-playing display). It is read-only: it can't
//! play, pause or change anything. Every address starts with a secret (`/<secret>/…`), because a
//! picture frame can't send a password; a wrong or missing secret gets a plain 404.
//!
//! * `/<secret>/` a full-screen page: the cover, and the song under it.
//! * `/<secret>/art.jpg` the cover of the song playing now as a JPEG (`?size=` 64 to 2000, default 800).
//! * `/<secret>/now.json` the song, artist, album, room and whether it's playing.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::backend::{Backend, Waker};
use crate::config::FrameConfig;
use crate::model::{AppState, Transport};
use crate::net::soap;

/// What the display shows right now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub room: String,
    pub art: Option<String>,
    /// Changes whenever the song or its cover does, so a poller knows when to fetch again.
    pub id: String,
}

/// The group the display follows: the chosen room's group, else the room being used in the
/// window, else the first group that is playing.
pub fn current(s: &AppState, cfg: &FrameConfig) -> Option<Snapshot> {
    let group_of_room = |name: &str| {
        s.households.iter().find_map(|h| {
            let uuid = h.rooms.iter().find(|r| r.name.eq_ignore_ascii_case(name.trim()))?.uuid.clone();
            h.groups.iter().find(|g| g.members.contains(&uuid)).map(|g| (h, g))
        })
    };
    let picked = if !cfg.room.trim().is_empty() {
        group_of_room(&cfg.room)
    } else {
        s.selected_group
            .as_deref()
            .and_then(|id| s.find_group(id))
            .or_else(|| s.households.iter().flat_map(|h| h.groups.iter().map(move |g| (h, g))).find(|(_, g)| g.now.transport == Transport::Playing))
    };
    let (h, g) = picked?;
    let room = g.members.iter().filter_map(|m| h.room(m)).map(|r| r.name.clone()).collect::<Vec<_>>().join(", ");
    let n = &g.now;
    let has_song = !n.title.is_empty() && !n.is_stream;
    let mut hasher = DefaultHasher::new();
    (&n.title, &n.artist, &n.art, &room).hash(&mut hasher);
    Some(Snapshot {
        playing: n.transport == Transport::Playing && !n.title.is_empty(),
        title: n.title.clone(),
        artist: n.artist.clone(),
        album: n.album.clone(),
        room,
        art: if has_song || n.art.is_some() { n.art.clone().filter(|a| a.starts_with("http")) } else { None },
        id: format!("{:x}", hasher.finish()),
    })
}

fn now_json(snap: &Option<Snapshot>) -> String {
    match snap {
        Some(n) => json!({"playing": n.playing, "title": n.title, "artist": n.artist, "album": n.album, "room": n.room, "has_art": n.art.is_some(), "id": n.id}),
        None => json!({"playing": false, "title": "", "artist": "", "album": "", "room": "", "has_art": false, "id": ""}),
    }
    .to_string()
}

/// The address a frame or browser opens, for the settings page.
pub fn address(cfg: &FrameConfig, host: &str) -> String {
    format!("http://{host}:{}/{}/", cfg.port, cfg.token)
}

/// A plain dark square, sent when there is no cover so a frame always gets a picture.
fn placeholder(size: u32) -> Vec<u8> {
    let side = size.clamp(64, 2000).min(256);
    let img = image::RgbImage::from_pixel(side, side, image::Rgb([16, 17, 22]));
    encode(&img)
}

fn encode(img: &image::RgbImage) -> Vec<u8> {
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
    let _ = img.write_with_encoder(enc);
    out
}

/// The cover at `url`, scaled down to fit `size` and encoded as JPEG.
fn fetch_cover(url: &str, size: u32) -> Option<Vec<u8>> {
    let bytes = soap::get_bytes(url).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > size || img.height() > size { img.thumbnail(size, size) } else { img };
    Some(encode(&img.to_rgb8()))
}

/// The last cover fetched, so a frame asking every few seconds doesn't make the speaker send it again.
#[derive(Default)]
struct Cache {
    key: String,
    jpeg: Vec<u8>,
}

#[derive(Clone)]
pub struct Frame {
    b: Backend,
    cache: Arc<Mutex<Cache>>,
    busy: Arc<AtomicUsize>,
}

const MAX_CONNECTIONS: usize = 16;

impl Frame {
    pub fn new(b: Backend) -> Frame {
        Frame { b, cache: Arc::new(Mutex::new(Cache::default())), busy: Arc::new(AtomicUsize::new(0)) }
    }

    fn settings(&self) -> FrameConfig {
        self.b.lock().config.frame.clone()
    }

    fn cover(&self, snap: &Option<Snapshot>, size: u32) -> (Vec<u8>, bool) {
        let Some(url) = snap.as_ref().and_then(|n| n.art.clone()) else { return (placeholder(size), false) };
        let key = format!("{url}@{size}");
        if let Ok(c) = self.cache.lock() {
            if c.key == key && !c.jpeg.is_empty() {
                return (c.jpeg.clone(), true);
            }
        }
        match fetch_cover(&url, size) {
            Some(jpeg) => {
                if let Ok(mut c) = self.cache.lock() {
                    c.key = key;
                    c.jpeg = jpeg.clone();
                }
                (jpeg, true)
            }
            None => (placeholder(size), false),
        }
    }
}

/// Keep a listener open while the display is switched on, following changes to the settings.
pub fn start(frame: Frame, waker: Waker) {
    thread::spawn(move || {
        let mut listening: Option<(TcpListener, u16)> = None;
        let mut last_status = String::new();
        loop {
            let cfg = frame.settings();
            let want = cfg.enabled.then_some(cfg.port);
            let have = listening.as_ref().map(|(_, p)| *p);
            if want != have {
                listening = None;
                let status = match want {
                    None => String::new(),
                    Some(port) => match TcpListener::bind(("0.0.0.0", port)) {
                        Ok(l) => {
                            let _ = l.set_nonblocking(true);
                            listening = Some((l, port));
                            format!("ok:{port}")
                        }
                        Err(e) => format!("err:{e}"),
                    },
                };
                if status != last_status {
                    if status.starts_with("ok:") {
                        eprintln!("Now-playing display listening on port {}", cfg.port);
                    }
                    frame.b.lock().frame_status = status.clone();
                    last_status = status;
                    waker.request_repaint();
                }
                if want.is_some() && listening.is_none() {
                    thread::sleep(Duration::from_secs(3));
                    last_status.clear();
                    continue;
                }
            }
            match &listening {
                Some((l, _)) => match l.accept() {
                    Ok((stream, _)) => {
                        if frame.busy.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                            frame.busy.fetch_sub(1, Ordering::SeqCst);
                            let mut s = stream;
                            let _ = respond(&mut s, 503, "Service Unavailable", "text/plain", b"Busy", false);
                            continue;
                        }
                        let f = frame.clone();
                        thread::spawn(move || {
                            let _ = serve(&f, stream);
                            f.busy.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(_) => thread::sleep(Duration::from_millis(50)),
                },
                None => thread::sleep(Duration::from_millis(400)),
            }
        }
    });
}

fn respond(stream: &mut TcpStream, code: u16, reason: &str, ctype: &str, body: &[u8], head_only: bool) -> std::io::Result<()> {
    let csp = if ctype.starts_with("text/html") {
        "Content-Security-Policy: default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'\r\n"
    } else {
        ""
    };
    write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\n{csp}Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn serve(f: &Frame, mut stream: TcpStream) -> std::io::Result<()> {
    let _ = stream.set_nonblocking(false);
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    for _ in 0..100 {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let cfg = f.settings();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let head_only = method == "HEAD";
    let secret = path.trim_start_matches('/').split('/').next().unwrap_or("");
    if !cfg.enabled || cfg.token.is_empty() || !same(secret, &cfg.token) {
        return respond(&mut stream, 404, "Not Found", "text/plain", b"Not found", head_only);
    }
    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, 405, "Method Not Allowed", "text/plain", b"Read only", false);
    }
    let rest = path.trim_start_matches('/').strip_prefix(secret).unwrap_or("");
    match rest {
        "" => {
            let to = if query.is_empty() { format!("/{secret}/") } else { format!("/{secret}/?{query}") };
            write!(stream, "HTTP/1.1 301 Moved Permanently\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
            stream.flush()
        }
        "/" => respond(&mut stream, 200, "OK", "text/html; charset=utf-8", PAGE.as_bytes(), head_only),
        "/now.json" => {
            let snap = current(&f.b.lock(), &cfg);
            respond(&mut stream, 200, "OK", "application/json", now_json(&snap).as_bytes(), head_only)
        }
        "/art.jpg" => {
            let size = query.split('&').find_map(|kv| kv.strip_prefix("size=")).and_then(|v| v.parse::<u32>().ok()).unwrap_or(800).clamp(64, 2000);
            let snap = current(&f.b.lock(), &cfg);
            let (jpeg, _) = f.cover(&snap, size);
            respond(&mut stream, 200, "OK", "image/jpeg", &jpeg, head_only)
        }
        _ => respond(&mut stream, 404, "Not Found", "text/plain", b"Not found", head_only),
    }
}

/// The full-screen page. Plain HTML, no outside scripts, fonts or images.
const PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="referrer" content="no-referrer">
<title>Roomtone: now playing</title>
<style>
html,body{margin:0;height:100%;background:#000;color:#e8e8ee;font-family:system-ui,-apple-system,"Segoe UI",sans-serif;overflow:hidden}
#art{position:absolute;inset:0;width:100%;height:100%;object-fit:contain;opacity:0;transition:opacity .7s}
#art.on{opacity:1}
#art.paused{opacity:.35}
#txt{position:absolute;left:0;right:0;bottom:3vh;text-align:center;padding:0 4vw;text-shadow:0 0 10px #000,0 0 3px #000}
#t{font-size:3.4vmin;font-weight:600}
#a{font-size:2.5vmin;opacity:.8;margin-top:.5vmin}
</style></head>
<body><img id="art" alt=""><div id="txt"><div id="t"></div><div id="a"></div></div>
<script>
(function(){
var q=new URLSearchParams(location.search);
var art=document.getElementById('art'),t=document.getElementById('t'),a=document.getElementById('a');
if(q.get('text')==='0'){document.getElementById('txt').style.display='none';}
if(q.get('fit')==='cover'){art.style.objectFit='cover';}
var size=parseInt(q.get('size'),10)||Math.min(2000,Math.max(64,Math.round(Math.max(screen.width,screen.height)*(window.devicePixelRatio||1))));
var last='';
function show(n){
  var on=n.playing||(n.title!==''&&n.has_art);
  t.textContent=n.playing||n.title?n.title:'';
  a.textContent=n.artist||'';
  if(!n.title){art.className='';last='';return;}
  art.className=(on?'on':'')+(n.playing?'':' paused');
  if(n.id!==last){
    last=n.id;
    var im=new Image();
    im.onload=function(){if(last===n.id){art.src=im.src;}};
    im.src='art.jpg?size='+size+'&v='+encodeURIComponent(n.id);
  }
}
function poll(){
  fetch('now.json',{cache:'no-store'}).then(function(r){return r.json();}).then(show).catch(function(){});
}
poll();setInterval(poll,2000);
})();
</script></body></html>
"#;

#[cfg(test)]
mod unit {
    use super::*;
    use crate::model::{Group, Household, Room};

    fn state() -> AppState {
        let mut s = AppState::default();
        let room = |uuid: &str, name: &str| Room {
            uuid: uuid.into(),
            name: name.into(),
            ip: String::new(),
            model: String::new(),
            version: String::new(),
            volume: 20,
            muted: false,
            settings: None,
            online: None,
            last_seen: None,
            problem: String::new(),
        };
        let (kitchen, den) = (room("K", "Kitchen"), room("D", "Den"));
        let mut a = Group { id: "gk".into(), coordinator: "K".into(), members: vec!["K".into()], ..Default::default() };
        a.now.title = "Song A".into();
        a.now.artist = "Band".into();
        a.now.art = Some("http://10.0.0.5:1400/art.jpg".into());
        a.now.transport = Transport::Paused;
        let mut b = Group { id: "gd".into(), coordinator: "D".into(), members: vec!["D".into()], ..Default::default() };
        b.now.title = "Song B".into();
        b.now.transport = Transport::Playing;
        s.households.push(Household { id: "h".into(), system: crate::model::System::S2, rooms: vec![kitchen, den], groups: vec![a, b] });
        s
    }

    #[test]
    fn it_follows_the_chosen_room_then_the_selected_one_then_whatever_plays() {
        let mut s = state();
        let mut cfg = FrameConfig { room: "kitchen".into(), ..Default::default() };
        let n = current(&s, &cfg).unwrap();
        assert_eq!((n.title.as_str(), n.room.as_str(), n.playing), ("Song A", "Kitchen", false));
        cfg.room.clear();
        s.selected_group = Some("gk".into());
        assert_eq!(current(&s, &cfg).unwrap().title, "Song A");
        s.selected_group = None;
        assert_eq!(current(&s, &cfg).unwrap().title, "Song B", "the playing group when nothing is selected");
        cfg.room = "Garage".into();
        assert!(current(&s, &cfg).is_none(), "a room that isn't there shows nothing");
    }

    #[test]
    fn the_id_changes_with_the_song_and_its_cover() {
        let mut s = state();
        let cfg = FrameConfig { room: "Kitchen".into(), ..Default::default() };
        let one = current(&s, &cfg).unwrap().id;
        assert_eq!(one, current(&s, &cfg).unwrap().id);
        s.households[0].groups[0].now.title = "Song C".into();
        assert_ne!(one, current(&s, &cfg).unwrap().id);
    }

    #[test]
    fn the_json_says_what_is_playing_and_is_empty_when_nothing_is() {
        let s = state();
        let cfg = FrameConfig { room: "Den".into(), ..Default::default() };
        let v: serde_json::Value = serde_json::from_str(&now_json(&current(&s, &cfg))).unwrap();
        assert_eq!((v["title"].as_str(), v["playing"].as_bool(), v["has_art"].as_bool()), (Some("Song B"), Some(true), Some(false)));
        let v: serde_json::Value = serde_json::from_str(&now_json(&None)).unwrap();
        assert_eq!((v["title"].as_str(), v["playing"].as_bool()), (Some(""), Some(false)));
    }

    #[test]
    fn a_placeholder_is_a_real_jpeg() {
        let jpeg = placeholder(5000);
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        assert!(image::load_from_memory(&jpeg).is_ok());
    }

    #[test]
    fn secrets_compare_by_content() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
        assert!(!same("", "x"));
    }
}

#[cfg(test)]
mod serving {
    use super::*;
    use crate::test_support::demo_backend;
    use std::io::Read;

    fn get(port: u16, path: &str, method: &str) -> (u16, String, Vec<u8>) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(s, "{method} {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut all = Vec::new();
        let _ = s.read_to_end(&mut all);
        let cut = all.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(all.len());
        let head = String::from_utf8_lossy(&all[..cut]).to_string();
        let code = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (code, head, all.get(cut + 4..).unwrap_or(&[]).to_vec())
    }

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    /// A speaker that serves one small picture, so the cover can be fetched for real.
    fn fake_speaker() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let mut png = Vec::new();
        let img = image::RgbImage::from_pixel(1200, 900, image::Rgb([200, 80, 40]));
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut stream = stream;
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 2 {
                    line.clear();
                }
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", png.len());
                let _ = stream.write_all(&png);
            }
        });
        port
    }

    #[test]
    fn the_display_is_off_until_switched_on_and_needs_the_secret() {
        let b = demo_backend();
        let port = free_port();
        let speaker = fake_speaker();
        {
            let mut s = b.lock();
            s.config.frame = FrameConfig { enabled: false, port, token: "s3cret".into(), room: String::new() };
            let g = &mut s.households[0].groups[0];
            g.now.title = "Song".into();
            g.now.artist = "Band".into();
            g.now.transport = Transport::Playing;
            g.now.art = Some(format!("http://127.0.0.1:{speaker}/cover.png"));
            let id = g.id.clone();
            s.selected_group = Some(id);
        }
        start(Frame::new(b.clone()), Waker::none());
        std::thread::sleep(Duration::from_millis(900));
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "nothing listens while it's off");

        b.lock().config.frame.enabled = true;
        let mut up = false;
        for _ in 0..40 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                up = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(up, "it listens once switched on");

        assert_eq!(get(port, "/", "GET").0, 404, "no secret");
        assert_eq!(get(port, "/wrong/now.json", "GET").0, 404, "wrong secret");
        assert_eq!(get(port, "/s3cret/now.json", "POST").0, 405, "read only");
        assert_eq!(get(port, "/s3cret", "GET").0, 301, "the page address ends in a slash");

        let (code, head, page) = get(port, "/s3cret/", "GET");
        assert_eq!(code, 200);
        assert!(head.contains("Content-Security-Policy"), "the page has a content policy");
        assert!(String::from_utf8_lossy(&page).contains("now.json"));

        let (code, _, body) = get(port, "/s3cret/now.json", "GET");
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!((code, v["title"].as_str(), v["playing"].as_bool(), v["has_art"].as_bool()), (200, Some("Song"), Some(true), Some(true)));

        let (code, head, jpeg) = get(port, "/s3cret/art.jpg?size=300", "GET");
        assert_eq!(code, 200);
        assert!(head.contains("image/jpeg"));
        let img = image::load_from_memory(&jpeg).expect("a real picture");
        assert_eq!((img.width(), img.height()), (300, 225), "scaled down to fit, keeping its shape");

        let (code, _, empty) = get(port, "/s3cret/art.jpg", "HEAD");
        assert_eq!((code, empty.len()), (200, 0), "HEAD has no body");

        b.lock().config.frame.token = "other".into();
        assert_eq!(get(port, "/s3cret/now.json", "GET").0, 404, "a new address stops the old one");
        assert_eq!(get(port, "/other/now.json", "GET").0, 200);
    }
}
