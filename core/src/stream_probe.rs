//! The stream's (or podcast episode's) own description of its sound (experimental, off until the person switches it on).
//!
//! A speaker usually reports only the *format* of an internet radio stream. The stream itself often
//! says more in the headers of its reply: Shoutcast and Icecast servers send `icy-br` (the bitrate),
//! `icy-audio-info` (sample rate, channels, bitrate) and `Content-Type`. With the switch on, Roomtone
//! opens the playing stream's address once, as the speaker does, reads those headers and closes it
//! again without reading any music. Only a plain `http` or `https` stream is asked; nothing about
//! the person or the speakers is sent (a `User-Agent` with the app version). The answer is kept in
//! memory for this run (listed in SPEC §9).

use std::time::Duration;

/// What a stream said about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    /// From the `Content-Type` header, e.g. "audio/aacp".
    pub content_type: Option<String>,
    /// The station's own name for itself (`icy-name`), shown only in the details.
    pub name: Option<String>,
    /// The size of a file (`Content-Length`): for a podcast episode, which gives an average bitrate.
    pub size_bytes: Option<u64>,
}

impl Probe {
    pub fn is_empty(&self) -> bool {
        *self == Probe::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Loading,
    Found(Probe),
    /// The stream couldn't be reached or said nothing useful; tried again the next time it plays.
    Failed(String),
}

/// The web address of a podcast episode or other single file the speaker is playing: only the
/// song's own address, and only a plain `http(s)` one (a queue or a service has none).
pub fn file_address(track_uri: &str) -> Option<String> {
    let u = track_uri.trim();
    ((u.starts_with("http://") || u.starts_with("https://")) && address(u, "").is_some()).then(|| u.to_string())
}

/// The web address of the stream a speaker was given, if it is a plain `http(s)` one: the song's
/// address, or what the room was asked to play. Speakers' own streams (`x-sonosapi-…`, queues,
/// files) have no address a computer can open.
pub fn address(track_uri: &str, media_uri: &str) -> Option<String> {
    [track_uri, media_uri].iter().find_map(|u| web_address(u))
}

fn web_address(uri: &str) -> Option<String> {
    let u = uri.trim();
    let url = if let Some(rest) = u.strip_prefix("x-rincon-mp3radio://").or_else(|| u.strip_prefix("aac://")) {
        format!("http://{rest}")
    } else if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        return None;
    };
    // A listing of the stream, not the stream: a playlist file or a segmented (HLS) one.
    let path = url.split(['?', '#']).next().unwrap_or("").to_lowercase();
    if [".m3u", ".m3u8", ".pls", ".asx"].iter().any(|e| path.ends_with(e)) || url.len() > 600 {
        return None;
    }
    Some(url)
}

/// Read a stream's description from its reply headers (`header` returns one by name).
pub fn from_headers(header: &dyn Fn(&str) -> Option<String>) -> Probe {
    let num = |s: &str| s.trim().parse::<u32>().ok().filter(|n| *n > 0);
    // `icy-br` can list several bitrates ("128,64"); the first is this stream's.
    let mut p = Probe { bitrate_kbps: header("icy-br").and_then(|v| v.split([',', ';']).next().and_then(num)), ..Default::default() };
    // `icy-audio-info`: "ice-samplerate=44100;ice-bitrate=128;ice-channels=2" (also `ice-audio-info`).
    if let Some(info) = header("icy-audio-info").or_else(|| header("ice-audio-info")) {
        for part in info.split([';', '&']) {
            let Some((k, v)) = part.split_once('=') else { continue };
            match k.trim().to_lowercase().trim_start_matches("ice-").trim_start_matches("icy-") {
                "samplerate" | "sample-rate" => p.sample_rate = p.sample_rate.or(num(v)),
                "bitrate" => p.bitrate_kbps = p.bitrate_kbps.or(num(v)),
                "channels" => p.channels = p.channels.or(num(v)),
                _ => {}
            }
        }
    }
    p.sample_rate = p.sample_rate.or_else(|| header("icy-sr").and_then(|v| num(&v)));
    p.content_type = header("content-type").map(|v| v.split(';').next().unwrap_or("").trim().to_string()).filter(|v| !v.is_empty());
    p.size_bytes = header("content-length").and_then(|v| v.trim().parse::<u64>().ok()).filter(|n| *n > 0);
    p.name = header("icy-name").map(|v| v.trim().chars().take(80).collect::<String>()).filter(|v| !v.is_empty());
    p
}

