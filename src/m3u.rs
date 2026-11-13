//! M3U / M3U8 playlist files: the plain text list of songs most players can read and write.
//!
//! Written as "extended" M3U in UTF-8 (`#EXTM3U`, `#PLAYLIST:`, `#EXTINF:`, `#EXTALB:`), with
//! one address per song: a real file path for songs from this computer's folders, `smb://` for
//! songs on a network share (that's what the speakers call `x-file-cifs://`), and the plain
//! address for anything else. Reading takes the same, plus bare file paths (absolute, relative
//! to the playlist, or `file://`).
//!
//! Songs from streaming services only mean something to the speakers (their addresses are
//! written as they are and come back unchanged); other players will skip them.

use std::path::{Path, PathBuf};

use crate::model::Item;

/// Most songs read from one file.
const MAX_SONGS: usize = 2000;

/// What reading a playlist file gave.
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    /// From `#PLAYLIST:`, if the file has one.
    pub title: Option<String>,
    pub tracks: Vec<Item>,
    /// Entries that couldn't be used, one plain sentence each.
    pub notes: Vec<String>,
}

/// Is this a file name that should be read as M3U?
pub fn is_m3u(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
}

/// The playlist as M3U text. `local_path` says which file on this computer an address from the
/// app's own music server stands for.
pub fn write(title: &str, tracks: &[Item], local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let line = |s: &str| s.replace(['\r', '\n'], " ");
    let mut out = String::from("#EXTM3U\n");
    out.push_str(&format!("#PLAYLIST:{}\n", line(title)));
    for t in tracks {
        let secs = if t.duration > 0.0 { t.duration.round() as i64 } else { -1 };
        let name = match (t.artist.is_empty(), t.title.is_empty()) {
            (false, false) => format!("{} - {}", t.artist, t.title),
            (true, false) => t.title.clone(),
            _ => String::new(),
        };
        out.push_str(&format!("#EXTINF:{secs},{}\n", line(&name)));
        if !t.album.is_empty() && !t.album.starts_with("http") {
            out.push_str(&format!("#EXTALB:{}\n", line(&t.album)));
        }
        out.push_str(&line(&location_of(t, local_path)));
        out.push('\n');
    }
    out
}

/// Where a song is, for a playlist file: its file path if it's on this computer, an `smb://`
/// address for a network share, otherwise its own address.
pub(crate) fn location_of(t: &Item, local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    match local_path(&t.uri) {
        Some(p) => p.to_string_lossy().to_string(),
        None => match t.uri.strip_prefix("x-file-cifs://") {
            Some(rest) => format!("smb://{rest}"),
            None => t.uri.clone(),
        },
    }
}

/// Read M3U text. `base` is the folder the file is in (for relative paths); `resolve_local`
/// turns a file on this computer into a song from the music folders, if it's one of them.
pub fn parse(text: &str, base: Option<&Path>, resolve_local: &dyn Fn(&Path) -> Option<Item>) -> Result<Parsed, String> {
    let text = text.trim_start_matches('\u{feff}');
    if text.lines().any(|l| l.starts_with("#EXT-X-")) {
        return Err("this is a streaming (HLS) playlist, not a list of songs".into());
    }
    let mut p = Parsed::default();
    let mut info: Option<(f32, String)> = None;
    let mut album = String::new();
    for raw in text.lines() {
        let l = raw.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(rest) = l.strip_prefix('#') {
            if let Some(v) = rest.strip_prefix("PLAYLIST:") {
                p.title = Some(v.trim().to_string()).filter(|t| !t.is_empty());
            } else if let Some(v) = rest.strip_prefix("EXTINF:") {
                let (head, name) = v.split_once(',').unwrap_or((v, ""));
                let secs: f32 = head.split_whitespace().next().and_then(|d| d.parse().ok()).unwrap_or(-1.0);
                info = Some((secs, name.trim().to_string()));
            } else if let Some(v) = rest.strip_prefix("EXTALB:") {
                album = v.trim().to_string();
            }
            continue;
        }
        if p.tracks.len() >= MAX_SONGS {
            p.notes.push(format!("Only the first {MAX_SONGS} songs were read."));
            break;
        }
        let (secs, name) = info.take().unwrap_or((-1.0, String::new()));
        let album = std::mem::take(&mut album);
        match place(l, base) {
            Place::Address(uri) => p.tracks.push(item_for(&uri, &name, secs, &album)),
            Place::File(path) => match resolve_local(&path) {
                Some(item) => p.tracks.push(item),
                None => p.notes.push(format!("Skipped “{}”: it isn't in your music folders on this computer.", file_name(&path))),
            },
            Place::Unusable => p.notes.push(format!("Skipped “{}”: not an address or file Roomtone can use.", l.chars().take(60).collect::<String>())),
        }
    }
    if p.tracks.is_empty() && p.notes.is_empty() {
        return Err("there are no songs in this file".into());
    }
    Ok(p)
}

