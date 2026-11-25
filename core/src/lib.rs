//! Roomtone's core: everything that talks to speakers and keeps their state, with no window code.
//! The desktop program (`roomtone`) draws it; other programs (the phone apps) can use it too.

#[macro_use]
pub mod i18n;
pub mod backend;
pub mod backup;
pub mod config;
pub mod csv;
pub mod demo;
pub mod diagnose;
pub mod frame;
pub mod help;
pub mod history;
pub mod known;
pub mod layout;
pub mod limits;
pub mod lyrics;
pub mod m3u;
pub mod mcp;
pub mod model;
pub mod net;
pub mod pls_xspf;
pub mod quality;
pub mod s2;
pub mod scene;
pub mod sources;
pub mod status;
pub mod stream_probe;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod update;
pub mod version;
