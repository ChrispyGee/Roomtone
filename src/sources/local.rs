//! Music folders on this computer (or drives mounted on it). The folders are scanned
//! for audio files, and a small web server lets the speakers fetch them, so S1 and S2
//! speakers play them like any other song.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::thread;

use lofty::prelude::*;

use crate::model::Item;

const AUDIO: &[&str] = &["mp3", "flac", "m4a", "aac", "ogg", "oga", "wav", "aif", "aiff", "wma", "opus", "alac"];
const COVERS: &[&str] = &["cover.jpg", "folder.jpg", "front.jpg", "album.jpg", "cover.png", "folder.png", "Cover.jpg", "Folder.jpg", "AlbumArtSmall.jpg"];

#[derive(Clone, Debug)]
pub struct Track {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub track_no: u32,
    pub duration: f32,
    pub ext: String,
}

#[derive(Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    pub by_id: HashMap<String, usize>,
    /// The folders this was scanned from.
    #[allow(dead_code)]
    pub roots: Vec<PathBuf>,
}

fn lib() -> &'static RwLock<Library> {
    static LIB: OnceLock<RwLock<Library>> = OnceLock::new();
    LIB.get_or_init(|| RwLock::new(Library::default()))
}

fn proxies() -> &'static std::sync::Mutex<HashMap<String, String>> {
    static P: OnceLock<std::sync::Mutex<HashMap<String, String>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// A plain-http address on this computer that relays `url`. Older S1 speakers can't
/// open many modern https addresses; this lets them play podcasts, archive recordings
/// and https radio. Only addresses the app itself registered are relayed.
pub fn proxied(url: &str) -> String {
    let Some(b) = base_url() else { return url.to_string() };
    let id = format!("{:016x}", super::hash(url));
    if let Ok(mut m) = proxies().lock() {
        m.insert(id.clone(), url.to_string());
    }
    super::memory::remember_relay(&id, url);
    format!("{b}/p/{id}")
}

fn base() -> &'static RwLock<String> {
    static BASE: OnceLock<RwLock<String>> = OnceLock::new();
    BASE.get_or_init(|| RwLock::new(String::new()))
}

fn port() -> &'static OnceLock<u16> {
    static PORT: OnceLock<u16> = OnceLock::new();
    &PORT
}

/// This app's own server address, as the speakers see it.
pub fn base_url() -> Option<String> {
    base().read().ok().map(|b| b.clone()).filter(|b| !b.is_empty())
}

/// Work out this computer's network address again (it can change after sleep).
pub fn refresh_address() {
    if let Some(p) = port().get() {
        if let Ok(mut b) = base().write() {
            *b = format!("http://{}:{p}", lan_ip());
        }
    }
}

pub fn track_count() -> usize {
    lib().read().map(|l| l.tracks.len()).unwrap_or(0)
}

/// This computer's address on the home network, as the speakers will see it.
pub fn lan_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.168.255.255:9")?;
            s.local_addr()
        })
        .or_else(|_| {
            let s = UdpSocket::bind("0.0.0.0:0")?;
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

/// Start the file server once; returns its address ("http://ip:port").
pub fn start_server() -> String {
    if let Some(b) = base_url() {
        return b;
    }
    let listener = TcpListener::bind("0.0.0.0:52880").or_else(|_| TcpListener::bind("0.0.0.0:0"));
    let Ok(listener) = listener else {
        return String::new();
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let url = format!("http://{}:{port}", lan_ip());
    let _ = self::port().set(port);
    if let Ok(mut b) = base().write() {
        *b = url.clone();
    }
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let _ = serve(stream);
            });
        }
    });
    url
}

fn hash_id(p: &Path) -> String {
    format!("{:016x}", super::hash(&p.to_string_lossy()))
}

fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 14 || out.len() > 60_000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            scan_dir(&p, depth + 1, out);
        } else if let Some(ext) = p.extension().map(|x| x.to_string_lossy().to_lowercase()) {
            if AUDIO.contains(&ext.as_str()) {
                out.push(p);
            }
        }
    }
}

fn read_track(p: &Path) -> Track {
    let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let parent = p.parent().and_then(|d| d.file_name()).map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let mut t = Track { id: hash_id(p), path: p.to_path_buf(), title: stem, artist: String::new(), album: parent, genre: String::new(), track_no: 0, duration: 0.0, ext };
    if let Ok(f) = lofty::read_from_path(p) {
        t.duration = f.properties().duration().as_secs_f32();
        if let Some(tag) = f.primary_tag().or_else(|| f.first_tag()) {
            if let Some(v) = tag.title() {
                if !v.trim().is_empty() {
                    t.title = v.trim().to_string();
                }
            }
            if let Some(v) = tag.artist() {
                t.artist = v.trim().to_string();
            }
            if let Some(v) = tag.album() {
                if !v.trim().is_empty() {
                    t.album = v.trim().to_string();
                }
            }
            if let Some(v) = tag.genre() {
                t.genre = v.trim().to_string();
            }
            t.track_no = tag.track().unwrap_or(0);
        }
    }
    if t.artist.is_empty() {
        t.artist = "Unknown artist".into();
    }
    t
}

