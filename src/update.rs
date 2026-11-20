//! Fetching a newer Roomtone: the installer for this computer, downloaded from the project's
//! own release page when the person asks, and checked against the release's `SHA256SUMS`
//! before it is offered. Nothing is installed: the file is only opened for the person.

use crate::version::{Asset, Release};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where a download stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Download {
    #[default]
    Idle,
    /// Bytes received and the size it should be.
    Working(u64, u64),
    /// Saved and checked.
    Ready(PathBuf),
    Failed(String),
}

/// What kind of computer this is, as far as picking an installer goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
    pub has_dpkg: bool,
    pub has_rpm: bool,
}

impl Platform {
    pub fn here() -> Platform {
        Platform { os: std::env::consts::OS, arch: std::env::consts::ARCH, has_dpkg: Path::new("/usr/bin/dpkg").exists(), has_rpm: Path::new("/usr/bin/rpm").exists() }
    }
}

/// The installer for this computer among a release's files, if there is one.
pub fn pick_asset(release: &Release, p: Platform) -> Option<&Asset> {
    let ending = match (p.os, p.arch) {
        ("macos", "aarch64") => "-macos-arm64.dmg",
        ("windows", "x86_64") => "-windows-x64-setup.exe",
        ("linux", "x86_64") if p.has_dpkg => "-linux-x64.deb",
        ("linux", "x86_64") if p.has_rpm => "-linux-x64.rpm",
        ("linux", "x86_64") => "-linux-x64.tar.gz",
        _ => return None,
    };
    let name = format!("roomtone-v{}{ending}", release.version);
    release.assets.iter().find(|a| a.name == name && is_ours(&a.url, &name))
}

/// Only files from this project's own release downloads are fetched.
fn is_ours(url: &str, name: &str) -> bool {
    let prefix = format!("{}/releases/download/", crate::help::REPO_URL);
    url.starts_with(&prefix) && url.ends_with(&format!("/{name}")) && !name.contains(['/', '\\']) && !url.contains("..")
}

/// The `SHA256SUMS` file of a release, if it has one (from the project's own downloads).
pub fn sums_asset(release: &Release) -> Option<&Asset> {
    release.assets.iter().find(|a| a.name == "SHA256SUMS" && is_ours(&a.url, "SHA256SUMS"))
}

/// `sha256sum` output (`hex  name`, the name possibly after a `*`) → name → hex.
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (hash, name) = l.split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*');
            (hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) && !name.is_empty()).then(|| (name.to_string(), hash.to_lowercase()))
        })
        .collect()
}

/// The SHA-256 of what a reader gives, as lower-case hex, with a running count for progress.
#[cfg(test)]
fn sha256_hex(mut r: impl Read, mut progress: impl FnMut(u64)) -> std::io::Result<String> {
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
        progress(total);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The most that will be downloaded, whatever a release says its size is.
const LARGEST: u64 = 300 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(30)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build()
}

/// Where downloads are kept: the Downloads folder, or the temporary folder if there isn't one.
pub fn downloads_dir() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    home.map(|h| h.join("Downloads")).filter(|d| d.is_dir()).unwrap_or_else(std::env::temp_dir)
}

