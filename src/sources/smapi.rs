//! Streaming services through the speakers' own music-service connection (the same
//! service addresses the speakers list for themselves). Signing in happens on the
//! service's own web page; this app then browses and searches with that sign-in, and
//! the speakers play the music with the account already set up on the system.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::config::Account;
use crate::model::Item;
use crate::net::soap::escape;

#[derive(Clone, Debug)]
pub struct MusicService {
    pub sid: u32,
    pub name: String,
    pub uri: String,
    pub auth: String,
    pub manifest: String,
}

pub struct LinkStart {
    pub url: String,
    pub code: String,
    pub device_link_id: String,
}

fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(6)).timeout(Duration::from_secs(20)).build())
}

/// Percent-encode like the speakers do (lower-case hex, nothing left unescaped but letters/digits/-_.~).
pub fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02x}")),
        }
    }
    out
}

enum Fault {
    Refresh(String, String),
    Other(String),
}

fn envelope(device_id: &str, login: Option<(&str, &str, &str)>, action: &str, args: &[(&str, &str)]) -> String {
    let login =
        login.map(|(t, k, h)| format!("<loginToken><token>{}</token><key>{}</key><householdId>{}</householdId></loginToken>", escape(t), escape(k), escape(h))).unwrap_or_default();
    let body: String = args.iter().map(|(k, v)| format!("<{k}>{}</{k}>", escape(v))).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Header>\
<credentials xmlns=\"http://www.sonos.com/Services/1.1\"><deviceId>{}</deviceId><deviceProvider>Sonos</deviceProvider>{login}</credentials>\
</s:Header><s:Body><{action} xmlns=\"http://www.sonos.com/Services/1.1\">{body}</{action}></s:Body></s:Envelope>",
        escape(device_id)
    )
}

fn post(uri: &str, action: &str, envelope: &str) -> Result<String, Fault> {
    let resp = agent()
        .post(uri)
        .set("Content-Type", "text/xml; charset=\"utf-8\"")
        .set("SOAPACTION", &format!("\"http://www.sonos.com/Services/1.1#{action}\""))
        .set("User-Agent", "Linux UPnP/1.0 Sonos/57.23-74170 (ZPS1)")
        .set("Accept-Language", "en-US")
        .send_string(envelope);
    let text = match resp {
        Ok(r) => r.into_string().map_err(|e| Fault::Other(e.to_string()))?,
        Err(ureq::Error::Status(_, r)) => r.into_string().unwrap_or_default(),
        Err(e) => return Err(Fault::Other(e.to_string())),
    };
    let doc = roxmltree::Document::parse(&text).map_err(|e| Fault::Other(format!("unexpected reply ({e})")))?;
    if let Some(fault) = doc.descendants().find(|n| n.tag_name().name() == "Fault") {
        let find = |name: &str| fault.descendants().find(|n| n.tag_name().name() == name).and_then(|n| n.text()).map(|s| s.trim().to_string());
        if let (Some(t), Some(k)) = (find("authToken"), find("privateKey")) {
            return Err(Fault::Refresh(t, k));
        }
        return Err(Fault::Other(find("faultstring").unwrap_or_else(|| "The service refused the request".into())));
    }
    Ok(text)
}

fn field(xml: &str, name: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    doc.descendants().find(|n| n.tag_name().name() == name).and_then(|n| n.text()).map(|s| s.trim().to_string())
}

/// Step 1 of signing in: get the web page and code the person uses to approve this app.
pub fn start_link(svc: &MusicService, household: &str, device_id: &str) -> Result<LinkStart, String> {
    let (action, args): (&str, Vec<(&str, &str)>) = if svc.auth == "AppLink" {
        ("getAppLink", vec![("householdId", household), ("hardware", "Roomtone"), ("osVersion", "macOS"), ("sonosAppName", "Roomtone"), ("callbackPath", "")])
    } else {
        ("getDeviceLinkCode", vec![("householdId", household)])
    };
    let text = post(&svc.uri, action, &envelope(device_id, None, action, &args)).map_err(|f| match f {
        Fault::Other(e) | Fault::Refresh(e, _) => e,
    })?;
    let url = field(&text, "regUrl").ok_or("The service didn't offer a sign-in page")?;
    Ok(LinkStart { url, code: field(&text, "linkCode").unwrap_or_default(), device_link_id: field(&text, "linkDeviceId").unwrap_or_default() })
}