/// Re-read every folder. Runs on a background thread; `progress` gets status lines.
pub fn rescan(folders: &[String], progress: impl Fn(String)) {
    let roots: Vec<PathBuf> = folders.iter().map(PathBuf::from).collect();
    let mut files = vec![];
    for r in &roots {
        scan_dir(r, 0, &mut files);
    }
    let total = files.len();
    let mut tracks = Vec::with_capacity(total);
    for (i, f) in files.iter().enumerate() {
        tracks.push(read_track(f));
        if i % 200 == 0 && total > 200 {
            progress(format!("Reading {} of {total} songs…", i + 1));
        }
    }
    tracks.sort_by(|a, b| {
        (a.artist.to_lowercase(), a.album.to_lowercase(), a.track_no, a.title.to_lowercase()).cmp(&(
            b.artist.to_lowercase(),
            b.album.to_lowercase(),
            b.track_no,
            b.title.to_lowercase(),
        ))
    });
    let by_id = tracks.iter().enumerate().map(|(i, t)| (t.id.clone(), i)).collect();
    if let Ok(mut l) = lib().write() {
        *l = Library { tracks, by_id, roots };
    }
    progress(if total == 0 && !folders.is_empty() {
        "No music files found in your folders".into()
    } else {
        format!("{total} songs in {} folder{}", folders.len(), if folders.len() == 1 { "" } else { "s" })
    });
}

fn album_key(artist: &str, album: &str) -> String {
    format!("{artist}\u{1f}{album}")
}

/// The file on this computer behind a song address from this app's own music server
/// (`http://…/t/<code>.<ext>`).
pub fn path_of_address(uri: &str) -> Option<PathBuf> {
    let id = uri.split_once("/t/")?.1.split(['.', '?']).next()?;
    let l = lib().read().ok()?;
    l.tracks.get(*l.by_id.get(id)?).map(|t| t.path.clone())
}

/// The song in the music folders for a file path from a playlist. Tries the exact file first,
/// then a file with the same last folder and name (so a list made on another computer, with its
/// own drive letters or home folder, still finds your copies).
pub fn item_for_path(path: &Path) -> Option<Item> {
    let l = lib().read().ok()?;
    let exact = std::fs::canonicalize(path).ok();
    if let Some(t) = l.tracks.iter().find(|t| t.path == path || exact.as_ref().is_some_and(|e| std::fs::canonicalize(&t.path).ok().as_ref() == Some(e))) {
        return Some(track_item(t));
    }
    let tail = |p: &str| -> Vec<String> {
        let parts: Vec<String> = p.split(['/', '\\']).filter(|s| !s.is_empty()).map(|s| s.to_lowercase()).collect();
        parts.into_iter().rev().take(2).collect()
    };
    let wanted = tail(&path.to_string_lossy());
    if wanted.len() < 2 {
        return None;
    }
    l.tracks.iter().find(|t| tail(&t.path.to_string_lossy()) == wanted).map(track_item)
}

/// A song in the music folders with this title (and artist, if one is given), ignoring case.
pub fn item_for_name(title: &str, artist: &str) -> Option<Item> {
    let l = lib().read().ok()?;
    let (title, artist) = (title.trim().to_lowercase(), artist.trim().to_lowercase());
    if title.is_empty() {
        return None;
    }
    let same_artist = |t: &Track| artist.is_empty() || t.artist.to_lowercase() == artist;
    l.tracks.iter().find(|t| t.title.to_lowercase() == title && same_artist(t)).map(track_item)
}

/// Details of a song from the folders, by its `/t/<code>` address part.
pub fn known_track(key: &str) -> Option<super::memory::Known> {
    let id = key.strip_prefix("/t/")?;
    let l = lib().read().ok()?;
    let t = &l.tracks[*l.by_id.get(id)?];
    let b = base_url().unwrap_or_default();
    Some(super::memory::Known {
        title: t.title.clone(),
        artist: t.artist.clone(),
        album: t.album.clone(),
        art: Some(format!("{b}/a/{}", t.id)),
        duration: t.duration,
        genre: t.genre.clone(),
        source: "local".into(),
    })
}

