//! The Internet Archive's free audio collections (live concerts, old recordings,
//! public-domain music). No account needed.

use std::time::Duration;

use crate::model::Item;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(15)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build()
}

fn text_of(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a.first().map(text_of).unwrap_or_default(),
        _ => String::new(),
    }
}

fn query(q: &str) -> Result<Vec<Item>, String> {
    let resp = agent()
        .get("https://archive.org/advancedsearch.php")
        .query("q", q)
        .query("fl[]", "identifier")
        .query("fl[]", "title")
        .query("fl[]", "creator")
        .query("fl[]", "subject")
        .query("sort[]", "downloads desc")
        .query("rows", "40")
        .query("output", "json")
        .call()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let docs = v.pointer("/response/docs").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    Ok(docs
        .iter()
        .filter_map(|d| {
            let id = text_of(d.get("identifier")?);
            let title = text_of(d.get("title").unwrap_or(&serde_json::Value::Null));
            let creator = text_of(d.get("creator").unwrap_or(&serde_json::Value::Null));
            let genre = text_of(d.get("subject").unwrap_or(&serde_json::Value::Null));
            Some(Item {
                id: format!("IA:ITEM/{id}"),
                parent_id: "IA:".into(),
                title: if title.is_empty() { id.clone() } else { title.clone() },
                artist: creator.clone(),
                album: title,
                description: if creator.is_empty() { "Internet Archive".into() } else { format!("Album · {creator}") },
                class: "object.container.album.musicAlbum".into(),
                is_container: true,
                art: Some(format!("https://archive.org/services/img/{id}")),
                source: "archive".into(),
                genre: genre.split([';', ',']).next().unwrap_or("").trim().to_string(),
                ..Default::default()
            })
        })
        .collect())
}

pub fn featured() -> Result<Vec<Item>, String> {
    query("collection:(etree OR audio_music) AND mediatype:(audio)")
}

pub fn search(term: &str) -> Result<Vec<Item>, String> {
    let clean: String = term.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    query(&format!("({clean}) AND mediatype:(audio)"))
}

pub fn tracks(identifier: &str) -> Result<Vec<Item>, String> {
    let v: serde_json::Value = agent().get(&format!("https://archive.org/metadata/{identifier}")).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let album = v.pointer("/metadata/title").map(text_of).unwrap_or_else(|| identifier.to_string());
    let creator = v.pointer("/metadata/creator").map(text_of).unwrap_or_default();
    let files = v.get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default();
    let mut out = vec![];
    for f in files {
        let name = f.get("name").map(text_of).unwrap_or_default();
        let format = f.get("format").map(text_of).unwrap_or_default();
        if !(format.contains("MP3") && !format.contains("64Kbps")) {
            continue;
        }
        let title = f.get("title").map(text_of).filter(|t| !t.is_empty()).unwrap_or_else(|| name.trim_end_matches(".mp3").to_string());
        let duration = f.get("length").map(text_of).map(|l| if l.contains(':') { crate::net::didl::parse_time(&l) } else { l.parse().unwrap_or(0.0) }).unwrap_or(0.0);
        let track_no = f.get("track").map(text_of).and_then(|t| t.split('/').next().and_then(|n| n.trim().parse().ok())).unwrap_or(0);
        let path: String = name.split('/').map(encode).collect::<Vec<_>>().join("/");
        out.push(Item {
            id: format!("IA:T/{identifier}/{name}"),
            parent_id: format!("IA:ITEM/{identifier}"),
            title,
            artist: creator.clone(),
            album: album.clone(),
            class: "object.item.audioItem.musicTrack".into(),
            uri: format!("https://archive.org/download/{identifier}/{path}"),
            art: Some(format!("https://archive.org/services/img/{identifier}")),
            duration,
            source: "archive".into(),
            track_no,
            ..Default::default()
        });
    }
    out.sort_by_key(|i| i.track_no);
    Ok(out)
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