/// Step 2: after the person approves on the web page, collect the sign-in. `Ok(None)` means not yet.
pub fn finish_link(svc: &MusicService, household: &str, device_id: &str, link: &LinkStart) -> Result<Option<(String, String)>, String> {
    let args = [("householdId", household), ("linkCode", link.code.as_str()), ("linkDeviceId", link.device_link_id.as_str())];
    match post(&svc.uri, "getDeviceAuthToken", &envelope(device_id, None, "getDeviceAuthToken", &args)) {
        Ok(text) => match (field(&text, "authToken"), field(&text, "privateKey")) {
            (Some(t), Some(k)) => Ok(Some((t, k))),
            _ => Err("The service didn't return a sign-in".into()),
        },
        Err(Fault::Refresh(t, k)) => Ok(Some((t, k))),
        Err(Fault::Other(e)) if e.contains("NOT_LINKED_RETRY") || e.to_lowercase().contains("retry") => Ok(None),
        Err(Fault::Other(e)) => Err(e),
    }
}

/// Tokens the services hand back when they renew a sign-in, to be saved by the caller.
fn renewed() -> &'static Mutex<HashMap<u32, (String, String)>> {
    static R: OnceLock<Mutex<HashMap<u32, (String, String)>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

pub fn take_renewed() -> Vec<(u32, String, String)> {
    renewed().lock().map(|mut m| m.drain().map(|(k, (t, key))| (k, t, key)).collect()).unwrap_or_default()
}

fn call(acc: &Account, action: &str, args: &[(&str, &str)]) -> Result<String, String> {
    let (mut token, mut key) = (acc.token.clone(), acc.key.clone());
    for _ in 0..2 {
        let env = envelope(&acc.device_id, Some((&token, &key, &acc.household)), action, args);
        match post(&acc.uri, action, &env) {
            Ok(t) => return Ok(t),
            Err(Fault::Refresh(t, k)) => {
                token = t.clone();
                key = k.clone();
                if let Ok(mut m) = renewed().lock() {
                    m.insert(acc.sid, (t, k));
                }
            }
            Err(Fault::Other(e)) => {
                if e.contains("LoginInvalid") || e.contains("AuthTokenExpired") {
                    return Err(format!("{} needs you to sign in again (Music Services)", acc.name));
                }
                return Err(e);
            }
        }
    }
    Err(format!("{} kept asking to renew its sign-in", acc.name))
}

fn text_of(n: roxmltree::Node, name: &str) -> String {
    n.descendants().find(|c| c.tag_name().name() == name).and_then(|c| c.text()).unwrap_or("").trim().to_string()
}

fn desc(acc: &Account) -> String {
    let t = acc.sid * 256 + 7;
    format!("SA_RINCON{t}_X_#Svc{t}-0-Token")
}

fn didl(acc: &Account, tag: &str, id: &str, title: &str, class: &str) -> String {
    format!(
        "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" \
xmlns:r=\"urn:schemas-rinconnetworks-com:metadata-1-0/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\">\
<{tag} id=\"{}\" parentID=\"\" restricted=\"true\"><dc:title>{}</dc:title><upnp:class>{}</upnp:class>\
<desc id=\"cdudn\" nameSpace=\"urn:schemas-rinconnetworks-com:metadata-1-0/\">{}</desc></{tag}></DIDL-Lite>",
        escape(id),
        escape(title),
        escape(class),
        desc(acc)
    )
}

