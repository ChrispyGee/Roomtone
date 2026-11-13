//! Music sources beyond the speakers' own library: folders on this computer,
//! internet radio, podcasts and the Internet Archive. Each gives items the speakers
//! can play directly, so S1 and S2 systems both work.

pub mod archive;
pub mod local;
pub mod memory;
pub mod podcasts;
pub mod radio;
pub mod smapi;

use crate::config::Config;
use crate::model::Item;

pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Every place search can look, in the order results are shown.
pub const SEARCH_SOURCES: &[&str] = &["library", "favorites", "playlists", "local", "mystations", "radio", "podcasts", "archive"];

pub fn source_label(id: &str) -> &'static str {
    if let Some(sid) = id.strip_prefix("svc:").and_then(|x| x.parse::<u32>().ok()) {
        return SERVICES.iter().find(|s| s.sid == sid).map(|s| s.name).unwrap_or(crate::i18n::tr(n_!("Streaming service")));
    }
    match id {
        "library" => crate::i18n::tr(n_!("System library")),
        "favorites" => crate::i18n::tr(n_!("Favorites")),
        "playlists" => crate::i18n::tr(n_!("Playlists")),
        "local" => crate::i18n::tr(n_!("This computer")),
        "mystations" => crate::i18n::tr(n_!("My stations")),
        "radio" => crate::i18n::tr(n_!("Radio directory")),
        "podcasts" => crate::i18n::tr(n_!("Podcasts")),
        "archive" => crate::i18n::tr(n_!("Internet Archive")),
        _ => crate::i18n::tr(n_!("Other")),
    }
}

/// Is this source switched on (the speakers' own lists always are)?
pub fn enabled(config: &Config, id: &str) -> bool {
    if let Some(sid) = id.strip_prefix("svc:").and_then(|x| x.parse::<u32>().ok()) {
        return config.account(sid).is_some();
    }
    match id {
        "local" | "radio" | "podcasts" | "archive" => config.service_on(id),
        _ => true,
    }
}

pub fn is_source_id(id: &str) -> bool {
    ["LOCAL:", "RADIO:", "POD:", "IA:", "SVC:", "ARTIST:"].iter().any(|p| id.starts_with(p))
}

pub struct Service {
    pub id: &'static str,
    /// The speakers' number for this service (0 for the free sources built into this app).
    pub sid: u32,
    pub name: &'static str,
    pub blurb: &'static str,
    pub ready: bool,
    pub note: &'static str,
}

pub const SERVICES: &[Service] = &[
    Service { id: "local", sid: 0, name: n_!("This computer"), blurb: n_!("Music folders on this Mac, or drives and network shares mounted on it"), ready: true, note: "" },
    Service {
        id: "radio",
        sid: 0,
        name: n_!("Radio directory"),
        blurb: n_!("Tens of thousands of free internet radio stations from a community directory"),
        ready: true,
        note: "",
    },
    Service { id: "podcasts", sid: 0, name: n_!("Podcasts"), blurb: n_!("Search any public podcast, subscribe and play episodes"), ready: true, note: "" },
    Service { id: "archive", sid: 0, name: n_!("Internet Archive"), blurb: n_!("Free live concerts, old recordings and public-domain music"), ready: true, note: "" },
    Service { id: "svc:9", sid: 9, name: "Spotify", blurb: n_!("Songs, albums, playlists and podcasts"), ready: false, note: "" },
    Service {
        id: "svc:204",
        sid: 204,
        name: "Apple Music",
        blurb: n_!("Songs, albums, playlists and radio"),
        ready: false,
        note: n_!("Apple only lets the speaker maker's own app sign in. Save Apple Music albums, playlists and stations to Favorites there and they play here."),
    },
    Service { id: "svc:201", sid: 201, name: "Amazon Music", blurb: n_!("Songs, albums, playlists and stations"), ready: false, note: "" },
    Service {
        id: "svc:284",
        sid: 284,
        name: "YouTube Music",
        blurb: n_!("Songs, albums and mixes"),
        ready: false,
        note: n_!("Google only lets the speaker maker's own app sign in. Save YouTube Music items to Favorites there and they play here."),
    },
    Service { id: "svc:2", sid: 2, name: "Deezer", blurb: n_!("Songs, albums, playlists and Flow"), ready: false, note: "" },
    Service { id: "svc:174", sid: 174, name: "TIDAL", blurb: n_!("High-quality streaming"), ready: false, note: "" },
    Service {
        id: "svc:160",
        sid: 160,
        name: "SoundCloud",
        blurb: n_!("Independent artists and mixes"),
        ready: false,
        note: n_!("SoundCloud only lets the speaker maker's own app sign in. Save SoundCloud items to Favorites there and they play here."),
    },
    Service { id: "svc:31", sid: 31, name: "Qobuz", blurb: n_!("Hi-res streaming"), ready: false, note: "" },
];

