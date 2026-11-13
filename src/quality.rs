//! What the speakers tell us about the sound of what's playing: the format, and the sample
//! rate, bit depth and bitrate when they say so.
//!
//! Nothing is asked of the speakers for this. It's read from details already fetched with
//! each poll (the track's address and its DIDL-Lite description), so it costs no extra
//! traffic. What a speaker reports varies by model, firmware and source, so everything here
//! is optional; the person can also open the plain list of what the speaker sent
//! (*Technical details*) and copy it into a report.

use roxmltree::Document;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quality {
    /// "FLAC", "MP3", "AAC", "Ogg Vorbis"…
    pub format: Option<String>,
    /// FLAC, WAV, AIFF, ALAC: nothing was thrown away.
    pub lossless: bool,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u32>,
    /// What the speaker sent, as (what, value), with addresses reduced to their kind.
    pub details: Vec<(String, String)>,
}

impl Quality {
    /// Is there anything worth showing next to the song?
    pub fn is_empty(&self) -> bool {
        self.format.is_none() && self.sample_rate.is_none() && self.bit_depth.is_none() && self.bitrate_kbps.is_none()
    }

    /// Short labels for the Now Playing page, in the order they read best.
    pub fn labels(&self) -> Vec<String> {
        let mut v = vec![];
        if let Some(f) = &self.format {
            v.push(f.clone());
        }
        if let Some(r) = self.sample_rate {
            v.push(khz(r));
        }
        if let Some(b) = self.bit_depth {
            v.push(format!("{b}-bit"));
        }
        if let Some(k) = self.bitrate_kbps {
            v.push(format!("{k} kbps"));
        }
        v
    }

    /// The details as plain text, for pasting into a bug report. No addresses, names or titles.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.details {
            out.push_str(&format!("{k}: {v}\n"));
        }
        out
    }
}

/// 44100 → "44.1 kHz", 48000 → "48 kHz".
pub fn khz(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{:.1} kHz", hz as f32 / 1000.0)
    }
}

/// The track's quality from what the speaker reported: the song's address, what was asked to
/// play (a station or queue) and the song's DIDL-Lite description.
pub fn of(track_uri: &str, media_uri: &str, track_meta: &str) -> Quality {
    let mut q = Quality::default();
    let kind = |uri: &str| uri.split_once(':').map(|(s, _)| s.to_string()).filter(|s| !s.is_empty());
    if let Some(k) = kind(track_uri) {
        q.details.push(("Song address type".into(), k));
    }
    if let Some(k) = kind(media_uri).filter(|k| Some(k.as_str()) != kind(track_uri).as_deref()) {
        q.details.push(("Playing from".into(), k));
    }
    let mut mime: Option<String> = None;
    if let Ok(doc) = Document::parse(track_meta) {
        let mut other: Vec<String> = vec![];
        for n in doc.descendants().filter(|n| n.is_element()) {
            let name = n.tag_name().name();
            match name {
                "res" => {
                    if let Some(m) = n.attribute("protocolInfo").and_then(|p| p.split(':').nth(2)).filter(|m| *m != "*" && !m.is_empty()) {
                        mime.get_or_insert_with(|| m.to_string());
                    }
                    for (attr, label) in [
                        ("bitrate", "Bitrate (as sent)"),
                        ("sampleFrequency", "Sample rate (as sent)"),
                        ("bitsPerSample", "Bit depth (as sent)"),
                        ("nrAudioChannels", "Channels (as sent)"),
                    ] {
                        if let Some(v) = n.attribute(attr) {
                            q.details.push((label.into(), v.to_string()));
                        }
                    }
                    q.sample_rate = q.sample_rate.or_else(|| n.attribute("sampleFrequency").and_then(|v| v.trim().parse().ok()));
                    q.bit_depth = q.bit_depth.or_else(|| n.attribute("bitsPerSample").and_then(|v| v.trim().parse().ok()));
                    q.channels = q.channels.or_else(|| n.attribute("nrAudioChannels").and_then(|v| v.trim().parse().ok()));
                    q.bitrate_kbps = q.bitrate_kbps.or_else(|| n.attribute("bitrate").and_then(|v| v.trim().parse().ok()).map(to_kbps));
                }
                "streamInfo" => {
                    let text: String = n.children().filter_map(|c| c.text()).collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        q.details.push(("Stream info".into(), text.chars().take(200).collect()));
                        apply_stream_info(&mut q, &text);
                    }
                }
                "DIDL-Lite"
                | "item"
                | "container"
                | "title"
                | "creator"
                | "album"
                | "albumArtURI"
                | "class"
                | "artist"
                | "albumArtist"
                | "originalTrackNumber"
                | "streamContent"
                | "desc"
                | "description"
                | "radioShowMd" => {}
                other_name => {
                    let tag = match n.tag_name().namespace() {
                        Some(ns) if ns.contains("rinconnetworks") => format!("r:{other_name}"),
                        Some(ns) if ns.contains("upnp") => format!("upnp:{other_name}"),
                        _ => other_name.to_string(),
                    };
                    if !other.contains(&tag) {
                        other.push(tag);
                    }
                }
            }
        }
        if !other.is_empty() {
            q.details.push(("Other fields sent".into(), other.join(", ")));
        }
    }
    if let Some(m) = &mime {
        q.details.push(("Format (MIME)".into(), m.clone()));
    }
    let (format, lossless) = mime.as_deref().and_then(from_mime).or_else(|| from_extension(track_uri)).or_else(|| from_scheme(track_uri)).unzip();
    q.format = q.format.take().or(format.map(String::from));
    q.lossless = lossless.unwrap_or(false);
    if let Some(f) = &q.format {
        if is_lossless(f) {
            q.lossless = true;
        }
    }
    q
}

