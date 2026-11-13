//! DIDL-Lite: the metadata format the speakers use for tracks, favourites and queues.

use super::soap::escape;
use crate::model::Item;

pub fn parse(xml: &str, base: &str) -> Vec<Item> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return vec![];
    };
    doc.root_element().children().filter(|n| n.is_element()).filter(|n| matches!(n.tag_name().name(), "item" | "container")).map(|n| item_from(n, base)).collect()
}

/// The first item of a single-item DIDL document (track metadata).
pub fn parse_one(xml: &str, base: &str) -> Option<Item> {
    if xml.trim().is_empty() || xml == "NOT_IMPLEMENTED" {
        return None;
    }
    parse(xml, base).into_iter().next()
}

fn item_from(n: roxmltree::Node, base: &str) -> Item {
    let mut it = Item {
        id: n.attribute("id").unwrap_or_default().to_string(),
        parent_id: n.attribute("parentID").unwrap_or_default().to_string(),
        is_container: n.tag_name().name() == "container",
        ..Default::default()
    };
    let mut stream_content = String::new();
    for c in n.children().filter(|c| c.is_element()) {
        let text = c.text().unwrap_or_default().trim().to_string();
        match c.tag_name().name() {
            "title" => it.title = text,
            "creator" => {
                if it.artist.is_empty() {
                    it.artist = text
                }
            }
            "albumArtist" => {
                if it.artist.is_empty() {
                    it.artist = text
                }
            }
            "album" => it.album = text,
            "class" => it.class = text,
            "res" => {
                if it.uri.is_empty() {
                    it.uri = text;
                    if let Some(d) = c.attribute("duration") {
                        it.duration = parse_time(d);
                    }
                }
            }
            "albumArtURI" => {
                if it.art.is_none() && !text.is_empty() {
                    it.art = Some(absolute(&text, base));
                }
            }
            "resMD" => it.meta = text,
            "description" => it.description = text,
            "streamContent" => stream_content = text,
            _ => {}
        }
    }
    if !stream_content.is_empty() && !stream_content.starts_with("ZPSTR_") {
        // Some stations send "BR P|TYPE=SNG|TITLE song|ARTIST name|ALBUM record".
        if stream_content.contains("|TYPE=") {
            let field = |key: &str| stream_content.split('|').find_map(|p| p.strip_prefix(key).map(|v| v.trim().to_string())).filter(|v| !v.is_empty());
            if let Some(t) = field("TITLE ") {
                it.album = it.title.clone();
                it.title = t;
                it.artist = field("ARTIST ").unwrap_or_default();
            }
        } else if let Some((a, t)) = stream_content.split_once(" - ") {
            // Radio: "Artist - Title" arrives as stream content.
            it.artist = a.trim().to_string();
            it.album = it.title.clone();
            it.title = t.trim().to_string();
        } else {
            it.album = it.title.clone();
            it.title = stream_content;
        }
    }
    if it.class.contains("container") {
        it.is_container = true;
    }
    it
}

pub fn absolute(url: &str, base: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with('/') {
        format!("{base}{url}")
    } else {
        format!("{base}/{url}")
    }
}

/// "0:03:21" or "00:03:21.000" to seconds.
pub fn parse_time(s: &str) -> f32 {
    let s = s.split('.').next().unwrap_or("");
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.parse::<f32>().unwrap_or(0.0);
    }
    total
}

