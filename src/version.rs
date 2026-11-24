//! "Is there a newer Roomtone?": an optional check against the project's GitHub releases page.
//!
//! Off until the person switches it on. It sends one plain `GET` for the latest release (no
//! account, no identifiers: the only thing that identifies the app is the usual `User-Agent`
//! with its version number), at most once a day, and never installs anything: it only offers a
//! link to the download page.

use std::time::Duration;

/// How long to wait between automatic checks.
pub const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// How long to wait before looking again after `failures` failed looks in a row (5 minutes,
/// then 15, 45, 2¼ hours, and never more than 6 hours), so an offline or rate-limited computer
/// isn't asking every few seconds.
pub fn retry_after(failures: u32) -> Duration {
    let minutes = 5u64.saturating_mul(3u64.saturating_pow(failures.saturating_sub(1)));
    Duration::from_secs(minutes.min(6 * 60) * 60)
}

/// The command that updates Roomtone when it was installed with Homebrew.
pub const BREW_UPGRADE: &str = "brew upgrade --cask roomtone";

/// Was Roomtone installed with Homebrew? Homebrew keeps a folder per installed cask.
pub fn installed_with_homebrew() -> bool {
    static ANSWER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ANSWER.get_or_init(|| {
        // Picture checks: ROOMTONE_TEST_HOMEBREW=0 or 1 says what the answer is.
        if let Some(v) = std::env::var_os("ROOMTONE_TEST_HOMEBREW") {
            return v == "1";
        }
        let mut prefixes: Vec<std::path::PathBuf> = ["/opt/homebrew", "/usr/local"].iter().map(Into::into).collect();
        prefixes.extend(std::env::var_os("HOMEBREW_PREFIX").map(std::path::PathBuf::from));
        cask_installed_under(&prefixes)
    })
}

fn cask_installed_under(prefixes: &[std::path::PathBuf]) -> bool {
    prefixes.iter().any(|p| p.join("Caskroom").join("roomtone").is_dir())
}

/// What the latest release is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Release {
    /// "0.5.5" (no leading `v`).
    pub version: String,
    /// The release's page.
    pub url: String,
    /// The files attached to it.
    pub assets: Vec<Asset>,
}

/// One file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// What the last check found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    /// Not checked yet (or switched off).
    #[default]
    Unknown,
    UpToDate,
    Newer(Release),
    /// Couldn't ask (offline, rate limited…), with a short reason.
    Failed(String),
}

/// `"v1.2.3"` or `"1.2.3"` (an ending like `-beta` is ignored) → `(1, 2, 3)`.
pub fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
    let out = (parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?);
    parts.next().is_none().then_some(out)
}

/// Is `latest` a newer version than `current`? (Anything that isn't a version says no.)
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// The latest release out of GitHub's JSON, if it's a real (not draft or pre-release) one.
pub fn parse_release(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("draft").and_then(|d| d.as_bool()) == Some(true) || v.get("prerelease").and_then(|d| d.as_bool()) == Some(true) {
        return None;
    }
    let tag = v.get("tag_name")?.as_str()?;
    parse(tag)?;
    let url = v.get("html_url").and_then(|u| u.as_str()).filter(|u| u.starts_with("https://")).unwrap_or("").to_string();
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a.get("name")?.as_str()?.to_string(),
                        url: a.get("browser_download_url")?.as_str()?.to_string(),
                        size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), url, assets })
}

/// The address of the latest-release record for this project.
pub fn api_url(repo_url: &str) -> Option<String> {
    let path = repo_url.strip_prefix("https://github.com/")?.trim_end_matches('/').trim_end_matches(".git");
    path.contains('/').then(|| format!("https://api.github.com/repos/{path}/releases/latest"))
}

/// The project's public "latest release" page. It redirects to the page of the newest release
/// (`…/releases/tag/v1.2.3`) and, unlike the API, isn't limited to 60 looks an hour per network.
pub fn latest_page_url(repo_url: &str) -> Option<String> {
    api_url(repo_url).map(|_| format!("{}/releases/latest", repo_url.trim_end_matches('/').trim_end_matches(".git")))
}

/// Did GitHub refuse because this network has asked too often (HTTP 403 or 429)?
fn is_rate_limited(error: &str) -> bool {
    error.contains("status code 403") || error.contains("status code 429")
}

