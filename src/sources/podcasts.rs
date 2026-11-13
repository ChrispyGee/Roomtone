//! Podcasts: found through the free public podcast directory search (no account),
//! episodes read straight from each show's feed.

use std::time::Duration;

use crate::config::Podcast;
use crate::model::Item;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(12)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build()
}

pub fn show_item(p: &Podcast) -> Item {
    Item {
        id: format!("POD:FEED/{}", p.feed),
        parent_id: "POD:SUBS".into(),
        title: p.title.clone(),
        artist: p.author.clone(),
        description: if p.author.is_empty() { "Podcast".into() } else { format!("Podcast · {}", p.author) },
        class: "object.container.playlistContainer".into(),
        is_container: true,
        art: (!p.art.is_empty()).then(|| p.art.clone()),
        source: "podcasts".into(),
        ..Default::default()
    }
}

pub fn podcast_of(item: &Item) -> Option<Podcast> {
    let feed = item.id.strip_prefix("POD:FEED/")?;
    Some(Podcast { title: item.title.clone(), author: item.artist.clone(), feed: feed.to_string(), art: item.art.clone().unwrap_or_default() })
}

pub fn search(term: &str) -> Result<Vec<Item>, String> {
    let resp = agent()
        .get("https://itunes.apple.com/search")
        .query("media", "podcast")
        .query("entity", "podcast")
        .query("limit", "30")
        .query("term", term)
        .call()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let mut out = vec![];
    for r in v.get("results").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
        let get = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let feed = get("feedUrl");
        if feed.is_empty() {
            continue;
        }
        let mut item = show_item(&Podcast { title: get("collectionName"), author: get("artistName"), feed, art: get("artworkUrl600") });
        item.genre = get("primaryGenreName");
        out.push(item);
    }
    Ok(out)
}

pub fn episodes(feed: &str) -> Result<Vec<Item>, String> {
    let text = agent().get(feed).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    parse_feed(&text, feed)
}

/// The episodes in a podcast's RSS feed (newest first, as the feed lists them; at most 200).
pub(crate) fn parse_feed(text: &str, feed: &str) -> Result<Vec<Item>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("feed: {e}"))?;
    let show = doc
        .descendants()
        .find(|n| n.tag_name().name() == "channel")
        .and_then(|c| c.children().find(|n| n.tag_name().name() == "title"))
        .and_then(|n| n.text())
        .unwrap_or("Podcast")
        .to_string();
    let show_art = doc.descendants().find(|n| n.tag_name().name() == "image" && n.attribute("href").is_some()).and_then(|n| n.attribute("href")).map(String::from);
    let mut out = vec![];
    for it in doc.descendants().filter(|n| n.tag_name().name() == "item").take(200) {
        let child = |name: &str| it.children().find(|n| n.tag_name().name() == name);
        let Some(url) = child("enclosure").and_then(|e| e.attribute("url")) else { continue };
        let title = child("title").and_then(|n| n.text()).unwrap_or("Episode").trim().to_string();
        let date = child("pubDate").and_then(|n| n.text()).unwrap_or("").trim();
        let short_date: String = date.split_whitespace().skip(1).take(3).collect::<Vec<_>>().join(" ");
        let duration = child("duration").and_then(|n| n.text()).map(|d| if d.contains(':') { crate::net::didl::parse_time(d) } else { d.trim().parse::<f32>().unwrap_or(0.0) });
        let art = it.children().find(|n| n.tag_name().name() == "image").and_then(|n| n.attribute("href")).map(String::from).or_else(|| show_art.clone());
        out.push(Item {
            id: format!("POD:EP/{url}"),
            parent_id: format!("POD:FEED/{feed}"),
            title,
            artist: show.clone(),
            album: show.clone(),
            description: if short_date.is_empty() { "Episode".into() } else { format!("Episode · {short_date}") },
            class: "object.item.audioItem.musicTrack".into(),
            uri: url.to_string(),
            art,
            duration: duration.unwrap_or(0.0),
            source: "podcasts".into(),
            ..Default::default()
        });
    }
    Ok(out)
}

#[cfg(test)]
mod unit {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0"?><rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
<title>Sample Show</title><itunes:image href="https://img.example/show.jpg"/>
<item><title>Episode Two</title><pubDate>Tue, 10 Nov 2026 06:00:00 GMT</pubDate><enclosure url="https://cdn.example/2.mp3" type="audio/mpeg"/><itunes:duration>1:02:03</itunes:duration></item>
<item><title>Episode One</title><enclosure url="https://cdn.example/1.mp3"/><itunes:duration>754</itunes:duration><itunes:image href="https://img.example/1.jpg"/></item>
<item><title>No audio</title></item>
</channel></rss>"#;

    #[test]
    fn reads_episodes() {
        let eps = parse_feed(FEED, "https://feed.example/rss").unwrap();
        assert_eq!(eps.len(), 2, "items without audio are skipped");
        assert_eq!(eps[0].title, "Episode Two");
        assert_eq!(eps[0].artist, "Sample Show");
        assert_eq!(eps[0].duration, 3723.0);
        assert_eq!(eps[0].description, "Episode · 10 Nov 2026");
        assert_eq!(eps[0].art.as_deref(), Some("https://img.example/show.jpg"), "the show's art when the episode has none");
        assert_eq!(eps[1].duration, 754.0);
        assert_eq!(eps[1].art.as_deref(), Some("https://img.example/1.jpg"));
        assert_eq!(eps[1].parent_id, "POD:FEED/https://feed.example/rss");
        assert!(parse_feed("<<", "x").is_err());
    }

    #[test]
    fn shows_round_trip() {
        let p = Podcast { title: "Show".into(), author: "Host".into(), feed: "https://f/rss".into(), art: String::new() };
        let item = show_item(&p);
        assert!(item.is_container);
        assert_eq!(item.description, "Podcast · Host");
        assert_eq!(podcast_of(&item), Some(p));
        assert!(podcast_of(&Item::default()).is_none());
    }
}