pub fn format_hms(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

/// Metadata to send with an item that came from the library (favourites carry their own).
pub fn metadata_for(item: &Item) -> String {
    if !item.meta.is_empty() {
        return item.meta.clone();
    }
    let tag = if item.is_container { "container" } else { "item" };
    let class = if item.class.is_empty() { "object.item.audioItem.musicTrack" } else { &item.class };
    let mut extra = String::new();
    if !item.artist.is_empty() {
        extra.push_str(&format!("<dc:creator>{}</dc:creator>", escape(&item.artist)));
    }
    if !item.album.is_empty() && !item.album.starts_with("http") {
        extra.push_str(&format!("<upnp:album>{}</upnp:album>", escape(&item.album)));
    }
    if let Some(art) = item.art.as_ref().filter(|a| a.starts_with("http")) {
        extra.push_str(&format!("<upnp:albumArtURI>{}</upnp:albumArtURI>", escape(art)));
    }
    // Items the speakers fetch from the web carry no service descriptor.
    let desc = if item.uri.starts_with("http") {
        String::new()
    } else {
        "<desc id=\"cdudn\" nameSpace=\"urn:schemas-rinconnetworks-com:metadata-1-0/\">RINCON_AssociatedZPUDN</desc>".to_string()
    };
    let id = if item.id.is_empty() || crate::sources::is_source_id(&item.id) { "-1".to_string() } else { item.id.clone() };
    let parent = if crate::sources::is_source_id(&item.parent_id) { "-1".to_string() } else { item.parent_id.clone() };
    format!(
        "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" \
xmlns:r=\"urn:schemas-rinconnetworks-com:metadata-1-0/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\">\
<{tag} id=\"{}\" parentID=\"{}\" restricted=\"true\"><dc:title>{}</dc:title><upnp:class>{}</upnp:class>{extra}{desc}</{tag}></DIDL-Lite>",
        escape(&id),
        escape(&parent),
        escape(&item.title),
        escape(class)
    )
}

#[cfg(test)]
mod unit {
    use super::*;

    const LIST: &str = r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:r="urn:schemas-rinconnetworks-com:metadata-1-0/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/">
<item id="Q:0/1" parentID="Q:0" restricted="true"><res protocolInfo="x-file-cifs:*:audio/flac:*" duration="0:03:21.000">x-file-cifs://nas/Music/a.flac</res><upnp:albumArtURI>/getaa?u=x</upnp:albumArtURI><dc:title>Morning Fog</dc:title><upnp:class>object.item.audioItem.musicTrack</upnp:class><dc:creator>Marlow Quartet</dc:creator><upnp:album>Low Tide</upnp:album></item>
<container id="A:ALBUM/Low Tide" parentID="A:ALBUM" restricted="true"><dc:title>Low Tide</dc:title><upnp:class>object.container.album.musicAlbum</upnp:class><upnp:albumArtURI>https://art.example/x.jpg</upnp:albumArtURI></container>
</DIDL-Lite>"#;

    #[test]
    fn reads_items_and_containers() {
        let items = parse(LIST, "http://10.0.0.5:1400");
        assert_eq!(items.len(), 2);
        let t = &items[0];
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.album.as_str()), ("Morning Fog", "Marlow Quartet", "Low Tide"));
        assert_eq!(t.uri, "x-file-cifs://nas/Music/a.flac");
        assert_eq!(t.duration, 201.0);
        assert_eq!(t.art.as_deref(), Some("http://10.0.0.5:1400/getaa?u=x"), "relative art becomes absolute");
        assert!(!t.is_container);
        let c = &items[1];
        assert!(c.is_container);
        assert_eq!(c.art.as_deref(), Some("https://art.example/x.jpg"));
    }

    #[test]
    fn radio_stream_content_becomes_title_and_artist() {
        let meta = |sc: &str| {
            format!(
                r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:r="urn:schemas-rinconnetworks-com:metadata-1-0/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"><item id="-1" parentID="-1"><dc:title>Jazz FM</dc:title><r:streamContent>{sc}</r:streamContent></item></DIDL-Lite>"#
            )
        };
        let a = parse_one(&meta("Nina Simone - Feeling Good"), "").unwrap();
        assert_eq!((a.artist.as_str(), a.title.as_str(), a.album.as_str()), ("Nina Simone", "Feeling Good", "Jazz FM"));
        let b = parse_one(&meta("BR P|TYPE=SNG|TITLE Blue in Green|ARTIST Miles Davis|ALBUM Kind of Blue"), "").unwrap();
        assert_eq!((b.title.as_str(), b.artist.as_str()), ("Blue in Green", "Miles Davis"));
        let c = parse_one(&meta("ZPSTR_CONNECTING"), "").unwrap();
        assert_eq!(c.title, "Jazz FM", "status codes are ignored");
    }

    #[test]
    fn empty_and_unsupported_metadata() {
        assert!(parse_one("", "").is_none());
        assert!(parse_one("NOT_IMPLEMENTED", "").is_none());
        assert!(parse("not xml at all", "").is_empty());
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("0:03:21"), 201.0);
        assert_eq!(parse_time("01:00:00.500"), 3600.0);
        assert_eq!(parse_time("3:05"), 185.0);
        assert_eq!(format_hms(3725.0), "1:02:05");
        assert_eq!(format_hms(-5.0), "0:00:00");
    }

    #[test]
    fn addresses() {
        assert_eq!(absolute("http://a/b", "http://x"), "http://a/b");
        assert_eq!(absolute("/b", "http://x"), "http://x/b");
        assert_eq!(absolute("b", "http://x"), "http://x/b");
    }

    #[test]
    fn metadata_written_for_the_speakers() {
        let item = Item {
            id: "A:TRACKS/x".into(),
            parent_id: "A:TRACKS".into(),
            title: "Rock & Roll <live>".into(),
            artist: "Band".into(),
            uri: "x-file-cifs://nas/a.mp3".into(),
            ..Default::default()
        };
        let m = metadata_for(&item);
        assert!(m.contains("Rock &amp; Roll &lt;live&gt;"), "titles are escaped");
        assert!(m.contains("<dc:creator>Band</dc:creator>"));
        assert!(m.contains("RINCON_AssociatedZPUDN"));
        assert!(parse_one(&m, "").is_some(), "what we write, we can read back");
        let web = Item { title: "Ep".into(), uri: "https://cdn.example/ep.mp3".into(), id: "POD:EP/x".into(), ..Default::default() };
        let w = metadata_for(&web);
        assert!(!w.contains("RINCON_AssociatedZPUDN"), "web addresses carry no service descriptor");
        assert!(w.contains("id=\"-1\""), "the app's own ids aren't sent to the speakers");
        let fav = Item { meta: "<DIDL-Lite>kept</DIDL-Lite>".into(), ..Default::default() };
        assert_eq!(metadata_for(&fav), "<DIDL-Lite>kept</DIDL-Lite>");
    }
}
