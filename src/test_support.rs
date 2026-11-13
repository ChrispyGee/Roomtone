//! Helpers shared by the unit tests: a private settings folder (so tests never touch the real
//! user's settings), English wording, and a running demo system to drive.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::backend::{Backend, Waker};

/// Point the app's data folder at a scratch folder under `target/` and use English. Safe to
/// call from every test; it only acts once per test run.
pub fn isolate() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("unit-test-home");
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("HOME", &dir);
        std::env::set_var("APPDATA", &dir);
        std::env::set_var("ROOMTONE_LANG", "en");
        crate::i18n::set_language("en");
    });
}

/// Wait up to `secs` for `cond`.
pub fn wait_for(secs: u64, cond: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    cond()
}

/// A backend running the demo system (one S2 and one S1 system), ready to use.
pub fn demo_backend() -> Backend {
    isolate();
    let b = Backend::start(Waker::none(), true);
    assert!(wait_for(10, || b.lock().households.len() == 2), "the demo system didn't load");
    b
}