fn track_item(t: &Track) -> Item {
    let b = base_url().unwrap_or_default();
    Item {
        id: format!("LOCAL:T/{}", t.id),
        parent_id: format!("LOCAL:ALBUM/{}", album_key(&t.artist, &t.album)),
        title: t.title.clone(),
        artist: t.artist.clone(),
        album: t.album.clone(),
        class: "object.item.audioItem.musicTrack".into(),
        uri: format!("{b}/t/{}.{}", t.id, t.ext),
        art: Some(format!("{b}/a/{}", t.id)),
        duration: t.duration,
        source: "local".into(),
        genre: t.genre.clone(),
        track_no: t.track_no,
        ..Default::default()
    }
}

fn container(id: String, title: &str, class: &str, artist: &str, art_from: Option<&Track>) -> Item {
    let b = base_url().unwrap_or_default();
    Item {
        id,
        title: title.to_string(),
        artist: artist.to_string(),
        album: if class.contains("Album") { title.to_string() } else { String::new() },
        class: class.into(),
        is_container: true,
        art: art_from.map(|t| format!("{b}/a/{}", t.id)),
        source: "local".into(),
        genre: art_from.map(|t| t.genre.clone()).unwrap_or_default(),
        ..Default::default()
    }
}

fn albums_of<'a>(tracks: impl Iterator<Item = &'a Track>) -> Vec<Item> {
    let mut seen: BTreeMap<(String, String), &Track> = BTreeMap::new();
    for t in tracks {
        seen.entry((t.album.to_lowercase(), t.artist.clone())).or_insert(t);
    }
    seen.values().map(|t| container(format!("LOCAL:ALBUM/{}", album_key(&t.artist, &t.album)), &t.album, "object.container.album.musicAlbum", &t.artist, Some(t))).collect()
}

pub fn browse(id: &str, folders: &[String]) -> Vec<Item> {
    let Ok(l) = lib().read() else { return vec![] };
    let rest = id.trim_start_matches("LOCAL:");
    match rest {
        "ROOT" => vec![
            container("LOCAL:ARTISTS".into(), "Artists", "object.container", "", None),
            container("LOCAL:ALBUMS".into(), "Albums", "object.container", "", None),
            container("LOCAL:TRACKS".into(), "Songs", "object.container", "", None),
            container("LOCAL:GENRES".into(), "Genres", "object.container", "", None),
            container("LOCAL:FOLDERS".into(), "Folders", "object.container", "", None),
        ],
        "ARTISTS" => {
            let names: BTreeSet<&str> = l.tracks.iter().map(|t| t.artist.as_str()).collect();
            names
                .into_iter()
                .map(|n| {
                    let first = l.tracks.iter().find(|t| t.artist == n);
                    let mut c = container(format!("LOCAL:ARTIST/{n}"), n, "object.container.person.musicArtist", "", first);
                    c.art = None;
                    c
                })
                .collect()
        }
        "ALBUMS" => albums_of(l.tracks.iter()),
        "TRACKS" => l.tracks.iter().take(5000).map(track_item).collect(),
        "GENRES" => {
            let names: BTreeSet<&str> = l.tracks.iter().map(|t| t.genre.as_str()).filter(|g| !g.is_empty()).collect();
            names.into_iter().map(|g| container(format!("LOCAL:GENRE/{g}"), g, "object.container.genre.musicGenre", "", None)).collect()
        }
        "FOLDERS" => folders
            .iter()
            .map(|f| {
                let name = Path::new(f).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| f.clone());
                let mut c = container(format!("LOCAL:FOLDER/{f}"), &name, "object.container", "", None);
                c.description = format!("This computer · {f}");
                c
            })
            .collect(),
        other => {
            if let Some(name) = other.strip_prefix("ARTIST/") {
                albums_of(l.tracks.iter().filter(|t| t.artist == name))
            } else if let Some(key) = other.strip_prefix("ALBUM/") {
                let (artist, album) = key.split_once('\u{1f}').unwrap_or(("", key));
                let mut v: Vec<&Track> = l.tracks.iter().filter(|t| t.album == album && (artist.is_empty() || t.artist == artist)).collect();
                v.sort_by_key(|t| (t.track_no, t.title.to_lowercase()));
                v.into_iter().map(track_item).collect()
            } else if let Some(g) = other.strip_prefix("GENRE/") {
                albums_of(l.tracks.iter().filter(|t| t.genre == g))
            } else if let Some(dir) = other.strip_prefix("FOLDER/") {
                folder_listing(&l, Path::new(dir))
            } else if let Some(tid) = other.strip_prefix("T/") {
                l.by_id.get(tid).map(|i| vec![track_item(&l.tracks[*i])]).unwrap_or_default()
            } else {
                vec![]
            }
        }
    }
}