fn to_kbps(v: u32) -> u32 {
    // The UPnP standard says bytes per second; some devices send bits per second. A value over
    // 64000 can only be bits per second at any bitrate people use. (An estimate.)
    if v >= 64_000 {
        v / 1000
    } else {
        v * 8 / 1000
    }
}

fn is_lossless(format: &str) -> bool {
    ["FLAC", "WAV", "AIFF", "ALAC"].contains(&format)
}

fn from_mime(mime: &str) -> Option<(&'static str, bool)> {
    let m = mime.to_lowercase();
    Some(match m.as_str() {
        "audio/flac" | "audio/x-flac" => ("FLAC", true),
        "audio/mpeg" | "audio/mp3" | "audio/x-mpeg" => ("MP3", false),
        "audio/mp4" | "audio/aac" | "audio/aacp" | "audio/x-m4a" | "audio/m4a" => ("AAC", false),
        "audio/ogg" | "audio/x-ogg" | "application/ogg" => ("Ogg Vorbis", false),
        "audio/opus" => ("Opus", false),
        "audio/wav" | "audio/x-wav" | "audio/wave" => ("WAV", true),
        "audio/aiff" | "audio/x-aiff" => ("AIFF", true),
        "audio/x-ms-wma" => ("WMA", false),
        _ => return None,
    })
}

fn from_extension(uri: &str) -> Option<(&'static str, bool)> {
    let path = uri.split(['?', '#']).next()?;
    let ext = path.rsplit_once('.')?.1.to_lowercase();
    Some(match ext.as_str() {
        "flac" => ("FLAC", true),
        "mp3" => ("MP3", false),
        "m4a" | "aac" | "mp4" => ("AAC", false),
        "ogg" | "oga" => ("Ogg Vorbis", false),
        "opus" => ("Opus", false),
        "wav" => ("WAV", true),
        "aif" | "aiff" => ("AIFF", true),
        "wma" => ("WMA", false),
        _ => return None,
    })
}

