//! The person's own setup, kept between runs: added folders, radio stations,
//! podcast subscriptions and which music services are switched on.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Station {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub genre: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Podcast {
    pub title: String,
    #[serde(default)]
    pub author: String,
    pub feed: String,
    #[serde(default)]
    pub art: String,
}

/// A named set of rooms the person can group with one click (e.g. "Downstairs").
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SavedGroup {
    pub name: String,
    /// Which system the rooms belong to (rooms can only group within one system).
    pub household: String,
    pub rooms: Vec<String>,
}

/// A streaming service this app has been signed in to.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub sid: u32,
    pub name: String,
    pub uri: String,
    #[serde(default)]
    pub manifest: String,
    pub household: String,
    pub device_id: String,
    pub token: String,
    pub key: String,
    /// Which of the speakers' accounts for this service plays the music (usually 1).
    #[serde(default)]
    pub sn: u32,
}

/// The assistant link (MCP): lets AI assistants see and control the speakers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    /// Off until the person switches it on.
    #[serde(default)]
    pub enabled: bool,
    /// Assistants may change things (play, volume, grouping); off means look only.
    #[serde(default = "yes")]
    pub allow_control: bool,
    /// Accept assistants on other computers on the home network (otherwise this computer only).
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    /// The secret an assistant must send to connect over the network link.
    #[serde(default = "new_token")]
    pub token: String,
}

fn yes() -> bool {
    true
}

fn default_port() -> u16 {
    8790
}

/// A random-enough secret from the clock, the process and the address space.
pub fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

impl Default for McpConfig {
    fn default() -> Self {
        McpConfig { enabled: false, allow_control: true, allow_network: false, port: default_port(), token: new_token() }
    }
}

fn default_language() -> String {
    "auto".into()
}

fn default_services() -> Vec<String> {
    ["local", "radio", "podcasts", "archive"].iter().map(|s| s.to_string()).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub stations: Vec<Station>,
    #[serde(default)]
    pub podcasts: Vec<Podcast>,
    #[serde(default = "default_services")]
    pub services: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    /// Favorites the speakers can't store (albums, artists and folders from this computer,
    /// podcast shows, archive recordings).
    #[serde(default)]
    pub favorites: Vec<crate::model::Item>,
    #[serde(default)]
    pub groups: Vec<SavedGroup>,
    /// "auto" follows the computer's language; otherwise a code such as "de".
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub mcp: McpConfig,
    /// Appearance: "auto" follows the computer, or "light" / "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Let the speakers say when something changes (UPnP events) instead of asking over and
    /// over. Off until the person switches it on: it needs an incoming connection.
    #[serde(default)]
    pub speaker_events: bool,
}

fn default_theme() -> String {
    "auto".into()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            folders: vec![],
            stations: vec![],
            podcasts: vec![],
            services: default_services(),
            accounts: vec![],
            favorites: vec![],
            groups: vec![],
            language: default_language(),
            mcp: McpConfig::default(),
            theme: default_theme(),
            speaker_events: false,
        }
    }
}

impl Config {
    /// The folder where the app keeps its own files.
    pub fn data_dir() -> Option<PathBuf> {
        if cfg!(target_os = "windows") {
            // %APPDATA%\Roomtone
            let mut p = PathBuf::from(std::env::var_os("APPDATA").or_else(|| std::env::var_os("USERPROFILE"))?);
            p.push("Roomtone");
            return Some(p);
        }
        let mut p = PathBuf::from(std::env::var_os("HOME")?);
        if cfg!(target_os = "macos") {
            p.push("Library/Application Support/Roomtone");
        } else {
            p.push(".config/roomtone");
        }
        Some(p)
    }

    fn path() -> Option<PathBuf> {
        let mut p = Self::data_dir()?;
        p.push("config.json");
        Some(p)
    }

    pub fn load() -> Config {
        let text = Self::path().and_then(|p| std::fs::read_to_string(p).ok());
        let cfg: Config = text.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
        // Keep the assistant link's secret from the first run on, so it doesn't change.
        if text.as_deref().is_some_and(|t| !t.contains("\"mcp\"")) {
            let _ = cfg.save();
        }
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path().ok_or("No home folder to save settings in")?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&p, text).map_err(|e| e.to_string())?;
        // Sign-ins are kept here, so only this user may read the file.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn account(&self, sid: u32) -> Option<&Account> {
        self.accounts.iter().find(|a| a.sid == sid)
    }

    pub fn service_on(&self, id: &str) -> bool {
        self.services.iter().any(|s| s == id)
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let c = Config::default();
        assert_eq!(c.language, "auto");
        assert_eq!(c.theme, "auto");
        assert!(!c.mcp.enabled, "the assistant link starts switched off");
        assert!(c.mcp.allow_control);
        assert!(!c.mcp.allow_network);
        assert_eq!(c.mcp.port, 8790);
        assert_eq!(c.mcp.token.len(), 32);
        assert!(c.mcp.token.chars().all(|ch| ch.is_ascii_hexdigit()));
        for s in ["local", "radio", "podcasts", "archive"] {
            assert!(c.service_on(s));
        }
    }

    #[test]
    fn access_keys_differ() {
        assert_ne!(new_token(), new_token());
    }

    #[test]
    fn old_settings_files_still_load() {
        // A file from before languages, themes and the assistant link existed.
        let old = r#"{"folders": ["/music"], "stations": [{"name": "Jazz", "url": "http://x/jazz"}], "services": ["radio"]}"#;
        let c: Config = serde_json::from_str(old).unwrap();
        assert_eq!(c.folders, vec!["/music".to_string()]);
        assert_eq!(c.stations[0].genre, "");
        assert!(c.service_on("radio") && !c.service_on("local"));
        assert_eq!(c.language, "auto");
        assert_eq!(c.theme, "auto");
        assert!(!c.mcp.enabled);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let c: Config = serde_json::from_str(r#"{"from_the_future": 1, "theme": "dark"}"#).unwrap();
        assert_eq!(c.theme, "dark");
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let mut c = Config::default();
        c.groups.push(SavedGroup { name: "Downstairs".into(), household: "H".into(), rooms: vec!["A".into(), "B".into()] });
        c.mcp.enabled = true;
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.groups, c.groups);
        assert_eq!(back.mcp, c.mcp);
    }

    #[test]
    fn accounts_by_service_number() {
        let mut c = Config::default();
        c.accounts.push(Account { sid: 9, name: "Spotify".into(), ..Default::default() });
        assert_eq!(c.account(9).map(|a| a.name.as_str()), Some("Spotify"));
        assert!(c.account(2).is_none());
    }

    #[test]
    fn data_folder_is_per_platform() {
        crate::test_support::isolate();
        let d = Config::data_dir().unwrap();
        let s = d.to_string_lossy().to_lowercase();
        assert!(s.contains("roomtone"));
    }
}