fn folder_listing(l: &Library, dir: &Path) -> Vec<Item> {
    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subs.sort();
        for s in subs {
            let name = s.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }
            let has_music = l.tracks.iter().any(|t| t.path.starts_with(&s));
            if has_music {
                let first = l.tracks.iter().find(|t| t.path.starts_with(&s));
                let mut c = container(format!("LOCAL:FOLDER/{}", s.to_string_lossy()), &name, "object.container", "", first);
                c.description = "Folder".into();
                out.push(c);
            }
        }
    }
    let mut files: Vec<&Track> = l.tracks.iter().filter(|t| t.path.parent() == Some(dir)).collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    out.extend(files.into_iter().map(track_item));
    out
}

pub fn search(term: &str) -> Vec<Item> {
    let Ok(l) = lib().read() else { return vec![] };
    let t = term.to_lowercase();
    let mut out = vec![];
    let artists: BTreeSet<&str> = l.tracks.iter().map(|x| x.artist.as_str()).filter(|a| a.to_lowercase().contains(&t)).collect();
    for a in artists.into_iter().take(30) {
        out.push(container(format!("LOCAL:ARTIST/{a}"), a, "object.container.person.musicArtist", "", None));
    }
    out.extend(albums_of(l.tracks.iter().filter(|x| x.album.to_lowercase().contains(&t) || x.artist.to_lowercase().contains(&t))).into_iter().take(40));
    out.extend(
        l.tracks.iter().filter(|x| x.title.to_lowercase().contains(&t) || x.artist.to_lowercase().contains(&t) || x.genre.to_lowercase().contains(&t)).take(80).map(track_item),
    );
    out
}

// ---------- the little web server ----------

fn serve(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut range: Option<(u64, Option<u64>)> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(r) = lower.strip_prefix("range: bytes=") {
            let r = r.trim();
            if let Some((a, b)) = r.split_once('-') {
                if let Ok(start) = a.parse::<u64>() {
                    range = Some((start, b.parse::<u64>().ok()));
                }
            }
        }
    }
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let head_only = method == "HEAD";
    let (kind, rest) = match path.get(1..3) {
        Some("t/") => ("t", &path[3..]),
        Some("a/") => ("a", &path[3..]),
        Some("p/") => return relay(&mut stream, &path[3..], range, head_only),
        _ => return not_found(&mut stream),
    };
    let id = rest.split('.').next().unwrap_or("").split('?').next().unwrap_or("");
    let track = lib().read().ok().and_then(|l| l.by_id.get(id).map(|i| l.tracks[*i].clone()));
    let Some(track) = track else { return not_found(&mut stream) };
    if kind == "a" {
        return match art_bytes(&track) {
            Some((bytes, mime)) => {
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: max-age=86400\r\nConnection: close\r\n\r\n", bytes.len())?;
                if !head_only {
                    stream.write_all(&bytes)?;
                }
                Ok(())
            }
            None => not_found(&mut stream),
        };
    }
    let mut file = std::fs::File::open(&track.path)?;
    let len = file.metadata()?.len();
    let mime = match track.ext.as_str() {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "m4a" | "aac" | "alac" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "aif" | "aiff" => "audio/aiff",
        "wma" => "audio/x-ms-wma",
        _ => "application/octet-stream",
    };
    let (start, end) = match range {
        Some((s, e)) if s < len => (s, e.unwrap_or(len - 1).min(len - 1)),
        _ => (0, len.saturating_sub(1)),
    };
    let count = end + 1 - start;
    if range.is_some() {
        write!(stream, "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{len}\r\n")?;
    } else {
        write!(stream, "HTTP/1.1 200 OK\r\n")?;
    }
    write!(stream, "Content-Type: {mime}\r\nContent-Length: {count}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n")?;
    if head_only {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut limited = file.take(count);
    std::io::copy(&mut limited, &mut stream)?;
    Ok(())
}

