//! Playlists as CSV: a spreadsheet-friendly list of songs.
//!
//! Written as UTF-8 with a byte-order mark (so Excel reads accents correctly) and columns
//! `Title, Artist, Album, Duration (seconds), Address`. Reading is forgiving: comma, semicolon
//! or tab as separator, quoted fields with commas or line breaks, and the column names that
//! other services use (`Track Name`, `Artist Name(s)`, `Duration (ms)`, `Track URI`…).
//!
//! A row that has no usable address (as in lists exported from streaming services) is matched
//! to a song with the same title and artist in your music folders, if there is one; otherwise
//! it's skipped and listed.

use std::path::{Path, PathBuf};

use crate::m3u::{file_name, item_for, location_of, place, Parsed, Place};
use crate::model::Item;

const MAX_SONGS: usize = 2000;
const BOM: char = '\u{feff}';

pub fn is_csv(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"))
}

/// A cell that a spreadsheet might run as a formula gets a leading apostrophe (and loses it on
/// the way back in).
fn guard(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@']) {
        format!("'{s}")
    } else {
        s.to_string()
    }
}

fn unguard(s: &str) -> String {
    match s.strip_prefix('\'') {
        Some(rest) if rest.starts_with(['=', '+', '-', '@']) => rest.to_string(),
        _ => s.to_string(),
    }
}