pub(crate) enum Place {
    /// Something a speaker can fetch or play as it is.
    Address(String),
    /// A file on this computer.
    File(PathBuf),
    Unusable,
}

pub(crate) fn place(entry: &str, base: Option<&Path>) -> Place {
    let lower = entry.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("x-") {
        return Place::Address(entry.to_string());
    }
    if let Some(rest) = entry.strip_prefix("smb://").or_else(|| entry.strip_prefix("SMB://")) {
        return Place::Address(format!("x-file-cifs://{rest}"));
    }
    if let Some(rest) = lower.strip_prefix("file://").map(|_| &entry[7..]) {
        // file:///Users/me/a.mp3, or file://localhost/Users/me/a.mp3
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        let rest = rest.strip_prefix('/').filter(|r| r.chars().nth(1) == Some(':')).unwrap_or(rest); // file:///C:/Music
        return Place::File(PathBuf::from(percent_decode(rest)));
    }
    if lower.contains("://") || entry.starts_with("mms:") {
        return Place::Unusable;
    }
    let path = PathBuf::from(entry);
    let windows_style = entry.chars().nth(1) == Some(':') || entry.starts_with("\\\\");
    if path.is_absolute() || windows_style {
        return Place::File(path);
    }
    match base {
        Some(b) => Place::File(b.join(entry.replace('\\', "/"))),
        None => Place::File(path),
    }
}

pub(crate) fn file_name(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.rsplit(['/', '\\']).next().unwrap_or(&s).to_string()
}

pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = if b[i] == b'%' && i + 2 < b.len() { std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) } else { None };
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