/// The release a "latest release" page redirected to, from its address (`…/releases/tag/v1.2.3`).
/// It has no list of files, so the update is offered as a link to the download page.
pub fn release_from_page_address(address: &str) -> Option<Release> {
    let address = address.trim();
    let tag = address.split("/releases/tag/").nth(1)?.split(['/', '?', '#']).next()?;
    parse(tag)?;
    address.starts_with("https://").then(|| Release { version: tag.trim_start_matches(['v', 'V']).to_string(), url: address.to_string(), assets: vec![] })
}

/// Ask `fetch` (given the address, it returns the JSON text) and compare with `current`. If GitHub
/// is limiting this network, ask the public release page instead (`fetch` returns the address it
/// redirects to for that one).
pub fn check(repo_url: &str, current: &str, fetch: &dyn Fn(&str) -> Result<String, String>) -> Outcome {
    let Some(url) = api_url(repo_url) else { return Outcome::Failed("no project address".into()) };
    let release = match fetch(&url) {
        Ok(text) => match parse_release(&text) {
            Some(r) => r,
            None => return Outcome::Failed("the reply wasn't a release".into()),
        },
        Err(e) if is_rate_limited(&e) => {
            let page = latest_page_url(repo_url).and_then(|u| fetch(&u).ok()).and_then(|a| release_from_page_address(&a));
            match page {
                Some(r) => r,
                None => return Outcome::Failed("GitHub is limiting checks from this network right now; Roomtone will try again later".into()),
            }
        }
        Err(e) => return Outcome::Failed(e),
    };
    if is_newer(&release.version, current) {
        Outcome::Newer(release)
    } else {
        Outcome::UpToDate
    }
}

