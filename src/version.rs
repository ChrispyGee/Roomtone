//! "Is there a newer Roomtone?": an optional check against the project's GitHub releases page.
//!
//! Off until the person switches it on. It sends one plain `GET` for the latest release (no
//! account, no identifiers: the only thing that identifies the app is the usual `User-Agent`
//! with its version number), at most once a day, and never installs anything: it only offers a
//! link to the download page.

use std::time::Duration;

/// How long to wait between automatic checks.
pub const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What the latest release is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// "0.5.5" (no leading `v`).
    pub version: String,
    /// The release's page.
    pub url: String,
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
    Some(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), url })
}

/// The address of the latest-release record for this project.
pub fn api_url(repo_url: &str) -> Option<String> {
    let path = repo_url.strip_prefix("https://github.com/")?.trim_end_matches('/').trim_end_matches(".git");
    path.contains('/').then(|| format!("https://api.github.com/repos/{path}/releases/latest"))
}

/// Ask `fetch` (given the address, it returns the JSON text) and compare with `current`.
pub fn check(repo_url: &str, current: &str, fetch: &dyn Fn(&str) -> Result<String, String>) -> Outcome {
    let Some(url) = api_url(repo_url) else { return Outcome::Failed("no project address".into()) };
    match fetch(&url) {
        Err(e) => Outcome::Failed(e),
        Ok(text) => match parse_release(&text) {
            Some(r) if is_newer(&r.version, current) => Outcome::Newer(r),
            Some(_) => Outcome::UpToDate,
            None => Outcome::Failed("the reply wasn't a release".into()),
        },
    }
}

/// The real fetch: one HTTPS `GET`, a few seconds' patience, no cookies.
pub fn fetch_over_https(url: &str) -> Result<String, String> {
    let agent =
        ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).timeout(Duration::from_secs(10)).user_agent(concat!("Roomtone/", env!("CARGO_PKG_VERSION"))).build();
    agent.get(url).set("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

#[cfg(test)]
mod unit {
    use super::*;

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
        assert_eq!(parse_release(ok), Some(Release { version: "0.6.0".into(), url: "https://github.com/ChrispyGee/Roomtone/releases/tag/v0.6.0".into() }));
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