/// Does this saved item come from the streaming service with this number?
pub fn from_service(item: &Item, sid: u32) -> bool {
    let u = item.uri.replace("&amp;", "&");
    let tag = format!("sid={sid}");
    let in_uri = u.match_indices(&tag).any(|(i, _)| !u[i + tag.len()..].starts_with(|c: char| c.is_ascii_digit()));
    in_uri || item.meta.contains(&format!("SA_RINCON{}_", sid * 256 + 7))
}

/// The streaming service a saved item comes from, if any.
pub fn service_of_item(item: &Item) -> Option<&'static Service> {
    SERVICES.iter().filter(|s| s.sid > 0).find(|s| from_service(item, s.sid))
}

/// The list of favorites saved from one service ("FVSVC:204").
pub fn favorites_id(sid: u32) -> String {
    format!("FVSVC:{sid}")
}

/// Open a source container (album, folder, podcast, station list...).
pub fn browse(id: &str, config: &Config) -> Result<Vec<Item>, String> {
    if let Some(rest) = id.strip_prefix("SVC:") {
        let (sid, inner) = rest.split_once('/').unwrap_or((rest, ""));
        let sid: u32 = sid.parse().map_err(|_| "Unknown service".to_string())?;
        let acc = config.account(sid).ok_or("That service isn't connected")?;
        return smapi::browse(acc, if inner.is_empty() { "root" } else { inner });
    }
    if id.starts_with("LOCAL:") {
        return Ok(local::browse(id, &config.folders));
    }
    if let Some(rest) = id.strip_prefix("RADIO:") {
        return match rest {
            "MINE" => Ok(config.stations.iter().map(radio::my_station_item).collect()),
            "TOP" => radio::top(),
            other => match other.strip_prefix("TAG/") {
                Some(tag) => radio::by_tag(tag),
                None => Ok(vec![]),
            },
        };
    }
    if let Some(rest) = id.strip_prefix("POD:") {
        return match rest {
            "SUBS" => Ok(config.podcasts.iter().map(podcasts::show_item).collect()),
            other => match other.strip_prefix("FEED/") {
                Some(feed) => podcasts::episodes(feed),
                None => Ok(vec![]),
            },
        };
    }
    if let Some(rest) = id.strip_prefix("IA:") {
        return match rest {
            "FEATURED" => archive::featured(),
            other => match other.strip_prefix("ITEM/") {
                Some(ident) => archive::tracks(ident),
                None => Ok(vec![]),
            },
        };
    }
    Ok(vec![])
}

/// Search one non-speaker source.
pub fn search(source: &str, term: &str, config: &Config) -> Result<Vec<Item>, String> {
    let t = term.to_lowercase();
    if let Some(sid) = source.strip_prefix("svc:").and_then(|x| x.parse::<u32>().ok()) {
        let acc = config.account(sid).ok_or("That service isn't connected")?;
        return smapi::search(acc, term);
    }
    match source {
        "local" => Ok(local::search(term)),
        "mystations" => Ok(config.stations.iter().filter(|s| s.name.to_lowercase().contains(&t) || s.genre.to_lowercase().contains(&t)).map(radio::my_station_item).collect()),
        "radio" => radio::search(term),
        "podcasts" => {
            let mut subs: Vec<Item> = config.podcasts.iter().filter(|p| p.title.to_lowercase().contains(&t)).map(podcasts::show_item).collect();
            for found in podcasts::search(term)? {
                if !subs.iter().any(|s| s.id == found.id) {
                    subs.push(found);
                }
            }
            Ok(subs)
        }
        "archive" => archive::search(term),
        _ => Ok(vec![]),
    }
}

