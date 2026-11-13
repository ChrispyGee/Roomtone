//! Internet radio: the person's own stations and streams, plus the free, community-run
//! Radio Browser directory (tens of thousands of stations, no account needed).

use std::time::Duration;

use crate::config::Station;
use crate::model::Item;
use crate::net::soap::escape;

const SERVERS: &[&str] = &["https://de1.api.radio-browser.info", "https://de2.api.radio-browser.info", "https://all.api.radio-browser.info"];

pub const TAGS: &[&str] = &["pop", "rock", "jazz", "classical", "news", "electronic", "chillout", "hits", "country", "talk", "ambient", "dance"];

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(10)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build()
}

/// The address the speakers are given. Plain http streams use the speakers' radio scheme so
/// they show as live radio; https streams are passed as-is (estimate: not every S1 speaker
/// copes with modern https).
pub fn speaker_uri(url: &str) -> String {
    let u = url.trim();
    if let Some(rest) = u.strip_prefix("http://") {
        format!("x-rincon-mp3radio://{rest}")
    } else {
        u.to_string()
    }
}

pub fn station_meta(name: &str) -> String {
    format!(
        "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" \
xmlns:r=\"urn:schemas-rinconnetworks-com:metadata-1-0/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\">\
<item id=\"R:0/0/0\" parentID=\"R:0/0\" restricted=\"true\"><dc:title>{}</dc:title>\
<upnp:class>object.item.audioItem.audioBroadcast</upnp:class>\
<desc id=\"cdudn\" nameSpace=\"urn:schemas-rinconnetworks-com:metadata-1-0/\">SA_RINCON65031_</desc></item></DIDL-Lite>",
        escape(name)
    )
}

pub fn station_item(name: &str, url: &str, desc: &str, genre: &str, art: Option<String>, source: &str) -> Item {
    Item {
        id: format!("RADIO:S/{url}"),
        parent_id: "RADIO:".into(),
        title: name.to_string(),
        description: desc.to_string(),
        class: "object.item.audioItem.audioBroadcast".into(),
        uri: speaker_uri(url),
        meta: station_meta(name),
        art,
        source: source.into(),
        genre: genre.to_string(),
        // The original web address, so it can be saved to My stations.
        album: url.to_string(),
        ..Default::default()
    }
}

pub fn my_station_item(s: &Station) -> Item {
    let desc = if s.genre.is_empty() { "My station".to_string() } else { format!("My station · {}", s.genre) };
    station_item(&s.name, &s.url, &desc, &s.genre, None, "mystations")
}

