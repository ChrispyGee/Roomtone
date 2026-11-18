//! A local crash log. If Roomtone panics, what happened is written to `crash.log` in the
//! settings folder, so a tester who says "it closed" has something to attach. Nothing is sent
//! anywhere. Addresses and the person's home folder are blanked out before it's written.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// The file is cut back to about this much, oldest first, so it never grows without end.
const KEEP_BYTES: usize = 64 * 1024;

/// Where the log lives (the settings folder).
pub fn log_path() -> Option<PathBuf> {
    let mut p = crate::config::Config::data_dir()?;
    p.push("crash.log");
    Some(p)
}

/// When the log was last written to, as "2026-11-18 16:46 UTC", if there is a log.
pub fn last_crash() -> Option<String> {
    let secs = std::fs::metadata(log_path()?).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{} UTC", &utc(secs)[..16]))
}

/// Note panics in the log (and still print them as usual).
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "(no message)".to_string()
        };
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok();
        let entry = format_entry(now, env!("CARGO_PKG_VERSION"), thread.name().unwrap_or("unnamed"), &message, &place, &backtrace, home.as_deref());
        append(&entry);
        previous(info);
    }));
}

fn append(entry: &str) {
    let Some(path) = log_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let text = trim_front(&format!("{old}{entry}"), KEEP_BYTES);
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// One report, ready to write.
pub fn format_entry(when: u64, version: &str, thread: &str, message: &str, place: &str, backtrace: &str, home: Option<&str>) -> String {
    let body = format!(
        "== Roomtone {version} on {} {} at {} (UTC) ==\nthread: {thread}\nmessage: {message}\nwhere: {place}\n{backtrace}\n\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        utc(when)
    );
    scrub(&body, home)
}

/// Blank out IPv4 addresses and the home folder, which a panic message can carry.
pub fn scrub(text: &str, home: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(h) = home.filter(|h| h.len() > 3) {
        out = out.replace(h, "~");
    }
    // Four groups of 1 to 3 digits, joined by dots.
    let b = out.as_bytes();
    let mut result = String::with_capacity(out.len());
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_digit() || b[i - 1] == b'.')) {
            if let Some(len) = ipv4_len(&b[i..]) {
                result.push_str("x.x.x.x");
                i += len;
                continue;
            }
        }
        let ch = out[i..].chars().next().unwrap_or(' ');
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}

/// If the text starts with a dotted address (such as 192.168.1.20), how long it is.
fn ipv4_len(b: &[u8]) -> Option<usize> {
    let mut i = 0;
    for group in 0..4 {
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() && i - start < 3 {
            i += 1;
        }
        if i == start {
            return None;
        }
        if group < 3 {
            if i < b.len() && b[i] == b'.' {
                i += 1;
            } else {
                return None;
            }
        }
    }
    // Not part of a longer number.
    if i < b.len() && b[i].is_ascii_digit() {
        return None;
    }
    Some(i)
}

/// Keep roughly the last `max` bytes, starting at a line.
fn trim_front(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    match text[start..].find('\n') {
        Some(n) => text[start + n + 1..].to_string(),
        None => text[start..].to_string(),
    }
}

/// "2026-11-18 16:46:32" for seconds since 1970.
fn utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn times_are_written_in_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00:00");
        assert_eq!(utc(1_795_020_392), "2026-11-18 16:46:32");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00", "a leap day");
    }

    #[test]
    fn addresses_and_the_home_folder_are_blanked() {
        let t = "Browse: http://172.16.120.222:1400/x failed for /Users/pat/Music/a.mp3 at 10.0.0.5, version 1.2.3, port 52880";
        let s = scrub(t, Some("/Users/pat"));
        assert!(s.contains("http://x.x.x.x:1400/x"), "{s}");
        assert!(s.contains("~/Music/a.mp3"), "{s}");
        assert!(s.contains("at x.x.x.x,"), "{s}");
        assert!(s.contains("version 1.2.3,"), "three numbers aren't an address: {s}");
        assert!(s.contains("port 52880"), "{s}");
        assert!(!s.contains("pat"), "{s}");
        assert_eq!(scrub("1234.5.6.7 and 1.2.3.4567", None), "1234.5.6.7 and 1.2.3.4567", "not addresses");
    }

    #[test]
    fn an_entry_says_what_where_and_when() {
        let e = format_entry(1_795_020_392, "0.9.1", "main", "index out of bounds", "src/x.rs:10", "frame 0", None);
        for want in ["Roomtone 0.9.1", "2026-11-18 16:46:32", "thread: main", "message: index out of bounds", "where: src/x.rs:10", "frame 0"] {
            assert!(e.contains(want), "{e}");
        }
    }

    #[test]
    fn the_log_never_grows_without_end() {
        let mut text = String::new();
        for i in 0..5000 {
            text.push_str(&format!("line {i} with some words in it\n"));
        }
        let cut = trim_front(&text, 4096);
        assert!(cut.len() <= 4096);
        assert!(cut.starts_with("line "), "starts at a whole line");
        assert!(cut.ends_with("line 4999 with some words in it\n"), "keeps the newest");
        assert_eq!(trim_front("short", 4096), "short");
    }
}
