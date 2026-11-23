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
//!   --mcp-allow|--mcp-deny saved|alarms|names|updates, --mcp-files <folder> / --mcp-files-off
//!                            let assistants only look, or also change things, and quit
//!   --mcp-info               show the assistant link's settings and how to connect
//!   --write-icon <file> [px] save the app icon (a PNG, or a Windows .ico) for the installers
//!   --help [mcp]             options, or the assistant guide for this platform
//! ```
//!
//! See README.md, SPEC.md and docs/ for how the code fits together and what it does.

// On Windows the program is a window program, so opening it never shows a black console window
// (closing that window used to close the app too). Run from a terminal, it attaches to that
// terminal so `--help`, the command-line commands and `--headless` still print there; see `console`.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod backend;
mod backup;
mod brand;
mod cli;
mod config;
mod crash;
mod csv;
mod demo;
mod diagnose;
mod frame;
mod help;
mod history;
mod known;
mod layout;
mod limits;
mod lyrics;
mod m3u;
#[cfg(target_os = "macos")]
mod macos_about;
#[cfg(target_os = "macos")]
mod macos_menubar;
mod mcp;
mod model;
mod net;
mod pls_xspf;
mod quality;
mod s2;
mod scene;
mod sources;
mod status;
#[cfg(test)]
mod test_support;
mod ui;
mod update;
mod version;
mod window;

use std::path::PathBuf;
use std::time::Duration;

/// Windows only: a window program has no console of its own, so when it was started from a
/// terminal (Command Prompt, PowerShell, Windows Terminal) use that terminal for text output.
/// Started from the Start menu or Explorer there is no terminal and nothing happens. Output the
/// caller already redirected (a pipe, a file, an assistant starting `--mcp`) is left alone.
#[cfg(windows)]
mod console {
    use std::ffi::c_void;
    use std::os::windows::io::IntoRawHandle;

    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
    }

    fn missing(h: *mut c_void) -> bool {
        h.is_null() || h as isize == -1
    }

    pub fn attach_to_terminal() {
        // SAFETY: plain Win32 calls with valid arguments; handles opened here are leaked on purpose
        // so they stay valid for the life of the process.
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                return; // no terminal to attach to
            }
            for (which, name, write) in [(STD_OUTPUT_HANDLE, "CONOUT$", true), (STD_ERROR_HANDLE, "CONOUT$", true), (STD_INPUT_HANDLE, "CONIN$", false)] {
                if missing(GetStdHandle(which)) {
                    let file = std::fs::OpenOptions::new().read(!write).write(write).open(name);
                    if let Ok(f) = file {
                        SetStdHandle(which, f.into_raw_handle());
                    }
                }
            }
        }
    }
}

fn main() {
    #[cfg(windows)]
    console::attach_to_terminal();
    crash::install();
    // Test hook: a real panic, to see what the crash log keeps (`ROOMTONE_TEST_PANIC=1`).
    if std::env::var_os("ROOMTONE_TEST_PANIC").is_some() {
        panic!("test panic: speaker at 192.168.1.9 for {}", std::env::var("HOME").unwrap_or_default());
    }
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
            "--mcp-allow" | "--mcp-deny" | "--mcp-files" => match args.next() {
                Some(v) => edits.push(format!("{a}={v}")),
                None => {
                    eprintln!("{a} needs a value (see --help)");
                    std::process::exit(2);
                }
            },
            "--mcp-files-off" => edits.push(a.clone()),
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
    if quiet {
        // For picture checks that need a particular size, such as the Microsoft Store listing:
        // ROOMTONE_TEST_SIZE=1920x1080 (logical points; a Retina screen draws twice as many pixels).
        if let Some((w, h)) = std::env::var("ROOMTONE_TEST_SIZE").ok().and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))) {
            viewport = viewport.with_inner_size([w, h]).with_min_inner_size([w.min(1040.0), h.min(660.0)]);
        }
    }
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
            "--mcp-files-off" => cfg.mcp.allow_files = false,
            other => {
                if let Some((flag, value)) = other.split_once('=') {
                    let on = flag == "--mcp-allow";
                    match (flag, value) {
                        ("--mcp-allow" | "--mcp-deny", "saved") => cfg.mcp.allow_saved = on,
                        ("--mcp-allow" | "--mcp-deny", "alarms") => cfg.mcp.allow_alarms = on,
                        ("--mcp-allow" | "--mcp-deny", "names") => cfg.mcp.allow_names = on,
                        ("--mcp-allow" | "--mcp-deny", "updates") => cfg.mcp.allow_updates = on,
                        ("--mcp-files", folder) => {
                            if !std::path::Path::new(folder).is_dir() {
                                eprintln!("{folder} isn't a folder that exists");
                                return 2;
                            }
                            cfg.mcp.files_folder = folder.to_string();
                            cfg.mcp.allow_files = true;
                        }
                        _ => {
                            eprintln!("{flag} takes saved, alarms, names or updates (see --help)");
                            return 2;
                        }
                    }
                }
            }
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
    if m.allow_control {
        let yn = |b: bool| if b { "yes" } else { "no" };
        println!(
            "  also: saved items {}, alarms {}, room names and sound {}, speaker updates {}, files {}",
            yn(m.allow_saved),
            yn(m.allow_alarms),
            yn(m.allow_names),
            yn(m.allow_updates),
            if m.allow_files && !m.files_folder.trim().is_empty() { format!("yes, in {}", m.files_folder) } else { "no".to_string() }
        );
    }
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
    frame::start(frame::Frame::new(b.clone()), backend::Waker::none());
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