pub(crate) fn item_for(uri: &str, display: &str, secs: f32, album: &str) -> Item {
    let (artist, title) = match display.split_once(" - ") {
        Some((a, t)) if !a.trim().is_empty() && !t.trim().is_empty() => (a.trim().to_string(), t.trim().to_string()),
        _ => (String::new(), display.to_string()),
    };
    let title = if title.is_empty() {
        let name = file_name(Path::new(uri.split(['?', '#']).next().unwrap_or(uri)));
        let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(name);
        percent_decode(&stem)
    } else {
        title
    };
    Item { title, artist, album: album.to_string(), class: "object.item.audioItem.musicTrack".into(), uri: uri.to_string(), duration: secs.max(0.0), ..Default::default() }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn no_local(_: &Path) -> Option<Item> {
        None
    }

    fn song(title: &str, artist: &str, uri: &str, secs: f32) -> Item {
        Item { title: title.into(), artist: artist.into(), uri: uri.into(), duration: secs, album: "Low Tide".into(), ..Default::default() }
    }

    #[test]
    fn written_files_have_the_usual_shape() {
        let tracks = vec![
            song("Morning Fog", "Marlow Quartet", "x-file-cifs://nas/Music/a.flac", 201.4),
            song("Radio edit", "", "https://cdn.example/b.mp3", 0.0),
            song("On this computer", "Me", "http://192.168.1.5:52880/t/abc.mp3", 60.0),
        ];
        let text = write("Road trip", &tracks, &|u| u.contains("/t/abc").then(|| PathBuf::from("/Users/me/Music/mine.mp3")));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(&lines[..2], ["#EXTM3U", "#PLAYLIST:Road trip"]);
        assert!(text.contains("#EXTINF:201,Marlow Quartet - Morning Fog\n#EXTALB:Low Tide\nsmb://nas/Music/a.flac\n"), "{text}");
        assert!(text.contains("#EXTINF:-1,Radio edit\n#EXTALB:Low Tide\nhttps://cdn.example/b.mp3\n"), "{text}");
        assert!(text.contains("\n/Users/me/Music/mine.mp3\n"), "songs from this computer are written as real paths: {text}");
    }

    #[test]
    fn what_is_written_reads_back() {
        let tracks = vec![song("Morning Fog", "Marlow Quartet", "x-file-cifs://nas/Music/a.flac", 201.0), song("Other", "A. Band", "https://cdn.example/b.mp3", 95.0)];
        let text = write("Road trip", &tracks, &|_| None);
        let back = parse(&text, None, &no_local).unwrap();
        assert_eq!(back.title.as_deref(), Some("Road trip"));
        assert_eq!(back.tracks.len(), 2);
        assert_eq!(
            (back.tracks[0].title.as_str(), back.tracks[0].artist.as_str(), back.tracks[0].uri.as_str()),
            ("Morning Fog", "Marlow Quartet", "x-file-cifs://nas/Music/a.flac")
        );
        assert_eq!(back.tracks[0].duration, 201.0);
        assert_eq!(back.tracks[0].album, "Low Tide");
        assert_eq!(back.tracks[1].uri, "https://cdn.example/b.mp3");
        assert!(back.notes.is_empty());
    }

    #[test]
    fn plain_and_windows_style_files_are_read() {
        let text = "\u{feff}http://stream.example/one.mp3\r\n\r\n# a comment\r\nhttps://cdn.example/Two%20Songs.flac\r\n";
        let p = parse(text, None, &no_local).unwrap();
        assert_eq!(p.tracks.len(), 2, "BOM, blank lines, comments and CRLF are all fine");
        assert_eq!(p.tracks[0].title, "one", "no title: the file name is used");
        assert_eq!(p.tracks[1].title, "Two Songs");
        assert!(p.title.is_none());
    }

    #[test]
    fn files_on_this_computer_are_matched_to_the_music_folders() {
        let text = "#EXTM3U\n/Users/me/Music/a.flac\n../other/b.mp3\nC:\\Music\\c.mp3\nfile:///Users/me/Music/My%20Song.mp3\n";
        let seen = std::cell::RefCell::new(vec![]);
        let resolver = |p: &Path| {
            seen.borrow_mut().push(p.to_string_lossy().to_string());
            p.to_string_lossy().ends_with("a.flac").then(|| Item { title: "From library".into(), uri: "http://me/t/1.flac".into(), ..Default::default() })
        };
        let p = parse(text, Some(Path::new("/Users/me/Lists")), &resolver).unwrap();
        assert_eq!(p.tracks.len(), 1);
        assert_eq!(p.tracks[0].title, "From library", "the library's own details win");
        assert_eq!(p.notes.len(), 3, "{:?}", p.notes);
        assert!(p.notes.iter().any(|n| n.contains("c.mp3")) && p.notes.iter().any(|n| n.contains("My Song.mp3")), "{:?}", p.notes);
        let seen = seen.borrow();
        // (Joined the way this computer joins paths: a backslash on Windows.)
        let relative = Path::new("/Users/me/Lists").join("../other/b.mp3").to_string_lossy().to_string();
        assert!(seen.contains(&relative), "relative paths start from the playlist's folder: {seen:?}");
        assert!(seen.contains(&"/Users/me/Music/My Song.mp3".to_string()), "file:// addresses are decoded: {seen:?}");
        assert!(seen.contains(&"C:\\Music\\c.mp3".to_string()));
    }

    #[test]
    fn speaker_addresses_and_shares_come_through() {
        let text = "smb://nas/Music/a.flac\nx-sonos-http:abc.flac?sid=9&sn=1\n";
        let p = parse(text, None, &no_local).unwrap();
        assert_eq!(p.tracks[0].uri, "x-file-cifs://nas/Music/a.flac");
        assert_eq!(p.tracks[1].uri, "x-sonos-http:abc.flac?sid=9&sn=1");
    }

    #[test]
    fn things_that_are_not_song_lists_are_refused_kindly() {
        assert!(parse("#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:10,\nseg1.ts\n", None, &no_local).unwrap_err().contains("HLS"));
        assert!(parse("", None, &no_local).unwrap_err().contains("no songs"));
        assert!(parse("#EXTM3U\n#PLAYLIST:Empty\n", None, &no_local).is_err());
        let odd = parse("ftp://host/a.mp3\nmms://host/b\n", None, &no_local).unwrap();
        assert!(odd.tracks.is_empty() && odd.notes.len() == 2, "{:?}", odd.notes);
    }

    #[test]
    fn very_long_lists_are_cut_with_a_note() {
        let text: String = (0..MAX_SONGS + 5).map(|n| format!("http://x/{n}.mp3\n")).collect();
        let p = parse(&text, None, &no_local).unwrap();
        assert_eq!(p.tracks.len(), MAX_SONGS);
        assert!(p.notes.iter().any(|n| n.contains("first 2000")));
    }

    #[test]
    fn titles_with_line_breaks_cannot_break_the_file() {
        let t = vec![song("Two\nlines", "A\r\nB", "http://x/a.mp3", 1.0)];
        let text = write("My\nlist", &t, &|_| None);
        assert!(parse(&text, None, &no_local).unwrap().tracks.len() == 1, "{text}");
    }

    #[test]
    fn file_names() {
        assert!(is_m3u(Path::new("a.M3U8")) && is_m3u(Path::new("/x/b.m3u")));
        assert!(!is_m3u(Path::new("a.json")));
        assert_eq!(percent_decode("a%20b%2Fc%zz%4"), "a b/c%zz%4");
    }
}