fn from_scheme(uri: &str) -> Option<(&'static str, bool)> {
    if uri.starts_with("x-rincon-mp3radio:") {
        Some(("MP3", false))
    } else if uri.starts_with("aac:") || uri.starts_with("x-sonosapi-hls-static:") || uri.starts_with("x-sonosapi-hls:") {
        Some(("AAC", false))
    } else {
        None
    }
}

/// Read whatever the speaker puts in `streamInfo`: a JSON object, or `key=value` pairs.
/// Anything not understood is still listed in the details.
fn apply_stream_info(q: &mut Quality, text: &str) {
    let mut pairs: Vec<(String, String)> = vec![];
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(text) {
        for (k, v) in map {
            let v = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            pairs.push((k, v));
        }
    } else {
        for part in text.split([',', ';', '&', '|', '\n']) {
            if let Some((k, v)) = part.split_once(['=', ':']) {
                pairs.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
            }
        }
    }
    for (k, v) in pairs {
        let key: String = k.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let number = || v.trim().parse::<u32>().ok();
        match key.as_str() {
            "samplerate" | "samplefrequency" | "samplingrate" => q.sample_rate = q.sample_rate.or_else(number),
            "bitdepth" | "bitspersample" | "bitsample" => q.bit_depth = q.bit_depth.or_else(number),
            "bitrate" => q.bitrate_kbps = q.bitrate_kbps.or_else(|| number().map(to_kbps)),
            "channels" | "nrchannels" | "numchannels" => q.channels = q.channels.or_else(number),
            "codec" | "format" | "encoding" => {
                let f = match v.to_lowercase().as_str() {
                    "flac" => Some("FLAC"),
                    "mp3" | "mpeg" => Some("MP3"),
                    "aac" | "he-aac" | "heaac" | "aacp" => Some("AAC"),
                    "vorbis" | "ogg" | "oggvorbis" => Some("Ogg Vorbis"),
                    "opus" => Some("Opus"),
                    "alac" => Some("ALAC"),
                    "wav" | "pcm" => Some("WAV"),
                    _ => None,
                };
                if q.format.is_none() {
                    q.format = f.map(String::from);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn didl(inner: &str) -> String {
        format!(
            "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" xmlns:r=\"urn:schemas-rinconnetworks-com:metadata-1-0/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\"><item id=\"-1\" parentID=\"-1\"><dc:title>Secret title</dc:title>{inner}</item></DIDL-Lite>"
        )
    }

    #[test]
    fn a_library_flac_is_known_by_its_mime_type() {
        // What a real S1 system sent for a track from the music library.
        let meta = didl("<res protocolInfo=\"x-file-cifs:*:audio/flac:*\" duration=\"0:03:21\">x-file-cifs://nas/Music/a.flac</res>");
        let q = of("x-file-cifs://nas/Music/a.flac", "x-rincon-queue:RINCON_1#0", &meta);
        assert_eq!(q.format.as_deref(), Some("FLAC"));
        assert!(q.lossless);
        assert_eq!(q.labels(), ["FLAC"]);
        assert!(q.sample_rate.is_none(), "nothing is invented when the speaker doesn't say");
    }

    #[test]
    fn upnp_attributes_are_read_when_present() {
        let meta =
            didl("<res protocolInfo=\"http-get:*:audio/mpeg:*\" bitrate=\"40000\" sampleFrequency=\"44100\" bitsPerSample=\"16\" nrAudioChannels=\"2\">http://x/s.mp3</res>");
        let q = of("http://x/s.mp3", "", &meta);
        assert_eq!((q.sample_rate, q.bit_depth, q.channels, q.bitrate_kbps), (Some(44100), Some(16), Some(2), Some(320)));
        assert_eq!(q.labels(), ["MP3", "44.1 kHz", "16-bit", "320 kbps"]);
        assert!(!q.lossless);
    }

    #[test]
    fn stream_info_as_json_or_as_pairs() {
        let json = didl(r#"<r:streamInfo>{"codec":"flac","sampleRate":96000,"bitDepth":24}</r:streamInfo>"#);
        let q = of("x-sonos-http:abc.flac?sid=31", "", &json);
        assert_eq!((q.format.as_deref(), q.sample_rate, q.bit_depth), (Some("FLAC"), Some(96000), Some(24)));
        assert_eq!(q.labels(), ["FLAC", "96 kHz", "24-bit"]);
        let pairs = didl("<r:streamInfo>samplerate=48000, bitdepth=16, codec=aac</r:streamInfo>");
        let q = of("x-sonos-http:abc", "", &pairs);
        assert_eq!((q.format.as_deref(), q.sample_rate, q.bit_depth), (Some("AAC"), Some(48000), Some(16)));
    }

    #[test]
    fn unknown_stream_info_is_kept_for_the_report_not_guessed_at() {
        let meta = didl("<r:streamInfo>0:44100:3</r:streamInfo>");
        let q = of("x-sonos-http:abc", "", &meta);
        assert!(q.is_empty());
        assert!(q.report().contains("Stream info: 0:44100:3"), "{}", q.report());
    }

    #[test]
    fn format_falls_back_to_the_address() {
        assert_eq!(of("http://x/y/song.m4a?token=1", "", "").format.as_deref(), Some("AAC"));
        assert_eq!(of("x-rincon-mp3radio://stream.example/live", "", "").format.as_deref(), Some("MP3"));
        assert_eq!(of("x-sonosapi-hls-static:abc", "", "").format.as_deref(), Some("AAC"));
        assert!(of("x-rincon-queue:RINCON#0", "", "").is_empty());
        assert!(of("", "", "not xml at all").is_empty());
    }

    #[test]
    fn bitrate_in_bytes_or_bits_per_second() {
        assert_eq!(to_kbps(40_000), 320, "bytes per second, as the standard says");
        assert_eq!(to_kbps(320_000), 320, "bits per second");
        assert_eq!(to_kbps(16_000), 128);
    }

    #[test]
    fn the_report_has_no_titles_or_addresses() {
        let meta = didl("<res protocolInfo=\"x-file-cifs:*:audio/flac:*\">x-file-cifs://192.168.1.9/Music/Private/a.flac</res><upnp:odd>1</upnp:odd><r:other>2</r:other>");
        let q = of("x-file-cifs://192.168.1.9/Music/Private/a.flac", "x-rincon-queue:RINCON_ABC#0", &meta);
        let report = q.report();
        assert!(!report.contains("Secret") && !report.contains("192.168") && !report.contains("Private") && !report.contains("RINCON"), "{report}");
        assert!(report.contains("Song address type: x-file-cifs") && report.contains("Format (MIME): audio/flac"), "{report}");
        assert!(report.contains("upnp:odd") && report.contains("r:other"), "other field names help find new data: {report}");
    }

    #[test]
    fn rates_read_well() {
        assert_eq!(khz(44100), "44.1 kHz");
        assert_eq!(khz(48000), "48 kHz");
        assert_eq!(khz(192000), "192 kHz");
    }

    /// Read-only check against a real speaker that is playing (prints the sound details only,
    /// never titles or addresses): `ROOMTONE_SPEAKER=<ip> cargo test real_speaker_quality -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_speaker_quality() {
        let Ok(ip) = std::env::var("ROOMTONE_SPEAKER") else {
            eprintln!("Set ROOMTONE_SPEAKER=<ip> to run this.");
            return;
        };
        let poll = crate::net::speaker::poll_group(&ip, false).expect("the speaker answers");
        let n = &poll.now;
        eprintln!("transport={:?} stream={} source={:?}", n.transport, n.is_stream, n.source);
        let q = of(&n.uri, &n.media_uri, &n.track_meta);
        eprintln!("labels={:?} lossless={} channels={:?}", q.labels(), q.lossless, q.channels);
        eprintln!("--- what the speaker sent (as Copy details would) ---\n{}", q.report());
    }
}
