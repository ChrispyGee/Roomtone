//! Roomtone: a modern controller for S1 and S2 wireless speaker systems.
//!
//! Options:
//!
//! ```text
//!   --screenshot <file.png>  draw about 120 frames, save the window as a PNG and quit
//!                            (uses the demo system so the picture is the same every time)
//!   --demo                   start with the demo system instead of searching the network
//!   --view <page>            open a page first (home, search, favorites, playlists, library,
//!                            radio, inputs, alarms, settings, nowplaying)
//!   --lang <code>            use this language (en, de, fr, es, it, nl) for this run
//!   --headless               run without a window: find speakers and serve the assistant
//!                            link (MCP) over HTTP while it's switched on in Settings
//!   --mcp                    run without a window and serve the assistant link on
//!                            stdin/stdout (how most assistants start an MCP server)
//!   --mcp-on / --mcp-off     switch the assistant link on or off and quit
//!   --mcp-read-only / --mcp-allow-control
//!                            let assistants only look, or also change things, and quit
//!   --mcp-info               show the assistant link's settings and how to connect
//!   --write-icon <file> [px] save the app icon (a PNG, or a Windows .ico) for the installers
//!   --help [mcp]             options, or the assistant guide for this platform
//! ```
//!
//! See README.md, SPEC.md and docs/ for how the code fits together and what it does.

#[macro_use]
mod i18n;
mod backend;
mod backup;
mod brand;
mod cli;
mod config;
mod csv;
mod demo;
mod diagnose;
mod help;
mod history;
mod known;
mod m3u;
#[cfg(target_os = "macos")]
mod macos_about;
mod mcp;
mod media_keys;
mod model;
mod net;
mod pls_xspf;
mod quality;
mod s2;
mod sources;
#[cfg(test)]
mod test_support;
mod ui;
mod version;
mod window;

use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    // `roomtone play`, `roomtone volume 30 --room Kitchen`: control the running Roomtone.
    {
        let words: Vec<String> = std::env::args().skip(1).collect();
        if words.first().is_some_and(|w| cli::is_command(w)) {
            std::process::exit(cli::run(&words));
        }
    }
    let mut shot: Option<PathBuf> = None;
    let mut demo = false;
    let mut view: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut headless = false;
    let mut stdio = false;
    let mut edits: Vec<String> = vec![];
    let mut port: Option<u16> = None;
    let mut info = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => shot = args.next().map(PathBuf::from),
            "--write-icon" => {
                let Some(path) = args.next() else {
                    eprintln!("--write-icon needs a file name");
                    std::process::exit(2);
                };
                let size = args.next().and_then(|n| n.parse().ok()).unwrap_or(512);
                match brand::write_icon(std::path::Path::new(&path), size) {
                    Ok(()) => return,
                    Err(e) => {
                        eprintln!("Couldn't save {path}: {e}");
                        std::process::exit(1);
                    }
                }
            }
            "--demo" => demo = true,
            "--view" => view = args.next(),
            "--lang" => lang = args.next(),
            "--theme" => {
                if let Some(t) = args.next() {
                    std::env::set_var("ROOMTONE_THEME", t);
                }
            }
            "--headless" => headless = true,
            "--mcp" | "--mcp-stdio" => stdio = true,
            "--mcp-on" | "--mcp-off" | "--mcp-read-only" | "--mcp-allow-control" => edits.push(a.clone()),
            "--mcp-port" => port = args.next().and_then(|p| p.parse().ok()),
            "--mcp-info" => info = true,
            "--help" | "-h" => {
                if args.next().as_deref() == Some("mcp") {
                    print!("{}", help::mcp(&config::Config::load_readonly().mcp));
                } else {
                    print!("{}", help::general());
                }
                return;
            }
            "--help-mcp" => {
                print!("{}", help::mcp(&config::Config::load_readonly().mcp));
                return;
            }
            other => eprintln!("Ignoring unknown option {other} (see --help)"),
        }
    }
    if let Some(l) = &lang {
        std::env::set_var("ROOMTONE_LANG", l);
    }

    if !edits.is_empty() || port.is_some() || info {
        std::process::exit(mcp_settings(&edits, port, info));
    }
    if stdio || headless {
        run_headless(demo, stdio);
        return;
    }

    // Screenshots use the demo system unless ROOMTONE_LIVE is set (for testing real start-up).
    let live = std::env::var_os("ROOMTONE_LIVE").is_some();
    if shot.is_some() && !live {
        demo = true;
    }
    let quiet = shot.is_some();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Roomtone")
        .with_inner_size([1320.0, 840.0])
        .with_min_inner_size([1040.0, 660.0])
        .with_icon(eframe::egui::IconData { rgba: brand::icon_rgba(256), width: 256, height: 256 });
    if !quiet {
        // Open where and how the window was left.
        if let Some(w) = window::load() {
            viewport = viewport.with_inner_size([w.width, w.height]);
            if let (Some(x), Some(y)) = (w.x, w.y) {
                viewport = viewport.with_position([x, y]);
            }
            if w.maximized {
                viewport = viewport.with_maximized(true);
            }
        }
    }
    if quiet {
        // A check mustn't take focus from whatever the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(eframe::egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    if quiet {
        options.event_loop_builder = Some(Box::new(|_builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                _builder.with_activate_ignoring_other_apps(false);
            }
        }));
    }
    let result = eframe::run_native("Roomtone", options, Box::new(move |cc| Ok(Box::new(ui::App::new(cc, shot, demo, view.as_deref())))));
    if let Err(e) = result {
        eprintln!("Couldn't open the window: {e}");
        std::process::exit(1);
    }
}