fn relay(stream: &mut TcpStream, id: &str, range: Option<(u64, Option<u64>)>, head_only: bool) -> std::io::Result<()> {
    let code = id.split('?').next().unwrap_or("");
    let url = proxies().lock().ok().and_then(|m| m.get(code).cloned()).or_else(|| super::memory::relay(code));
    let Some(url) = url else { return not_found(stream) };
    let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(8)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build();
    let mut req = agent.get(&url);
    if let Some((a, b)) = range {
        req = req.set("Range", &format!("bytes={a}-{}", b.map(|x| x.to_string()).unwrap_or_default()));
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return write!(stream, "HTTP/1.1 {code} Upstream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
        Err(_) => return not_found(stream),
    };
    write!(stream, "HTTP/1.1 {} {}\r\n", resp.status(), resp.status_text())?;
    for h in ["Content-Type", "Content-Length", "Content-Range", "Accept-Ranges", "icy-name"] {
        if let Some(v) = resp.header(h) {
            write!(stream, "{h}: {v}\r\n")?;
        }
    }
    write!(stream, "Connection: close\r\n\r\n")?;
    if head_only {
        return Ok(());
    }
    let mut body = resp.into_reader();
    std::io::copy(&mut body, stream)?;
    Ok(())
}

fn not_found(stream: &mut TcpStream) -> std::io::Result<()> {
    stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

fn art_bytes(t: &Track) -> Option<(Vec<u8>, &'static str)> {
    if let Some(dir) = t.path.parent() {
        for name in COVERS {
            let p = dir.join(name);
            if let Ok(b) = std::fs::read(&p) {
                return Some((b, if name.ends_with("png") { "image/png" } else { "image/jpeg" }));
            }
        }
    }
    let f = lofty::read_from_path(&t.path).ok()?;
    let tag = f.primary_tag().or_else(|| f.first_tag())?;
    let pic = tag.pictures().first()?;
    let data = pic.data().to_vec();
    let mime = if data.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else { "image/jpeg" };
    Some((data, mime))
}

#[cfg(test)]
mod tests {
    /// Scan a folder with one small WAV file and fetch it back the way a speaker would.
    #[test]
    fn scans_and_serves_a_folder() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-music/Test Artist");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Test Song.wav");
        let samples = 8000u32;
        let mut wav = vec![];
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + samples * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(samples * 2).to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, (samples * 2) as usize));
        std::fs::write(&file, &wav).unwrap();

        let base = super::start_server();
        let root = dir.parent().unwrap().to_string_lossy().to_string();
        super::rescan(std::slice::from_ref(&root), |m| println!("{m}"));
        assert_eq!(super::track_count(), 1);
        let songs = super::browse("LOCAL:TRACKS", &[root]);
        println!("{} -> {}", songs[0].title, songs[0].uri);
        assert!(songs[0].uri.starts_with(&base));
        let local = songs[0].uri.replace(&base, &format!("http://127.0.0.1:{}", base.rsplit(':').next().unwrap()));
        let resp = ureq::get(&local).set("Range", "bytes=0-99").call().unwrap();
        assert_eq!(resp.status(), 206);
        let mut body = vec![];
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut body).unwrap();
        assert_eq!(body.len(), 100);
        assert_eq!(&body[..4], b"RIFF");
        assert!(super::search("test").iter().any(|i| i.title == "Test Song"));
    }
}

#[cfg(test)]
mod playlist_files {
    use super::*;

    #[test]
    fn songs_are_found_by_address_and_by_path_even_from_another_computer() {
        let id = "m3u-test-0001".to_string();
        {
            let mut l = lib().write().unwrap();
            let t = Track {
                id: id.clone(),
                path: PathBuf::from("/Users/me/Music/Marlow Quartet/Morning Fog.flac"),
                title: "Morning Fog".into(),
                artist: "Marlow Quartet".into(),
                album: "Low Tide".into(),
                genre: String::new(),
                track_no: 1,
                duration: 200.0,
                ext: "flac".into(),
            };
            let at = l.tracks.len();
            l.tracks.push(t);
            l.by_id.insert(id.clone(), at);
        }
        assert_eq!(path_of_address(&format!("http://192.168.1.5:52880/t/{id}.flac")), Some(PathBuf::from("/Users/me/Music/Marlow Quartet/Morning Fog.flac")));
        assert_eq!(path_of_address("http://x/other.mp3"), None);
        // Another computer's path (a Windows drive) still finds it by the last folder and file name.
        let found = item_for_path(Path::new("D:\\Old Music\\marlow quartet\\MORNING FOG.flac")).expect("matched by folder and file name");
        assert_eq!(found.title, "Morning Fog");
        assert!(found.uri.ends_with(&format!("/t/{id}.flac")));
        assert!(item_for_path(Path::new("/nowhere/Else/song.mp3")).is_none());
        assert!(item_for_path(Path::new("Morning Fog.flac")).is_none(), "a bare file name is too loose to trust");
        assert_eq!(item_for_name("morning fog", "MARLOW QUARTET").map(|i| i.title), Some("Morning Fog".into()));
        assert!(item_for_name("Morning Fog", "Someone Else").is_none() && item_for_name("", "").is_none());
    }
}
