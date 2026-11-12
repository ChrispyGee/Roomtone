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
    for line in text.lines() {
        let line = line.trim();
        let candidate = line.split_once('=').filter(|(k, _)| k.to_lowercase().starts_with("file")).map(|(_, v)| v).unwrap_or(line);
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            return candidate.to_string();
        }
    }
    url.to_string()
}