/// Turn one service entry into something the window can show and the speakers can play.
/// The address formats follow the speakers' own conventions (estimate: checked against
/// common services, not every one).
fn to_item(acc: &Account, n: roxmltree::Node) -> Option<Item> {
    let id = text_of(n, "id");
    if id.is_empty() {
        return None;
    }
    let kind = text_of(n, "itemType").to_lowercase();
    let title = text_of(n, "title");
    let is_collection = n.tag_name().name() == "mediaCollection";
    let can_play = text_of(n, "canPlay") != "false";
    let q = quote(&id);
    let sn = acc.sn.max(1);
    let sid = acc.sid;
    let mut item = Item {
        id: format!("SVC:{sid}/{id}"),
        parent_id: format!("SVC:{sid}/"),
        title: title.clone(),
        artist: text_of(n, "artist"),
        album: text_of(n, "album"),
        art: Some(text_of(n, "albumArtURI")).filter(|a| a.starts_with("http")).or_else(|| Some(text_of(n, "logo")).filter(|a| a.starts_with("http"))),
        source: format!("svc:{sid}"),
        is_container: is_collection,
        duration: text_of(n, "duration").parse().unwrap_or(0.0),
        genre: text_of(n, "genre"),
        ..Default::default()
    };
    if is_collection {
        let (prefix, class) = match kind.as_str() {
            "album" => ("1004206c", "object.container.album.musicAlbum"),
            "playlist" | "albumlist" => ("1006206c", "object.container.playlistContainer"),
            "artist" => ("10052064", "object.container.person.musicArtist"),
            _ => ("1006206c", "object.container"),
        };
        item.class = class.into();
        item.description = match kind.as_str() {
            "album" => format!("Album{}", if item.artist.is_empty() { String::new() } else { format!(" · {}", item.artist) }),
            "playlist" => "Playlist".into(),
            "artist" => "Artist".into(),
            "program" | "stream" => "Station".into(),
            _ => String::new(),
        };
        if kind == "program" || kind == "stream" {
            item.class = "object.item.audioItem.audioBroadcast".into();
            item.is_container = false;
            item.uri = format!("x-sonosapi-radio:{q}?sid={sid}&flags=8300&sn={sn}");
            item.meta = didl(acc, "item", &format!("100c206c{q}"), &title, &item.class);
        } else if can_play && kind != "artist" {
            item.uri = format!("x-rincon-cpcontainer:{prefix}{q}?sid={sid}&flags=8300&sn={sn}");
            item.meta = didl(acc, "container", &format!("{prefix}{q}"), &title, class);
        }
        return Some(item);
    }
    match kind.as_str() {
        "stream" | "program" => {
            item.class = "object.item.audioItem.audioBroadcast".into();
            item.uri = format!("x-sonosapi-stream:{q}?sid={sid}&flags=8224&sn={sn}");
            item.meta = didl(acc, "item", &format!("10092020{q}"), &title, &item.class);
            item.description = "Station".into();
        }
        _ => {
            item.class = "object.item.audioItem.musicTrack".into();
            let mime = text_of(n, "mimeType").to_lowercase();
            item.uri = if sid == 9 {
                format!("x-sonos-spotify:{q}?sid={sid}&flags=8224&sn={sn}")
            } else if mime.contains("mpegurl") {
                format!("x-sonosapi-hls-static:{q}?sid={sid}&flags=8232&sn={sn}")
            } else {
                let ext = if mime.contains("mp4") || mime.contains("aac") || mime.contains("m4a") {
                    ".mp4"
                } else if mime.contains("flac") {
                    ".flac"
                } else if mime.contains("mpeg") || mime.contains("mp3") {
                    ".mp3"
                } else {
                    ""
                };
                format!("x-sonos-http:{q}{ext}?sid={sid}&flags=8224&sn={sn}")
            };
            let prefix = if sid == 9 { "00032020" } else { "10032020" };
            item.meta = didl(acc, "item", &format!("{prefix}{q}"), &title, &item.class);
        }
    }
    Some(item)
}

fn items_from(acc: &Account, xml: &str) -> Vec<Item> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return vec![] };
    doc.descendants().filter(|n| matches!(n.tag_name().name(), "mediaCollection" | "mediaMetadata")).filter_map(|n| to_item(acc, n)).collect()
}

/// Open a folder of the service ("root" for its home).
pub fn browse(acc: &Account, id: &str) -> Result<Vec<Item>, String> {
    let mut out = vec![];
    let mut index = 0usize;
    loop {
        let text = call(acc, "getMetadata", &[("id", id), ("index", &index.to_string()), ("count", "100"), ("recursive", "false")])?;
        let page = items_from(acc, &text);
        let total: usize = field(&text, "total").and_then(|t| t.parse().ok()).unwrap_or(0);
        let n = page.len();
        out.extend(page);
        index += n;
        if n == 0 || index >= total || index >= 600 {
            break;
        }
    }
    Ok(out)
}