/// Everything playable inside a source container, for "Play" and "Add to queue".
pub fn tracks_for(item: &Item, config: &Config) -> Result<Vec<Item>, String> {
    if !item.is_container || item.id.starts_with("SVC:") && !item.uri.is_empty() {
        return Ok(vec![item.clone()]);
    }
    let mut out = vec![];
    let mut todo = vec![item.id.clone()];
    while let Some(id) = todo.pop() {
        for child in browse(&id, config)? {
            if child.is_container {
                if todo.len() < 200 {
                    todo.insert(0, child.id.clone());
                }
            } else if !child.uri.is_empty() {
                out.push(child);
            }
            if out.len() >= 500 {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// Every source search can look in: the fixed ones plus each connected streaming service.
pub fn all_search_sources(config: &Config) -> Vec<String> {
    let mut v: Vec<String> = SEARCH_SOURCES.iter().map(|s| s.to_string()).collect();
    v.extend(config.accounts.iter().map(|a| format!("svc:{}", a.sid)));
    v
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn hashing_is_stable() {
        assert_eq!(hash("abc"), hash("abc"));
        assert_ne!(hash("abc"), hash("abd"));
    }

    #[test]
    fn source_ids_and_labels() {
        crate::test_support::isolate();
        for id in ["LOCAL:ALBUMS", "RADIO:TOP", "POD:SUBS", "IA:FEATURED", "SVC:9/root", "ARTIST:Nova"] {
            assert!(is_source_id(id), "{id}");
        }
        assert!(!is_source_id("A:ALBUM"));
        assert_eq!(source_label("local"), "This computer");
        assert_eq!(source_label("svc:9"), "Spotify");
        assert_eq!(source_label("svc:99999"), "Streaming service");
        assert_eq!(favorites_id(204), "FVSVC:204");
    }

    #[test]
    fn which_service_a_favorite_comes_from() {
        let apple = Item { uri: "x-rincon-cpcontainer:1006206cplaylist?sid=204&flags=8300&sn=2".into(), ..Default::default() };
        assert!(from_service(&apple, 204));
        assert!(!from_service(&apple, 20), "sid=204 isn't sid=20");
        assert_eq!(service_of_item(&apple).map(|s| s.name), Some("Apple Music"));
        let by_desc = Item { meta: "<desc>SA_RINCON52231_X_#Svc52231-0-Token</desc>".into(), ..Default::default() };
        assert!(from_service(&by_desc, 204), "204 × 256 + 7 = 52231");
        let plain = Item { uri: "x-file-cifs://nas/a.mp3".into(), ..Default::default() };
        assert!(service_of_item(&plain).is_none());
    }

    #[test]
    fn switched_on_sources_and_search_places() {
        let mut c = Config::default();
        assert!(enabled(&c, "radio"));
        c.services.retain(|s| s != "radio");
        assert!(!enabled(&c, "radio"));
        assert!(enabled(&c, "favorites"), "the speakers' own lists are always on");
        assert!(!enabled(&c, "svc:9"));
        c.accounts.push(crate::config::Account { sid: 9, name: "Spotify".into(), ..Default::default() });
        assert!(enabled(&c, "svc:9"));
        assert!(all_search_sources(&c).contains(&"svc:9".to_string()));
    }

    #[test]
    fn services_list_is_consistent() {
        let mut ids: Vec<&str> = SERVICES.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SERVICES.len(), "service ids are unique");
        for s in SERVICES.iter().filter(|s| s.sid > 0) {
            assert_eq!(s.id, format!("svc:{}", s.sid));
        }
    }

    #[test]
    fn a_single_item_plays_as_itself() {
        let song = Item { uri: "x-file-cifs://a".into(), title: "A".into(), ..Default::default() };
        let tracks = tracks_for(&song, &Config::default()).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "A");
    }
}
