//! Two more playlist file types other players use: PLS (the INI-style list Winamp, VLC and many
//! internet radio players write) and XSPF (an XML playlist, "spiff"). Same rules as M3U and
//! CSV: songs from this computer's folders are written as file paths and found again in your
//! music folders on the way in; anything else keeps its own address.

use std::path::{Path, PathBuf};

use crate::m3u::{file_name, item_for, location_of, place, Parsed, Place};
use crate::model::Item;

const MAX_SONGS: usize = 2000;

pub fn is_pls(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pls"))
}

pub fn is_xspf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xspf"))
}

fn display(t: &Item) -> String {
    match (t.artist.is_empty(), t.title.is_empty()) {
        (false, false) => format!("{} - {}", t.artist, t.title),
        (true, false) => t.title.clone(),
        _ => String::new(),
    }
}

fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

// ---------- PLS ----------

pub fn write_pls(tracks: &[Item], local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let mut out = String::from("[playlist]\n");
    for (i, t) in tracks.iter().enumerate() {
        let n = i + 1;
        out.push_str(&format!("File{n}={}\n", one_line(&location_of(t, local_path))));
        out.push_str(&format!("Title{n}={}\n", one_line(&display(t))));
        out.push_str(&format!("Length{n}={}\n", if t.duration > 0.0 { t.duration.round() as i64 } else { -1 }));
    }
    out.push_str(&format!("NumberOfEntries={}\nVersion=2\n", tracks.len()));
    out
}

pub fn parse_pls(text: &str, base: Option<&Path>, resolve_local: &dyn Fn(&Path) -> Option<Item>) -> Result<Parsed, String> {
    use std::collections::BTreeMap;
    let text = text.trim_start_matches('\u{feff}');
    if !text.lines().any(|l| l.trim().eq_ignore_ascii_case("[playlist]")) {
        return Err("this isn't a PLS playlist".into());
    }
    let mut files: BTreeMap<u32, String> = BTreeMap::new();
    let mut titles: BTreeMap<u32, String> = BTreeMap::new();
    let mut lengths: BTreeMap<u32, f32> = BTreeMap::new();
    for l in text.lines() {
        let Some((k, v)) = l.split_once('=') else { continue };
        let k = k.trim().to_lowercase();
        let v = v.trim();
        let num = |prefix: &str| k.strip_prefix(prefix).and_then(|n| n.parse::<u32>().ok());
        if let Some(n) = num("file") {
            files.insert(n, v.to_string());
        } else if let Some(n) = num("title") {
            titles.insert(n, v.to_string());
        } else if let Some(n) = num("length") {
            lengths.insert(n, v.parse().unwrap_or(-1.0));
        }
    }
    let mut p = Parsed::default();
    for (n, entry) in files {
        if p.tracks.len() >= MAX_SONGS {
            p.notes.push(format!("Only the first {MAX_SONGS} songs were read."));
            break;
        }
        add(&mut p, &entry, titles.get(&n).map(String::as_str).unwrap_or(""), lengths.get(&n).copied().unwrap_or(-1.0), "", base, resolve_local);
    }
    if p.tracks.is_empty() && p.notes.is_empty() {
        return Err("there are no songs in this file".into());
    }
    Ok(p)
}

fn add(p: &mut Parsed, entry: &str, name: &str, secs: f32, album: &str, base: Option<&Path>, resolve_local: &dyn Fn(&Path) -> Option<Item>) {
    match place(entry, base) {
        Place::Address(uri) => p.tracks.push(item_for(&uri, name, secs, album)),
        Place::File(path) => match resolve_local(&path) {
            Some(item) => p.tracks.push(item),
            None => p.notes.push(format!("Skipped “{}”: it isn't in your music folders on this computer.", file_name(&path))),
        },
        Place::Unusable => p.notes.push(format!("Skipped “{}”: not an address or file Roomtone can use.", entry.chars().take(60).collect::<String>())),
    }
}

