//! Command-line control: `roomtone play`, `roomtone volume 30 --room Kitchen`, … for scripts,
//! Stream Deck and shortcuts. Each command is sent to the Roomtone that's already running (the
//! window or `--headless`) over its assistant link, so that link has to be switched on and
//! (for anything that changes things) allowed to control. Nothing leaves this computer.

use crate::config::McpConfig;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Words that mean "this is a control command", not an option.
pub const COMMANDS: &[&str] =
    &["status", "rooms", "now", "play", "pause", "toggle", "next", "previous", "volume", "mute", "unmute", "group", "ungroup", "sleep", "queue", "favorites", "alarms"];

pub fn is_command(word: &str) -> bool {
    COMMANDS.contains(&word)
}

/// What to ask the link for: a tool name and its arguments.
#[derive(Debug, PartialEq)]
pub struct Call {
    pub tool: &'static str,
    pub args: Value,
}

fn call(tool: &'static str, args: Value) -> Result<Call, String> {
    Ok(Call { tool, args })
}

/// Turn `["volume", "+5", "--room", "Kitchen"]` into the call it means.
pub fn plan(words: &[String]) -> Result<Call, String> {
    let mut room: Option<String> = None;
    let mut whole_group = false;
    let mut rest: Vec<&str> = vec![];
    let mut it = words.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--room" | "-r" => room = Some(it.next().ok_or("--room needs a room name")?.clone()),
            "--group" | "-g" => whole_group = true,
            _ => rest.push(w),
        }
    }
    let with_room = |mut v: Value| {
        if let (Some(r), Some(o)) = (&room, v.as_object_mut()) {
            o.insert("room".into(), json!(r));
        }
        v
    };
    let Some((&cmd, args)) = rest.split_first() else { return Err("Say what to do, e.g. `play`, `pause`, `volume 30` (see --help)".into()) };
    match cmd {
        "status" | "rooms" => call("get_status", json!({})),
        "now" => call("now_playing", with_room(json!({}))),
        "queue" => call("get_queue", with_room(json!({}))),
        "favorites" => call("list_favorites", with_room(json!({}))),
        "alarms" => call("list_alarms", json!({})),
        "pause" | "toggle" | "next" | "previous" => call("playback", with_room(json!({"action": cmd}))),
        "play" if args.is_empty() => call("playback", with_room(json!({"action": "play"}))),
        "play" => call("play", with_room(json!({"name": args.join(" ")}))),
        "mute" | "unmute" => call("set_mute", with_room(json!({"muted": cmd == "mute", "whole_group": whole_group}))),
        "volume" => {
            let v = args.first().ok_or("volume needs a number: 30, +5 or -5")?;
            let n: i64 = v.trim_start_matches('+').parse().map_err(|_| format!("'{v}' isn't a volume: use 0–100, or +5 / -5 to change it"))?;
            if v.starts_with('+') || v.starts_with('-') {
                call("set_volume", with_room(json!({"change": n, "whole_group": whole_group})))
            } else if (0..=100).contains(&n) {
                call("set_volume", with_room(json!({"volume": n, "whole_group": whole_group})))
            } else {
                Err("Volume goes from 0 to 100".into())
            }
        }
        "group" if args.len() >= 2 => call("group_rooms", json!({"rooms": args})),
        "group" => Err("group needs at least two room names".into()),
        "ungroup" => call("ungroup_room", with_room(json!({}))),
        "sleep" => {
            let m: i64 = args.first().and_then(|m| m.parse().ok()).ok_or("sleep needs minutes (0 turns the timer off)")?;
            call("sleep_timer", with_room(json!({"minutes": m})))
        }
        other => Err(format!("Unknown command '{other}' (see --help)")),
    }
}

/// Send one tool call to the running Roomtone. `Err((code, message))` carries the exit code.
pub fn send(cfg: &McpConfig, c: &Call) -> Result<(String, bool), (i32, String)> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": c.tool, "arguments": c.args}}).to_string();
    let addr = format!("127.0.0.1:{}", cfg.port);
    let mut s = TcpStream::connect_timeout(&addr.parse().map_err(|_| (3, "Bad port".to_string()))?, Duration::from_secs(3)).map_err(|_| {
        (3, "Roomtone isn't running, or its assistant link is off. Open the app (or start it with --headless) and switch the link on in Settings › Assistant link.".to_string())
    })?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        cfg.port,
        cfg.token,
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| (3, e.to_string()))?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).map_err(|e| (3, e.to_string()))?;
    let (head, payload) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status: u16 = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    match status {
        200 => {}
        401 => return Err((3, "The access key was refused. Is this the same settings folder as the running Roomtone?".into())),
        503 => return Err((3, "The assistant link is switched off. Switch it on in Settings › Assistant link.".into())),
        other => return Err((3, format!("Roomtone answered with HTTP {other}"))),
    }
    let v: Value = serde_json::from_str(payload).map_err(|e| (1, format!("Couldn't read the answer: {e}")))?;
    if let Some(e) = v["error"]["message"].as_str() {
        return Err((1, e.to_string()));
    }
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
    Ok((text, v["result"]["isError"].as_bool().unwrap_or(false)))
}

/// Run a command line and return the exit code (0 done, 1 refused or failed, 2 not understood,
/// 3 can't reach Roomtone).
pub fn run(words: &[String]) -> i32 {
    let c = match plan(words) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    match send(&crate::config::Config::load_readonly().mcp, &c) {
        Ok((text, false)) => {
            println!("{text}");
            0
        }
        Ok((text, true)) => {
            eprintln!("{text}");
            1
        }
        Err((code, msg)) => {
            eprintln!("{msg}");
            code
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn p(s: &str) -> Result<Call, String> {
        plan(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn commands_become_tool_calls() {
        assert_eq!(p("pause").unwrap(), Call { tool: "playback", args: json!({"action": "pause"}) });
        assert_eq!(p("next --room Kitchen").unwrap().args, json!({"action": "next", "room": "Kitchen"}));
        assert_eq!(p("play").unwrap().args, json!({"action": "play"}));
        assert_eq!(p("play Morning Mix").unwrap(), Call { tool: "play", args: json!({"name": "Morning Mix"}) });
        assert_eq!(p("volume 30 -r Den").unwrap().args, json!({"volume": 30, "whole_group": false, "room": "Den"}));
        assert_eq!(p("volume -5 --group").unwrap().args, json!({"change": -5, "whole_group": true}));
        assert_eq!(p("volume +10").unwrap().args["change"], 10);
        assert_eq!(p("mute").unwrap().args["muted"], true);
        assert_eq!(p("unmute").unwrap().args["muted"], false);
        assert_eq!(p("group Kitchen Den").unwrap().args, json!({"rooms": ["Kitchen", "Den"]}));
        assert_eq!(p("sleep 30").unwrap().args["minutes"], 30);
        assert_eq!(p("status").unwrap().tool, "get_status");
    }

    #[test]
    fn mistakes_are_explained() {
        assert!(p("").is_err());
        assert!(p("volume").is_err());
        assert!(p("volume loud").is_err());
        assert!(p("volume 101").is_err());
        assert!(p("group Kitchen").is_err());
        assert!(p("sleep soon").is_err());
        assert!(p("dance").is_err());
        assert!(p("pause --room").is_err());
    }

    #[test]
    fn every_command_word_is_understood() {
        for w in COMMANDS {
            let r = p(&format!("{w} 5 6"));
            assert!(!matches!(&r, Err(e) if e.starts_with("Unknown")), "{w}");
        }
    }
}