/// The real fetch: one HTTPS `GET`, a few seconds' patience, no cookies.
pub fn fetch_over_https(url: &str) -> Result<String, String> {
    let builder = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).timeout(Duration::from_secs(10)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION")));
    // The public "latest release" page: don't follow its redirect, just say where it points.
    if url.starts_with("https://github.com/") {
        let reply = builder.redirects(0).build().get(url).call().map_err(|e| e.to_string())?;
        return reply.header("Location").map(|l| l.to_string()).ok_or_else(|| "the release page didn't point anywhere".to_string());
    }
    let agent = builder.build();
    agent.get(url).set("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_failed_look_is_retried_later_and_later() {
        let mins = |n| retry_after(n).as_secs() / 60;
        assert_eq!((mins(1), mins(2), mins(3), mins(4), mins(5)), (5, 15, 45, 135, 360));
        assert_eq!((mins(6), mins(50), mins(u32::MAX)), (360, 360, 360), "never more than 6 hours");
        assert_eq!(mins(0), 5, "no failures yet is not shorter than the first");
    }

    #[test]
    fn a_homebrew_install_is_told_by_its_cask_folder() {
        let dir = std::env::temp_dir().join(format!("roomtone-brew-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!cask_installed_under(std::slice::from_ref(&dir)));
        std::fs::create_dir_all(dir.join("Caskroom").join("other")).unwrap();
        assert!(!cask_installed_under(std::slice::from_ref(&dir)), "another cask is not this one");
        std::fs::create_dir_all(dir.join("Caskroom").join("roomtone")).unwrap();
        assert!(cask_installed_under(&[std::path::PathBuf::from("/nonexistent"), dir.clone()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versions_are_read_and_compared() {
        assert_eq!(parse("v0.5.5"), Some((0, 5, 5)));
        assert_eq!(parse("1.2"), Some((1, 2, 0)));
        assert_eq!(parse("0.6.0-beta.1"), Some((0, 6, 0)));
        assert_eq!(parse("1.2.3.4"), None);
        assert_eq!(parse("latest"), None);
        assert!(is_newer("0.6.0", "0.5.5") && is_newer("v1.0.0", "0.9.9") && is_newer("0.5.10", "0.5.9"), "numbers, not text");
        assert!(!is_newer("0.5.5", "0.5.5") && !is_newer("0.5.4", "0.5.5"));
        assert!(!is_newer("nightly", "0.5.5") && !is_newer("0.6.0", "dev"), "anything odd says no");
    }

    #[test]
    fn github_replies_are_read() {
        let ok = r#"{"tag_name":"v0.6.0","html_url":"https://github.com/ChrispyGee/Roomtone/releases/tag/v0.6.0","draft":false,"prerelease":false,"name":"Roomtone 0.6.0"}"#;
        assert_eq!(parse_release(ok), Some(Release { version: "0.6.0".into(), url: "https://github.com/ChrispyGee/Roomtone/releases/tag/v0.6.0".into(), assets: vec![] }));
        assert!(parse_release(r#"{"tag_name":"v0.6.0","prerelease":true}"#).is_none(), "pre-releases aren't offered");
        assert!(parse_release(r#"{"tag_name":"v0.6.0","draft":true}"#).is_none());
        assert!(parse_release(r#"{"message":"Not Found"}"#).is_none());
        assert!(parse_release("not json").is_none());
        let odd_link = parse_release(r#"{"tag_name":"v0.6.0","html_url":"javascript:alert(1)"}"#).unwrap();
        assert_eq!(odd_link.url, "", "only https links are kept");
    }

    #[test]
    fn the_address_comes_from_the_project_address() {
        assert_eq!(api_url("https://github.com/ChrispyGee/Roomtone").as_deref(), Some("https://api.github.com/repos/ChrispyGee/Roomtone/releases/latest"));
        assert_eq!(api_url("https://github.com/ChrispyGee/Roomtone.git/").as_deref(), Some("https://api.github.com/repos/ChrispyGee/Roomtone/releases/latest"));
        assert_eq!(api_url("https://example.com/x/y"), None);
        assert_eq!(api_url("https://github.com/only"), None);
    }

    #[test]
    fn when_github_limits_the_api_the_release_page_answers() {
        let repo = "https://github.com/ChrispyGee/Roomtone";
        assert_eq!(latest_page_url(repo).as_deref(), Some("https://github.com/ChrispyGee/Roomtone/releases/latest"));
        assert_eq!(latest_page_url("https://github.com/ChrispyGee/Roomtone.git/").as_deref(), Some("https://github.com/ChrispyGee/Roomtone/releases/latest"));
        let tag = "https://github.com/ChrispyGee/Roomtone/releases/tag/v0.9.6";
        assert_eq!(release_from_page_address(tag), Some(Release { version: "0.9.6".into(), url: tag.into(), assets: vec![] }));
        assert_eq!(release_from_page_address("https://github.com/ChrispyGee/Roomtone/releases"), None);
        assert_eq!(release_from_page_address("https://github.com/x/y/releases/tag/nightly"), None);
        assert_eq!(release_from_page_address("http://github.com/x/y/releases/tag/v1.0.0"), None, "only https");

        let limited = |page: Result<String, String>| move |u: &str| if u.starts_with("https://api.") { Err(format!("{u}: status code 403")) } else { page.clone() };
        assert!(matches!(check(repo, "0.9.5", &limited(Ok(tag.into()))), Outcome::Newer(r) if r.version == "0.9.6" && r.assets.is_empty()));
        assert_eq!(check(repo, "0.9.6", &limited(Ok(tag.into()))), Outcome::UpToDate);
        let both = check(repo, "0.9.5", &limited(Err("offline".into())));
        assert!(matches!(&both, Outcome::Failed(m) if m.contains("limiting") && !m.contains("403")), "{both:?}");
        assert!(matches!(check(repo, "0.9.5", &|u: &str| Err(format!("{u}: status code 404"))), Outcome::Failed(m) if m.contains("404")), "other errors are told as they are");
    }

    /// The real release page: `cargo test real_release_page -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_release_page() {
        let address = fetch_over_https(&latest_page_url(crate::help::REPO_URL).unwrap()).unwrap();
        eprintln!("{address}");
        assert!(release_from_page_address(&address).is_some());
    }

    #[test]
    fn a_check_says_newer_current_or_failed() {
        let repo = "https://github.com/ChrispyGee/Roomtone";
        let reply = |tag: &'static str| move |_: &str| Ok(format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/ChrispyGee/Roomtone/releases/tag/{tag}"}}"#));
        assert!(matches!(check(repo, "0.5.5", &reply("v0.6.0")), Outcome::Newer(r) if r.version == "0.6.0"));
        assert_eq!(check(repo, "0.5.5", &reply("v0.5.5")), Outcome::UpToDate);
        assert_eq!(check(repo, "0.6.1", &reply("v0.6.0")), Outcome::UpToDate, "a newer build than the latest release is fine");
        assert!(matches!(check(repo, "0.5.5", &|_| Err("offline".into())), Outcome::Failed(m) if m == "offline"));
        assert!(matches!(check(repo, "0.5.5", &|_| Ok("<html>".into())), Outcome::Failed(_)));
        let asked = std::cell::RefCell::new(String::new());
        check(repo, "0.5.5", &|u| {
            *asked.borrow_mut() = u.to_string();
            Err("x".into())
        });
        assert_eq!(*asked.borrow(), "https://api.github.com/repos/ChrispyGee/Roomtone/releases/latest");
    }
}