/// Ask the stream (headers only; the music is not read). Keeps to a few seconds.
pub fn probe_over_http(url: &str) -> Result<Probe, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("not a web address".into());
    }
    let agent =
        ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(6)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build();
    let reply = agent.get(url).set("Icy-MetaData", "0").call().map_err(|e| e.to_string())?;
    let p = from_headers(&|name| reply.header(name).map(|v| v.to_string()));
    // Dropping the reply closes the connection without reading the stream.
    drop(reply);
    if p.is_empty() {
        Err("the stream didn't say anything about its sound".into())
    } else {
        Ok(p)
    }
}

/// Made-up details for the demo system, which never goes online.
pub fn demo() -> Probe {
    Probe { bitrate_kbps: Some(128), sample_rate: Some(44100), channels: Some(2), content_type: Some("audio/aacp".into()), name: Some("Demo Radio".into()), size_bytes: None }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn headers(list: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |n: &str| list.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.to_string())
    }

    #[test]
    fn only_plain_web_streams_have_an_address_to_ask() {
        assert_eq!(address("x-rincon-mp3radio://stream.example.com/live", "").as_deref(), Some("http://stream.example.com/live"));
        assert_eq!(address("aac://radio.example.com:8000/hi", "").as_deref(), Some("http://radio.example.com:8000/hi"));
        assert_eq!(address("", "https://radio.example.com/stream").as_deref(), Some("https://radio.example.com/stream"));
        assert_eq!(address("x-sonosapi-stream:s1234?sid=254", "x-rincon-queue:RINCON_1#0"), None, "a speaker-side stream has no address");
        assert_eq!(address("x-file-cifs://nas/music/a.flac", ""), None);
        assert_eq!(address("https://example.com/list.m3u8", ""), None, "a segmented or playlist address isn't the stream");
        assert_eq!(address("http://example.com/radio.pls", ""), None);
        assert_eq!(address("", ""), None);
        assert_eq!(file_address("https://traffic.example.com/ep/1.mp3?x=1").as_deref(), Some("https://traffic.example.com/ep/1.mp3?x=1"));
        assert_eq!(file_address("x-rincon-mp3radio://s.example.com/live"), None, "a stream isn't a file");
        assert_eq!(file_address("x-rincon-queue:RINCON_1#0"), None);
        assert_eq!(file_address("https://example.com/list.m3u8"), None);
    }

    #[test]
    fn the_headers_give_bitrate_rate_and_channels() {
        let p = from_headers(&headers(&[
            ("icy-br", "128"),
            ("content-type", "audio/aacp; charset=x"),
            ("icy-name", " Jazz FM "),
            ("icy-audio-info", "ice-samplerate=44100;ice-bitrate=64;ice-channels=2"),
        ]));
        assert_eq!(p.bitrate_kbps, Some(128), "icy-br wins");
        assert_eq!((p.sample_rate, p.channels), (Some(44100), Some(2)));
        assert_eq!(p.content_type.as_deref(), Some("audio/aacp"));
        assert_eq!(p.name.as_deref(), Some("Jazz FM"));
        assert_eq!(p.size_bytes, None);
        assert_eq!(from_headers(&headers(&[("Content-Length", " 48000000 ")])).size_bytes, Some(48_000_000));
        let only_info = from_headers(&headers(&[("ice-audio-info", "ice-bitrate=192")]));
        assert_eq!(only_info.bitrate_kbps, Some(192));
        let list = from_headers(&headers(&[("icy-br", "96,48")]));
        assert_eq!(list.bitrate_kbps, Some(96), "the first of several");
        assert!(from_headers(&headers(&[("icy-br", "0"), ("icy-audio-info", "junk")])).is_empty(), "nonsense says nothing");
    }

    #[test]
    #[ignore = "asks a real internet stream"]
    fn a_real_stream() {
        let p = probe_over_http("https://ice1.somafm.com/groovesalad-128-mp3").expect("the stream should answer");
        println!("{p:?}");
        assert!(p.bitrate_kbps.is_some() || p.content_type.is_some());
    }
}