/// The service's search categories (e.g. artists, albums, tracks), from its published presentation map.
fn categories(acc: &Account) -> Vec<String> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Vec<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(c) = cache.lock().ok().and_then(|m| m.get(&acc.sid).cloned()) {
        return c;
    }
    let mut cats = vec![];
    let map_uri = agent()
        .get(&acc.manifest)
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok())
        .and_then(|v| v.pointer("/presentationMap/uri").and_then(|u| u.as_str()).map(String::from));
    if let Some(map) = map_uri.and_then(|u| agent().get(&u).call().ok()).and_then(|r| r.into_string().ok()) {
        if let Ok(doc) = roxmltree::Document::parse(&map) {
            for c in doc.descendants().filter(|n| matches!(n.tag_name().name(), "Category" | "CustomCategory")) {
                if let Some(m) = c.attribute("mappedId") {
                    if !cats.iter().any(|x| x == m) {
                        cats.push(m.to_string());
                    }
                }
            }
        }
    }
    if cats.is_empty() {
        cats = ["artists", "albums", "tracks", "playlists"].iter().map(|s| s.to_string()).collect();
    }
    if let Ok(mut m) = cache.lock() {
        m.insert(acc.sid, cats.clone());
    }
    cats
}

/// What a service has for an artist: find the artist by name, then open it.
pub fn artist_content(acc: &Account, name: &str) -> Result<Vec<Item>, String> {
    let cats = categories(acc);
    let cat = cats.iter().find(|c| c.to_lowercase().contains("artist")).cloned().unwrap_or_else(|| "artists".into());
    let text = call(acc, "search", &[("id", &cat), ("term", name), ("index", "0"), ("count", "10")])?;
    let found = items_from(acc, &text);
    let artist = found.iter().find(|i| i.title.eq_ignore_ascii_case(name)).or(found.first());
    match artist {
        Some(a) if a.is_container => {
            let inner = a.id.strip_prefix(&format!("SVC:{}/", acc.sid)).unwrap_or(&a.id).to_string();
            browse(acc, &inner)
        }
        _ => Ok(vec![]),
    }
}

pub fn search(acc: &Account, term: &str) -> Result<Vec<Item>, String> {
    let mut out = vec![];
    let mut last_err = None;
    for cat in categories(acc).iter().take(6) {
        match call(acc, "search", &[("id", cat), ("term", term), ("index", "0"), ("count", "25")]) {
            Ok(text) => out.extend(items_from(acc, &text)),
            Err(e) => last_err = Some(e),
        }
    }
    if out.is_empty() {
        if let Some(e) = last_err {
            return Err(e);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// Asks each streaming service for a sign-in page (nothing is linked):
    /// `ROOMTONE_SPEAKER=<ip> cargo test link_pages -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn link_pages() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else { return };
        let list = crate::net::speaker::music_services(&ip).unwrap();
        let household = crate::net::soap::call(&ip, crate::net::soap::DEVICE_PROPS, "GetHouseholdID", &[]).unwrap()["CurrentHouseholdID"].clone();
        let device = crate::net::speaker::device_id(&ip).unwrap_or_else(|| "test".into());
        println!("{} services, device id found: {}", list.len(), device != "test");
        for sid in [9, 204, 201, 284, 2, 174, 160, 31] {
            let Some(svc) = list.iter().find(|m| m.sid == sid) else {
                println!("{sid}: not offered");
                continue;
            };
            match super::start_link(svc, &household, &device) {
                Ok(l) => println!("{}: sign-in page ok ({}), code shown: {}", svc.name, l.url.split('?').next().unwrap_or(""), !l.code.is_empty()),
                Err(e) => println!("{}: {e}", svc.name),
            }
        }
        let fav_sn: Vec<String> = [9u32, 204, 201, 2].iter().map(|s| format!("{s}->{:?}", crate::net::speaker::account_number(&ip, *s))).collect();
        println!("account numbers from favorites: {}", fav_sn.join(" "));
    }
}

#[cfg(test)]
mod debug_tests {
    #[test]
    #[ignore]
    fn raw_replies() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else { return };
        let list = crate::net::speaker::music_services(&ip).unwrap();
        let household = crate::net::soap::call(&ip, crate::net::soap::DEVICE_PROPS, "GetHouseholdID", &[]).unwrap()["CurrentHouseholdID"].clone();
        let device = crate::net::speaker::device_id(&ip).unwrap();
        for sid in [204, 284, 160] {
            let svc = list.iter().find(|m| m.sid == sid).unwrap();
            let env = super::envelope(
                &device,
                None,
                "getAppLink",
                &[("householdId", &household), ("hardware", "Roomtone"), ("osVersion", "macOS"), ("sonosAppName", "Roomtone"), ("callbackPath", "")],
            );
            let r = super::agent()
                .post(&svc.uri)
                .set("Content-Type", "text/xml; charset=\"utf-8\"")
                .set("SOAPACTION", "\"http://www.sonos.com/Services/1.1#getAppLink\"")
                .set("User-Agent", "Linux UPnP/1.0 Sonos/57.23-74170 (ZPS1)")
                .send_string(&env);
            let (code, body) = match r {
                Ok(r) => (r.status(), r.into_string().unwrap_or_default()),
                Err(ureq::Error::Status(c, r)) => (c, r.into_string().unwrap_or_default()),
                Err(e) => (0, e.to_string()),
            };
            let body = body.replace(&household, "<HH>").replace(&device, "<DEV>");
            println!("== {} {} ==\n{}\n", svc.name, code, body.chars().take(900).collect::<String>());
        }
    }
}