// ---------- XSPF ----------

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn encode_path(p: &str) -> String {
    let mut out = String::new();
    for b in p.replace('\\', "/").bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Where a song is, as XSPF wants it: a `file://` address for a file path, else its own address.
fn xspf_location(t: &Item, local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    match local_path(&t.uri) {
        Some(p) => {
            let s = encode_path(&p.to_string_lossy());
            if s.starts_with('/') {
                format!("file://{s}")
            } else {
                format!("file:///{s}")
            }
        }
        None => location_of(t, local_path),
    }
}

pub fn write_xspf(title: &str, tracks: &[Item], local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n");
    out.push_str(&format!("  <title>{}</title>\n  <trackList>\n", xml(&one_line(title))));
    for t in tracks {
        out.push_str("    <track>\n");
        out.push_str(&format!("      <location>{}</location>\n", xml(&one_line(&xspf_location(t, local_path)))));
        if !t.title.is_empty() {
            out.push_str(&format!("      <title>{}</title>\n", xml(&one_line(&t.title))));
        }
        if !t.artist.is_empty() {
            out.push_str(&format!("      <creator>{}</creator>\n", xml(&one_line(&t.artist))));
        }
        if !t.album.is_empty() && !t.album.starts_with("http") {
            out.push_str(&format!("      <album>{}</album>\n", xml(&one_line(&t.album))));
        }
        if t.duration > 0.0 {
            out.push_str(&format!("      <duration>{}</duration>\n", (t.duration * 1000.0).round() as i64));
        }
        out.push_str("    </track>\n");
    }
    out.push_str("  </trackList>\n</playlist>\n");
    out
}

pub fn parse_xspf(text: &str, base: Option<&Path>, resolve_local: &dyn Fn(&Path) -> Option<Item>) -> Result<Parsed, String> {
    let text = text.trim_start_matches('\u{feff}');
    // Playlist files are small; refuse anything with a DTD rather than expanding entities.
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("this XML file uses features Roomtone doesn't read".into());
    }
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("this isn't a readable XSPF playlist ({e})"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "playlist" {
        return Err("this isn't an XSPF playlist".into());
    }
    let child = |n: roxmltree::Node, name: &str| {
        n.children().find(|c| c.is_element() && c.tag_name().name() == name).and_then(|c| c.text()).map(|t| t.trim().to_string()).unwrap_or_default()
    };
    let mut p = Parsed { title: Some(child(root, "title")).filter(|t| !t.is_empty()), ..Default::default() };
    let tracks = root.children().find(|c| c.is_element() && c.tag_name().name() == "trackList");
    for t in tracks.into_iter().flat_map(|l| l.children()).filter(|c| c.is_element() && c.tag_name().name() == "track") {
        if p.tracks.len() >= MAX_SONGS {
            p.notes.push(format!("Only the first {MAX_SONGS} songs were read."));
            break;
        }
        let (loc, title, artist, album) = (child(t, "location"), child(t, "title"), child(t, "creator"), child(t, "album"));
        let secs = child(t, "duration").parse::<f32>().map(|ms| ms / 1000.0).unwrap_or(-1.0);
        if loc.is_empty() {
            p.notes.push("Skipped a song with no location.".into());
            continue;
        }
        let name = match (artist.is_empty(), title.is_empty()) {
            (false, false) => format!("{artist} - {title}"),
            _ => title,
        };
        add(&mut p, &loc, &name, secs, &album, base, resolve_local);
    }
    if p.tracks.is_empty() && p.notes.is_empty() {
        return Err("there are no songs in this file".into());
    }
    Ok(p)
}

#[cfg(test)]
mod unit {
    use super::*;

    fn song(title: &str, artist: &str, uri: &str, secs: f32) -> Item {
        Item { title: title.into(), artist: artist.into(), uri: uri.into(), duration: secs, album: "Low Tide".into(), ..Default::default() }
    }

    fn none(_: &Path) -> Option<Item> {
        None
    }

    fn sample() -> Vec<Item> {
        vec![
            song("Morning Fog", "Marlow Quartet", "x-file-cifs://nas/Music/a.flac", 201.4),
            song("Radio & more <live>", "", "https://cdn.example/b.mp3?x=1&y=2", 0.0),
            song("Mine", "Me", "http://192.168.1.5:52880/t/abc.mp3", 60.0),
        ]
    }

    fn local(u: &str) -> Option<PathBuf> {
        u.contains("/t/abc").then(|| PathBuf::from("/Users/me/My Music/mine.mp3"))
    }

    fn mine(p: &Path) -> Option<Item> {
        (p == Path::new("/Users/me/My Music/mine.mp3")).then(|| song("Mine", "Me", "http://192.168.1.5:52880/t/abc.mp3", 60.0))
    }

    #[test]
    fn pls_has_the_usual_shape_and_reads_back() {
        let text = write_pls(&sample(), &local);
        assert!(text.starts_with("[playlist]\nFile1=smb://nas/Music/a.flac\nTitle1=Marlow Quartet - Morning Fog\nLength1=201\n"), "{text}");
        assert!(text.contains("NumberOfEntries=3\nVersion=2"));
        let back = parse_pls(&text, None, &mine).unwrap();
        assert_eq!(back.tracks.len(), 3, "{:?}", back.notes);
        assert_eq!(back.tracks[0].uri, "x-file-cifs://nas/Music/a.flac");
        assert_eq!((back.tracks[0].artist.as_str(), back.tracks[0].title.as_str()), ("Marlow Quartet", "Morning Fog"));
        assert_eq!(back.tracks[1].uri, "https://cdn.example/b.mp3?x=1&y=2");
        assert_eq!(back.tracks[2].title, "Mine");
    }

    #[test]
    fn pls_from_other_players_is_read_leniently() {
        let t = "\u{feff}[Playlist]\r\nnumberofentries=2\r\nFILE2=http://radio.example/live\r\nfile1=http://radio.example/one.mp3\r\nTITLE1=One\r\nLength1=-1\r\n";
        let p = parse_pls(t, None, &none).unwrap();
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.tracks[0].title, "One");
        assert!(parse_pls("hello", None, &none).is_err());
        assert!(parse_pls("[playlist]\nNumberOfEntries=0\n", None, &none).is_err());
        let p = parse_pls("[playlist]\nFile1=/no/such/song.mp3\n", None, &none).unwrap();
        assert!(p.tracks.is_empty() && p.notes[0].contains("song.mp3"));
    }

    #[test]
    fn xspf_is_escaped_and_reads_back() {
        let text = write_xspf("Road & trip", &sample(), &local);
        assert!(text.contains("<title>Road &amp; trip</title>"), "{text}");
        assert!(text.contains("<location>file:///Users/me/My%20Music/mine.mp3</location>"), "{text}");
        assert!(text.contains("<title>Radio &amp; more &lt;live&gt;</title>") && text.contains("x=1&amp;y=2"), "{text}");
        assert!(text.contains("<duration>201400</duration>"));
        let back = parse_xspf(&text, None, &mine).unwrap();
        assert_eq!(back.title.as_deref(), Some("Road & trip"));
        assert_eq!(back.tracks.len(), 3, "{:?}", back.notes);
        assert_eq!((back.tracks[0].artist.as_str(), back.tracks[0].title.as_str(), back.tracks[0].album.as_str()), ("Marlow Quartet", "Morning Fog", "Low Tide"));
        assert!((back.tracks[0].duration - 201.0).abs() < 1.0);
        assert_eq!(back.tracks[1].title, "Radio & more <live>");
        assert_eq!(back.tracks[1].uri, "https://cdn.example/b.mp3?x=1&y=2");
    }

    #[test]
    fn xspf_mistakes_and_unsafe_files_are_refused() {
        assert!(parse_xspf("not xml", None, &none).is_err());
        assert!(parse_xspf("<feed/>", None, &none).is_err());
        assert!(parse_xspf("<?xml version=\"1.0\"?><!DOCTYPE x [<!ENTITY a \"b\">]><playlist><trackList/></playlist>", None, &none).is_err());
        assert!(parse_xspf("<playlist><trackList/></playlist>", None, &none).is_err());
        let p = parse_xspf("<playlist><trackList><track><title>No place</title></track></trackList></playlist>", None, &none).unwrap();
        assert!(p.tracks.is_empty() && p.notes.len() == 1);
    }

    #[test]
    fn windows_paths_become_file_addresses() {
        let t = song("A", "B", "http://x/t/win.mp3", 1.0);
        let s = xspf_location(&t, &|_| Some(PathBuf::from("C:\\Music\\a b.mp3")));
        assert_eq!(s, "file:///C:/Music/a%20b.mp3");
    }
}