fn fetch(path_and_query: &[(&str, &str)], endpoint: &str) -> Result<Vec<Item>, String> {
    let mut last = String::from("Radio directory unavailable");
    for server in SERVERS {
        let mut req = agent().get(&format!("{server}/json/{endpoint}"));
        for (k, v) in path_and_query {
            req = req.query(k, v);
        }
        match req.call() {
            Ok(resp) => {
                let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
                return Ok(parse(&v));
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

fn parse(v: &serde_json::Value) -> Vec<Item> {
    let Some(arr) = v.as_array() else { return vec![] };
    let mut out: Vec<Item> = vec![];
    for s in arr {
        let get = |k: &str| s.get(k).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
        let name = get("name");
        let url = {
            let r = get("url_resolved");
            if r.is_empty() {
                get("url")
            } else {
                r
            }
        };
        if name.is_empty() || url.is_empty() || out.iter().any(|o| o.title == name) {
            continue;
        }
        let country = get("country");
        let codec = get("codec");
        let bitrate = s.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0);
        let tags = get("tags");
        let genre = tags.split(',').next().unwrap_or("").trim().to_string();
        let mut desc = vec!["Radio".to_string()];
        if !country.is_empty() {
            desc.push(country);
        }
        if !codec.is_empty() {
            desc.push(if bitrate > 0 { format!("{codec} {bitrate}k") } else { codec });
        }
        let favicon = get("favicon");
        let art = favicon.starts_with("http").then_some(favicon);
        out.push(station_item(&name, &url, &desc.join(" · "), &genre, art, "radio"));
    }
    out
}

pub fn top() -> Result<Vec<Item>, String> {
    fetch(&[("limit", "60"), ("hidebroken", "true")], "stations/topvote")
}

pub fn by_tag(tag: &str) -> Result<Vec<Item>, String> {
    fetch(&[("tag", tag), ("limit", "60"), ("order", "votes"), ("reverse", "true"), ("hidebroken", "true")], "stations/search")
}

pub fn search(term: &str) -> Result<Vec<Item>, String> {
    fetch(&[("name", term), ("limit", "40"), ("order", "votes"), ("reverse", "true"), ("hidebroken", "true")], "stations/search")
}

/// Playlist files (.pls, .m3u) point at the real stream; follow them.
pub fn resolve_stream(url: &str) -> String {
    let lower = url.to_lowercase();
    let is_list = [".pls", ".m3u", ".asx"].iter().any(|e| lower.split('?').next().unwrap_or("").ends_with(e));
    if !is_list || lower.ends_with(".m3u8") {
        return url.to_string();
    }
    let Ok(resp) = agent().get(url).call() else { return url.to_string() };
    let Ok(text) = resp.into_string() else { return url.to_string() };
    first_stream_in(&text).unwrap_or_else(|| url.to_string())
}

/// The first stream address in a .pls, .m3u or .asx playlist.
pub(crate) fn first_stream_in(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let candidate = line.split_once('=').filter(|(k, _)| k.to_lowercase().starts_with("file")).map(|(_, v)| v).unwrap_or(line);
        // .asx: <ref href="http://..."/>
        let candidate = candidate.split("href=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or(candidate);
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            return Some(candidate.to_string());
        }
    }
    None
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn plain_http_streams_use_the_radio_scheme() {
        assert_eq!(speaker_uri("http://stream.example.com/live.mp3"), "x-rincon-mp3radio://stream.example.com/live.mp3");
        assert_eq!(speaker_uri("https://secure.example.com/live"), "https://secure.example.com/live");
    }

    #[test]
    fn stations_carry_their_original_address() {
        let it = my_station_item(&Station { name: "Jazz".into(), url: "http://x/jazz".into(), genre: "Jazz".into() });
        assert_eq!(it.title, "Jazz");
        assert_eq!(it.album, "http://x/jazz");
        assert_eq!(it.source, "mystations");
        assert!(it.is_stream());
        assert_eq!(it.description, "My station · Jazz");
        assert!(it.meta.contains("<dc:title>Jazz</dc:title>"));
    }

    #[test]
    fn directory_results() {
        let v = serde_json::json!([
            {"name": "Jazz FM", "url": "http://a/jazz.pls", "url_resolved": "http://a/jazz.mp3", "country": "UK", "codec": "MP3", "bitrate": 128, "tags": "jazz,smooth", "favicon": "https://a/icon.png"},
            {"name": "Jazz FM", "url": "http://b/dup"},
            {"name": "", "url": "http://c/nameless"},
            {"name": "Talk", "url": "http://d/talk"}
        ]);
        let items = parse(&v);
        assert_eq!(items.len(), 2, "duplicates and nameless stations are skipped");
        assert_eq!(items[0].album, "http://a/jazz.mp3", "the resolved address wins");
        assert_eq!(items[0].description, "Radio · UK · MP3 128k");
        assert_eq!(items[0].genre, "jazz");
        assert_eq!(items[0].art.as_deref(), Some("https://a/icon.png"));
        assert!(parse(&serde_json::json!({"not": "a list"})).is_empty());
    }

    #[test]
    fn streams_inside_playlist_files() {
        assert_eq!(first_stream_in("[playlist]\nNumberOfEntries=1\nFile1=http://a/live\nTitle1=x").as_deref(), Some("http://a/live"));
        assert_eq!(first_stream_in("#EXTM3U\n#EXTINF:-1,Jazz\nhttps://b/jazz.aac\n").as_deref(), Some("https://b/jazz.aac"));
        assert_eq!(first_stream_in(r#"<asx version="3.0"><entry><ref href="http://c/stream"/></entry></asx>"#).as_deref(), Some("http://c/stream"));
        assert_eq!(first_stream_in("nothing here"), None);
    }

    #[test]
    fn non_playlist_addresses_are_left_alone() {
        assert_eq!(resolve_stream("http://a/live.mp3"), "http://a/live.mp3");
        assert_eq!(resolve_stream("http://a/hls.m3u8"), "http://a/hls.m3u8");
    }
}