fn quote(s: &str) -> String {
    let s = guard(s);
    if s.contains([',', '"', '\n', '\r', ';', '\t']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

pub fn write(tracks: &[Item], local_path: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let mut out = String::from(BOM);
    out.push_str("Title,Artist,Album,Duration (seconds),Address\r\n");
    for t in tracks {
        let album = if t.album.starts_with("http") { "" } else { t.album.as_str() };
        let secs = if t.duration > 0.0 { (t.duration.round() as i64).to_string() } else { String::new() };
        let cells = [quote(&t.title), quote(&t.artist), quote(album), secs, quote(&location_of(t, local_path))];
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

/// Split CSV text into rows of cells. `delimiter` separates cells; quotes may hold the
/// delimiter, line breaks and doubled quotes.
fn rows(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut row: Vec<String> = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == delimiter {
            row.push(std::mem::take(&mut cell));
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if row.iter().any(|c| !c.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(c);
        }
    }
    row.push(cell);
    if row.iter().any(|c| !c.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

/// Comma, semicolon or tab: whichever the header line uses most (outside quotes, roughly).
fn delimiter_of(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    [',', ';', '\t'].into_iter().max_by_key(|d| first.matches(*d).count()).filter(|d| first.contains(*d)).unwrap_or(',')
}

#[derive(Default)]
struct Columns {
    title: Option<usize>,
    artist: Option<usize>,
    album: Option<usize>,
    duration: Option<usize>,
    duration_ms: bool,
    address: Option<usize>,
}

fn columns(header: &[String]) -> Columns {
    let mut c = Columns::default();
    for (i, h) in header.iter().enumerate() {
        let name: String = h.trim().trim_start_matches(BOM).to_lowercase();
        match name.as_str() {
            "title" | "track" | "track name" | "name" | "song" | "song name" => c.title = c.title.or(Some(i)),
            "artist" | "artists" | "artist name" | "artist name(s)" | "artist(s)" | "artist names" => c.artist = c.artist.or(Some(i)),
            "album" | "album name" => c.album = c.album.or(Some(i)),
            "address" | "uri" | "url" | "location" | "path" | "file" | "file path" | "filename" | "track uri" | "link" => c.address = c.address.or(Some(i)),
            n if (n.starts_with("duration") || n == "length" || n == "time") && c.duration.is_none() => {
                c.duration = Some(i);
                c.duration_ms = n.contains("ms");
            }
            _ => {}
        }
    }
    c
}

fn seconds(cell: &str, ms: bool) -> f32 {
    let t = cell.trim();
    if t.contains(':') {
        return crate::net::didl::parse_time(t);
    }
    let v: f32 = t.parse().unwrap_or(0.0);
    if ms {
        v / 1000.0
    } else {
        v
    }
}

/// Read CSV text as a list of songs. `base` is the file's folder (for relative paths).
/// `by_path` turns a file on this computer into a song from the music folders; `by_name` finds
/// a song there by title and artist.
pub fn parse(text: &str, base: Option<&Path>, by_path: &dyn Fn(&Path) -> Option<Item>, by_name: &dyn Fn(&str, &str) -> Option<Item>) -> Result<Parsed, String> {
    let text = text.trim_start_matches(BOM);
    let all = rows(text, delimiter_of(text));
    let Some((header, body)) = all.split_first() else { return Err("there are no songs in this file".into()) };
    let c = columns(header);
    if c.title.is_none() && c.address.is_none() {
        return Err("it needs a column named Title (or Track Name) or Address".into());
    }
    let cell = |row: &[String], i: Option<usize>| i.and_then(|i| row.get(i)).map(|s| unguard(s.trim())).unwrap_or_default();
    let mut p = Parsed::default();
    for row in body {
        if p.tracks.len() >= MAX_SONGS {
            p.notes.push(format!("Only the first {MAX_SONGS} songs were read."));
            break;
        }
        let (title, artist, album) = (cell(row, c.title), cell(row, c.artist), cell(row, c.album));
        // "Artist A, Artist B" lists are fine for matching the first.
        let first_artist = artist.split([';', ',']).next().unwrap_or("").trim().to_string();
        let secs = c.duration.map(|i| seconds(&cell(row, Some(i)), c.duration_ms)).unwrap_or(0.0);
        let address = cell(row, c.address);
        let label = if title.is_empty() { address.chars().take(60).collect::<String>() } else { title.clone() };
        let found = if address.is_empty() {
            None
        } else {
            match place(&address, base) {
                Place::Address(uri) => {
                    let display = if artist.is_empty() { title.clone() } else { format!("{artist} - {title}") };
                    let mut item = item_for(&uri, &display, secs, &album);
                    if !title.is_empty() {
                        item.title = title.clone();
                        item.artist = artist.clone();
                    }
                    Some(item)
                }
                Place::File(path) => by_path(&path),
                Place::Unusable => None,
            }
        };
        let found = found.or_else(|| if title.is_empty() { None } else { by_name(&title, &first_artist) });
        match found {
            Some(item) => p.tracks.push(item),
            None if address.is_empty() => p.notes.push(format!("Skipped “{label}”: no address, and no song with that name in your music folders.")),
            None => p.notes.push(format!("Skipped “{}”: it isn't in your music folders on this computer.", if title.is_empty() { file_name(Path::new(&address)) } else { title })),
        }
    }
    if p.tracks.is_empty() && p.notes.is_empty() {
        return Err("there are no songs in this file".into());
    }
    Ok(p)
}

#[cfg(test)]
mod unit {
    use super::*;

    fn none_path(_: &Path) -> Option<Item> {
        None
    }
    fn none_name(_: &str, _: &str) -> Option<Item> {
        None
    }
    fn song(title: &str, artist: &str, album: &str, uri: &str, secs: f32) -> Item {
        Item { title: title.into(), artist: artist.into(), album: album.into(), uri: uri.into(), duration: secs, ..Default::default() }
    }

    #[test]
    fn written_files_open_in_a_spreadsheet() {
        let tracks = vec![
            song("Morning Fog", "Marlow Quartet", "Low Tide", "x-file-cifs://nas/Music/a.flac", 201.4),
            song("Hello, \"World\"\nagain", "A, B", "", "https://cdn.example/b.mp3", 0.0),
            song("=SUM(1+1)", "-Dash", "", "http://192.168.1.5:52880/t/abc.mp3", 60.0),
        ];
        let text = write(&tracks, &|u| u.contains("/t/abc").then(|| PathBuf::from("/Users/me/Music/mine.mp3")));
        assert!(text.starts_with("\u{feff}Title,Artist,Album,Duration (seconds),Address\r\n"));
        assert!(text.contains("Morning Fog,Marlow Quartet,Low Tide,201,smb://nas/Music/a.flac\r\n"), "{text}");
        assert!(text.contains("\"Hello, \"\"World\"\"\nagain\",\"A, B\",,,https://cdn.example/b.mp3\r\n"), "{text}");
        assert!(text.contains("'=SUM(1+1),'-Dash,,60,/Users/me/Music/mine.mp3"), "cells that look like formulas are guarded: {text}");
    }

    #[test]
    fn what_is_written_reads_back_exactly() {
        let tracks = vec![
            song("Morning Fog", "Marlow Quartet", "Low Tide", "x-file-cifs://nas/Music/a.flac", 201.0),
            song("Hello, \"World\"\nagain", "A, B", "", "https://cdn.example/b.mp3", 0.0),
            song("=SUM(1+1)", "-Dash", "", "https://cdn.example/c.mp3", 60.0),
        ];
        let p = parse(&write(&tracks, &|_| None), None, &none_path, &none_name).unwrap();
        assert!(p.notes.is_empty(), "{:?}", p.notes);
        assert_eq!(p.tracks.len(), 3);
        for (a, b) in tracks.iter().zip(&p.tracks) {
            assert_eq!((&a.title, &a.artist, &a.album), (&b.title, &b.artist, &b.album));
            assert_eq!(a.duration, b.duration);
        }
        assert_eq!(p.tracks[0].uri, "x-file-cifs://nas/Music/a.flac", "smb:// goes back to the speakers' name for it");
        assert_eq!(p.tracks[2].title, "=SUM(1+1)", "the guard is removed again");
    }

    #[test]
    fn other_services_column_names_and_separators() {
        let text = "\u{feff}Track URI;Track Name;Artist Name(s);Album Name;Duration (ms)\r\nspotify:track:abc;Morning Fog;Marlow Quartet;Low Tide;201000\r\nspotify:track:def;Unknown;Nobody;;1000\r\n";
        let by_name = |t: &str, a: &str| (t == "Morning Fog" && a == "Marlow Quartet").then(|| song("Morning Fog", "Marlow Quartet", "Low Tide", "http://me/t/1.flac", 201.0));
        let p = parse(text, None, &none_path, &by_name).unwrap();
        assert_eq!(p.tracks.len(), 1, "matched by title and artist where the address is no use");
        assert_eq!(p.tracks[0].uri, "http://me/t/1.flac");
        assert_eq!(p.notes.len(), 1);
        assert!(p.notes[0].contains("Unknown"), "{:?}", p.notes);
    }

    #[test]
    fn addresses_files_and_durations() {
        let text = "title,artist,length,file\nOne,A,3:21,/Users/me/Music/a.flac\nTwo,B,95,https://cdn.example/two.mp3\nThree,C,10,/elsewhere/c.mp3\n";
        let by_path = |p: &Path| p.to_string_lossy().ends_with("a.flac").then(|| song("From library", "A", "", "http://me/t/9.flac", 201.0));
        let p = parse(text, None, &by_path, &none_name).unwrap();
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.tracks[0].title, "From library");
        assert_eq!((p.tracks[1].title.as_str(), p.tracks[1].duration), ("Two", 95.0));
        assert!(p.notes.iter().any(|n| n.contains("Three")), "{:?}", p.notes);
    }

    #[test]
    fn rows_with_quotes_blank_lines_and_tabs() {
        let text = "Title\tAddress\n\"A\tB\"\thttp://x/1.mp3\n\n   \t  \nC\thttp://x/2.mp3";
        let p = parse(text, None, &none_path, &none_name).unwrap();
        assert_eq!(p.tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["A\tB", "C"]);
    }

    #[test]
    fn files_that_are_not_song_lists_are_refused() {
        assert!(parse("", None, &none_path, &none_name).unwrap_err().contains("no songs"));
        assert!(parse("Colour,Size\nred,3\n", None, &none_path, &none_name).unwrap_err().contains("Title"));
        assert!(parse("Title,Artist\n", None, &none_path, &none_name).is_err(), "a header and nothing else");
    }

    #[test]
    fn long_lists_are_cut_with_a_note() {
        let mut text = String::from("Title,Address\n");
        for n in 0..MAX_SONGS + 3 {
            text.push_str(&format!("S{n},http://x/{n}.mp3\n"));
        }
        let p = parse(&text, None, &none_path, &none_name).unwrap();
        assert_eq!(p.tracks.len(), MAX_SONGS);
        assert!(p.notes.iter().any(|n| n.contains("first 2000")));
    }

    #[test]
    fn small_helpers() {
        assert!(is_csv(Path::new("a.CSV")) && !is_csv(Path::new("a.m3u")));
        assert_eq!(delimiter_of("a;b;c"), ';');
        assert_eq!(delimiter_of("a,b;c,d"), ',');
        assert_eq!(delimiter_of("only"), ',');
        assert_eq!(unguard("'=x"), "=x");
        assert_eq!(unguard("'quoted"), "'quoted", "an ordinary apostrophe stays");
    }
}