#[cfg(test)]
mod deep_tests {
    /// Read-only: home, one level down and a search on each connected service.
    /// `cargo test service_deep -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn service_deep() {
        let cfg = crate::config::Config::load_readonly();
        let term = std::env::var("ROOMTONE_TERM").unwrap_or_else(|_| "Adele".into());
        for acc in &cfg.accounts {
            match super::browse(acc, "root") {
                Ok(home) => {
                    println!("{}: home has {} entries: {:?}", acc.name, home.len(), home.iter().take(8).map(|i| i.title.clone()).collect::<Vec<_>>());
                    if let Some(first) = home.iter().find(|i| i.is_container) {
                        let inner = first.id.strip_prefix(&format!("SVC:{}/", acc.sid)).unwrap_or(&first.id).to_string();
                        match super::browse(acc, &inner) {
                            Ok(v) => println!("  “{}” has {} entries; playable: {}", first.title, v.len(), v.iter().filter(|i| !i.uri.is_empty()).count()),
                            Err(e) => println!("  “{}”: {e}", first.title),
                        }
                    }
                }
                Err(e) => println!("{}: home failed: {e}", acc.name),
            }
            println!("  search categories: {:?}", super::categories(acc));
            match super::search(acc, &term) {
                Ok(v) => {
                    println!("  search “{term}”: {} results", v.len());
                    for i in v.iter().take(5) {
                        println!("    {} | {} | {}", i.title, i.class, i.uri.split('?').next().unwrap_or(""));
                    }
                }
                Err(e) => println!("  search failed: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod home_tests {
    /// Read-only: open each connected service's home. `cargo test service_home -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn service_home() {
        let cfg = crate::config::Config::load_readonly();
        for acc in &cfg.accounts {
            let t = std::time::Instant::now();
            let env = super::envelope(
                &acc.device_id,
                Some((&acc.token, &acc.key, &acc.household)),
                "getMetadata",
                &[("id", "root"), ("index", "0"), ("count", "100"), ("recursive", "false")],
            );
            match super::post(&acc.uri, "getMetadata", &env) {
                Ok(text) => {
                    let items = super::items_from(acc, &text);
                    println!("{}: {} items in {:?}: {:?}", acc.name, items.len(), t.elapsed(), items.iter().take(6).map(|i| i.title.clone()).collect::<Vec<_>>());
                    if items.is_empty() {
                        println!("raw: {}", text.chars().take(700).collect::<String>());
                    }
                }
                Err(super::Fault::Refresh(..)) => println!("{}: token refresh requested", acc.name),
                Err(super::Fault::Other(e)) => println!("{}: error {e} after {:?}", acc.name, t.elapsed()),
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn spotify() -> Account {
        Account { sid: 9, name: "Spotify".into(), sn: 3, ..Default::default() }
    }

    const REPLY: &str = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><getMetadataResponse xmlns="http://www.sonos.com/Services/1.1"><getMetadataResult>
<index>0</index><count>5</count><total>5</total>
<mediaCollection><id>album:1</id><itemType>album</itemType><title>Blue</title><artist>Joni</artist><canPlay>true</canPlay><albumArtURI>https://i/1.jpg</albumArtURI></mediaCollection>
<mediaCollection><id>playlist:2</id><itemType>playlist</itemType><title>Focus</title></mediaCollection>
<mediaCollection><id>artist:3</id><itemType>artist</itemType><title>Joni</title></mediaCollection>
<mediaCollection><id>station:4</id><itemType>program</itemType><title>Radio Joni</title></mediaCollection>
<mediaMetadata><id>track:5</id><itemType>track</itemType><title>River</title><mimeType>audio/x-spotify</mimeType><trackMetadata><artist>Joni</artist><duration>240</duration></trackMetadata></mediaMetadata>
</getMetadataResult></getMetadataResponse></s:Body></s:Envelope>"#;

    #[test]
    fn percent_encoding_matches_the_speakers() {
        assert_eq!(quote("spotify:track:1"), "spotify%3atrack%3a1");
        assert_eq!(quote("a b/c"), "a%20b%2fc");
        assert_eq!(quote("Az09-_.~"), "Az09-_.~");
    }

    #[test]
    fn service_entries_become_playable_items() {
        let items = items_from(&spotify(), REPLY);
        assert_eq!(items.len(), 5);
        let album = &items[0];
        assert!(album.is_container);
        assert_eq!(album.class, "object.container.album.musicAlbum");
        assert!(album.uri.starts_with("x-rincon-cpcontainer:1004206calbum%3a1?sid=9"));
        assert!(album.uri.ends_with("&sn=3"), "uses the system's account number");
        assert_eq!(album.id, "SVC:9/album:1");
        let playlist = &items[1];
        assert!(playlist.uri.starts_with("x-rincon-cpcontainer:1006206c"));
        let artist = &items[2];
        assert!(artist.uri.is_empty(), "artists are opened, not played");
        let station = &items[3];
        assert!(station.uri.starts_with("x-sonosapi-radio:"));
        assert!(!station.is_container);
        let track = &items[4];
        assert!(track.uri.starts_with("x-sonos-spotify:track%3a5?sid=9"));
        assert!(track.meta.contains("SA_RINCON2311_X_#Svc2311-0-Token"), "9 × 256 + 7 = 2311");
        assert_eq!(track.source, "svc:9");
    }

    #[test]
    fn other_services_use_http_tracks() {
        let deezer = Account { sid: 2, name: "Deezer".into(), sn: 1, ..Default::default() };
        let xml = r#"<r><mediaMetadata><id>t1</id><itemType>track</itemType><title>T</title><mimeType>audio/mp4</mimeType></mediaMetadata></r>"#;
        let items = items_from(&deezer, xml);
        assert!(items[0].uri.starts_with("x-sonos-http:t1.mp4?sid=2"));
        assert!(items_from(&deezer, "not xml").is_empty());
    }

    #[test]
    fn reading_fields() {
        assert_eq!(field("<a><regUrl>https://x/link</regUrl><linkCode>ABCD</linkCode></a>", "linkCode").as_deref(), Some("ABCD"));
        assert_eq!(field("<a/>", "linkCode"), None);
    }

    #[test]
    fn sign_in_requests_name_this_app_and_household() {
        let env = envelope("DEV1", None, "getDeviceLinkCode", &[("householdId", "HH<1>")]);
        assert!(env.contains("<deviceId>DEV1</deviceId>"));
        assert!(env.contains("<householdId>HH&lt;1&gt;</householdId>"), "values are escaped");
        assert!(!env.contains("loginToken"));
        let signed_in = envelope("DEV1", Some(("tok", "key", "HH")), "getMetadata", &[]);
        assert!(signed_in.contains("<loginToken><token>tok</token><key>key</key><householdId>HH</householdId></loginToken>"));
    }
}