/// Change the assistant link's settings from the command line (for computers without a screen).
fn mcp_settings(edits: &[String], port: Option<u16>, info: bool) -> i32 {
    let (mut cfg, _) = config::Config::load();
    for e in edits {
        match e.as_str() {
            "--mcp-on" => cfg.mcp.enabled = true,
            "--mcp-off" => cfg.mcp.enabled = false,
            "--mcp-read-only" => cfg.mcp.allow_control = false,
            "--mcp-allow-control" => cfg.mcp.allow_control = true,
            _ => {}
        }
    }
    if let Some(p) = port {
        cfg.mcp.port = p;
    }
    if !edits.is_empty() || port.is_some() {
        if let Err(e) = cfg.save() {
            eprintln!("Couldn't save settings: {e}");
            return 1;
        }
    }
    let m = &cfg.mcp;
    println!("Assistant link (MCP): {}", if m.enabled { "on" } else { "off" });
    println!("Assistants may: {}", if m.allow_control { "look and change things" } else { "only look" });
    println!("Other computers: {}", if m.allow_network { "allowed (with the access key)" } else { "not allowed" });
    if info || m.enabled {
        let exe = help::exe();
        println!();
        println!("Start it from an assistant (stdin/stdout):  {exe} --mcp");
        println!("Or connect over HTTP while the app or `{exe} --headless` runs:");
        println!("  {}", mcp::address(m));
        println!("  header  Authorization: Bearer {}", m.token);
        println!();
        println!("Step-by-step for {}: {exe} --help mcp", help::platform());
    }
    0
}

/// No window: find the speakers, keep them polled and serve the assistant link.
fn run_headless(demo: bool, stdio: bool) {
    let b = backend::Backend::start(backend::Waker::none(), demo);
    let link = mcp::Mcp::new(b.clone());
    // Settings changed in the app's window (another process) apply here too.
    {
        let b = b.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let fresh = config::Config::load_readonly();
            let mut s = b.lock();
            if s.config.mcp != fresh.mcp {
                s.config.mcp = fresh.mcp;
            }
        });
    }
    let enabled = b.lock().config.mcp.enabled;
    if !enabled {
        eprintln!("The assistant link (MCP) is switched off. Turn it on in Settings › Assistant link, or run `roomtone --mcp-on`.");
    }
    if stdio {
        // Give discovery a moment so the first question already sees the rooms.
        let until = std::time::Instant::now() + Duration::from_secs(6);
        while std::time::Instant::now() < until && b.lock().households.is_empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
        mcp::serve_stdio(link);
        return;
    }
    mcp::start_http(link, backend::Waker::none());
    eprintln!("Roomtone is running without a window. Press Ctrl+C to stop.");
    let mut last = String::new();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let line = {
            let s = b.lock();
            let rooms: usize = s.households.iter().map(|h| h.rooms.len()).sum();
            let phase = match s.phase {
                model::Phase::Searching => "searching for speakers",
                model::Phase::Live => "connected",
                model::Phase::Demo => "demo system",
                model::Phase::NothingFound => "no speakers found",
            };
            let link = match s.mcp_status.strip_prefix("ok:") {
                Some(addr) => format!("assistant link on at {addr}"),
                None if s.mcp_status.starts_with("err:") => format!("assistant link couldn't start ({})", &s.mcp_status[4..]),
                None => "assistant link off".to_string(),
            };
            format!("{phase} · {rooms} room(s) · {link}")
        };
        if line != last {
            eprintln!("{line}");
            last = line;
        }
    }
}