/// Download `asset` into `dir`, check it against the release's sums, and return where it is.
/// `progress` gets (bytes so far, expected size). A file that doesn't match is deleted.
pub fn download(release: &Release, asset: &Asset, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf, String> {
    let sums = sums_asset(release).ok_or_else(|| "This release has no checksum file, so the download can't be checked".to_string())?;
    let sums_text = agent().get(&sums.url).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    let expected = parse_sums(&sums_text).remove(&asset.name).ok_or_else(|| "The checksum file doesn't list this download".to_string())?;
    let response = agent().get(&asset.url).call().map_err(|e| e.to_string())?;
    let total = response.header("Content-Length").and_then(|l| l.parse::<u64>().ok()).unwrap_or(asset.size);
    if total == 0 || total > LARGEST {
        return Err("The download has an unexpected size".into());
    }
    let part = dir.join(format!("{}.part", asset.name));
    let result = (|| -> Result<String, String> {
        let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut reader = response.into_reader().take(LARGEST + 1);
        let mut buf = [0u8; 64 * 1024];
        let mut hash = Sha256::new();
        let mut done = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hash.update(&buf[..n]);
            done += n as u64;
            if done > LARGEST {
                return Err("The download is larger than expected".into());
            }
            progress(done, total);
        }
        file.flush().map_err(|e| e.to_string())?;
        Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
    })();
    let actual = match result {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    if actual != expected {
        let _ = std::fs::remove_file(&part);
        return Err("The download didn't match its checksum, so it was deleted".into());
    }
    let dest = dir.join(&asset.name);
    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Open a downloaded installer with the computer's usual program for it.
pub fn open(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

/// Show a file in the computer's file browser.
pub fn reveal(path: &Path) {
    let mut cmd;
    if cfg!(target_os = "macos") {
        cmd = std::process::Command::new("open");
        cmd.arg("-R").arg(path);
    } else if cfg!(target_os = "windows") {
        cmd = std::process::Command::new("explorer");
        cmd.arg(format!("/select,{}", path.display()));
    } else {
        cmd = std::process::Command::new("xdg-open");
        cmd.arg(path.parent().unwrap_or(path));
    }
    let _ = cmd.spawn();
}

#[cfg(test)]
mod unit {
    use super::*;

    fn release() -> Release {
        let base = format!("{}/releases/download/v1.2.3", crate::help::REPO_URL);
        let asset = |n: &str| Asset { name: n.into(), url: format!("{base}/{n}"), size: 100 };
        Release {
            version: "1.2.3".into(),
            url: String::new(),
            assets: [
                "roomtone-v1.2.3-macos-arm64.dmg",
                "roomtone-v1.2.3-windows-x64-setup.exe",
                "roomtone-v1.2.3-linux-x64.deb",
                "roomtone-v1.2.3-linux-x64.rpm",
                "roomtone-v1.2.3-linux-x64.tar.gz",
                "SHA256SUMS",
            ]
            .iter()
            .map(|n| asset(n))
            .collect(),
        }
    }

    fn plat(os: &'static str, arch: &'static str, dpkg: bool, rpm: bool) -> Platform {
        Platform { os, arch, has_dpkg: dpkg, has_rpm: rpm }
    }

    #[test]
    fn each_computer_gets_its_own_installer() {
        let r = release();
        let name = |p| pick_asset(&r, p).map(|a| a.name.clone());
        assert_eq!(name(plat("macos", "aarch64", false, false)).as_deref(), Some("roomtone-v1.2.3-macos-arm64.dmg"));
        assert_eq!(name(plat("windows", "x86_64", false, false)).as_deref(), Some("roomtone-v1.2.3-windows-x64-setup.exe"));
        assert_eq!(name(plat("linux", "x86_64", true, false)).as_deref(), Some("roomtone-v1.2.3-linux-x64.deb"));
        assert_eq!(name(plat("linux", "x86_64", false, true)).as_deref(), Some("roomtone-v1.2.3-linux-x64.rpm"));
        assert_eq!(name(plat("linux", "x86_64", false, false)).as_deref(), Some("roomtone-v1.2.3-linux-x64.tar.gz"));
        assert_eq!(name(plat("macos", "x86_64", false, false)), None, "no Intel Mac build: only the download page");
        assert_eq!(name(plat("linux", "aarch64", true, false)), None);
    }

    #[test]
    fn files_from_anywhere_else_are_not_fetched() {
        let mut r = release();
        r.assets[0].url = "https://example.com/releases/download/v1.2.3/roomtone-v1.2.3-macos-arm64.dmg".into();
        assert!(pick_asset(&r, plat("macos", "aarch64", false, false)).is_none());
        let mut r = release();
        r.assets[0].url = format!("{}/releases/download/v1.2.3/../x/roomtone-v1.2.3-macos-arm64.dmg", crate::help::REPO_URL);
        assert!(pick_asset(&r, plat("macos", "aarch64", false, false)).is_none());
        let mut r = release();
        r.assets[5].url = "http://github.com/x/SHA256SUMS".into();
        assert!(sums_asset(&r).is_none());
        assert!(sums_asset(&release()).is_some());
    }

    #[test]
    fn checksums_are_read_by_file_name() {
        let h = "a".repeat(64);
        let text = format!("{h}  roomtone-v1.dmg\n{}  *other.exe\nnot a line\n{}  short\n", "B".repeat(64), "c".repeat(10));
        let sums = parse_sums(&text);
        assert_eq!(sums.get("roomtone-v1.dmg"), Some(&h));
        assert_eq!(sums.get("other.exe"), Some(&"b".repeat(64)), "a star and capitals are fine");
        assert_eq!(sums.len(), 2, "lines that aren't a checksum are ignored");
    }

    #[test]
    fn the_checksum_of_known_text_is_right() {
        let mut last = 0;
        let h = sha256_hex(&b"abc"[..], |n| last = n).unwrap();
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(last, 3);
        assert_eq!(sha256_hex(&b""[..], |_| {}).unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn a_release_with_no_sums_cannot_be_downloaded() {
        let mut r = release();
        r.assets.retain(|a| a.name != "SHA256SUMS");
        let asset = r.assets[0].clone();
        let err = download(&r, &asset, &std::env::temp_dir(), |_, _| {}).unwrap_err();
        assert!(err.contains("checksum"), "{err}");
    }

    /// Real download of the project's latest release for this computer (about 7 MB):
    /// `cargo test real_download -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_download() {
        let text = crate::version::fetch_over_https(&crate::version::api_url(crate::help::REPO_URL).unwrap()).unwrap();
        let release = crate::version::parse_release(&text).unwrap();
        let Some(asset) = pick_asset(&release, Platform::here()) else {
            eprintln!("No installer for this computer in {}", release.version);
            return;
        };
        let dir = std::env::temp_dir().join(format!("roomtone-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut last = 0;
        let path = download(&release, asset, &dir, |d, _| last = d).unwrap();
        eprintln!("saved {} ({} bytes)", path.display(), last);
        assert!(path.is_file() && !dir.join(format!("{}.part", asset.name)).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
