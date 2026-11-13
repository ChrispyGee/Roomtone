//! Command-line help, written for the platform the app is running on: where its settings
//! live, how to keep it running without a window, and how to connect an AI assistant.
//! `roomtone --help` prints the options; `roomtone --help mcp` prints the assistant guide.

use crate::config::{Config, McpConfig};

/// Where the project lives: source code, issues, releases.
pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Where every setting is explained (the settings guide on the project page).
pub fn settings_guide_url() -> String {
    format!("{REPO_URL}/blob/main/docs/SETTINGS.md")
}

/// This program's own path, quoted if it has spaces, for commands the person can paste.
pub fn exe() -> String {
    let p = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "roomtone".into());
    if p.contains(' ') {
        format!("\"{p}\"")
    } else {
        p
    }
}

fn exe_raw() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "roomtone".into())
}

fn data_dir() -> String {
    Config::data_dir().map(|p| p.display().to_string()).unwrap_or_else(|| "(no home folder)".into())
}

/// The name of the platform, as the person knows it.
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

/// Where desktop assistants that use a `claude_desktop_config.json`-style file keep it.
pub fn assistant_config_file() -> &'static str {
    if cfg!(target_os = "macos") {
        "~/Library/Application Support/Claude/claude_desktop_config.json"
    } else if cfg!(target_os = "windows") {
        "%APPDATA%\\Claude\\claude_desktop_config.json"
    } else {
        "~/.config/Claude/claude_desktop_config.json"
    }
}

/// The JSON block that tells an assistant to start this app as its MCP server.
pub fn stdio_setup() -> String {
    let setup = serde_json::json!({"mcpServers": {"roomtone": {"command": exe_raw(), "args": ["--mcp"]}}});
    serde_json::to_string_pretty(&setup).unwrap_or_default()
}

pub fn general() -> String {
    let e = exe();
    format!(
        "Roomtone {version} — a controller for S1 and S2 speaker systems ({platform})

Usage: {e} [options]

Window
  (no options)              open the app
  --demo                    use made-up demo rooms instead of searching the network
  --lang <code>             language for this run: en, de, fr, es, it, nl
  --theme <mode>            appearance for this run: auto, light, dark
  --view <page>             open a page first: home, search, favorites, playlists, library,
                            radio, inputs, alarms, settings, nowplaying, services, history, speakers, finder
  --screenshot <file.png>   draw about 120 frames, save a picture of the window, then quit

Without a window
  --headless                find the speakers, keep them up to date and serve the assistant
                            link over HTTP while it's switched on; stop with Ctrl+C
  --mcp                     serve the assistant link on stdin/stdout (assistants start the
                            app this way themselves)

Assistant link (MCP) settings — each saves and quits
  --mcp-on, --mcp-off       switch the link on or off
  --mcp-read-only           assistants may only look
  --mcp-allow-control       assistants may also play, change volume, edit queues and group
  --mcp-port <port>         port for the HTTP link (default 8790)
  --mcp-info                show the link's settings, address and access key

Control from a terminal or script (talks to the Roomtone that's already running, with
its assistant link switched on)
  {e} play | pause | toggle | next | previous      [--room NAME]
  {e} play NAME                                    a favorite or playlist, by name
  {e} volume 30 | +5 | -5                          [--room NAME] [--group]
  {e} mute | unmute | sleep MINUTES | ungroup      [--room NAME]
  {e} group ROOM ROOM …                            play these rooms together
  {e} status | now | queue | favorites | alarms    look only
  {e} scene NAME | scenes                          do a saved scene; list them
  {e} backup FILE.json                             save a backup of favorites, playlists, alarms and settings
  Exit code 0 done, 1 refused, 2 not understood, 3 can't reach Roomtone.

Help
  --help                    this text
  --help mcp                how to run without a window and connect an assistant on {platform}

Settings are kept in: {dir}
Every setting explained: {guide}
Source code, issues and releases: {repo}
",
        version = env!("CARGO_PKG_VERSION"),
        platform = platform(),
        dir = data_dir(),
        guide = settings_guide_url(),
        repo = REPO_URL,
    )
}

/// Running headless and connecting assistants, with the steps for this platform.
pub fn mcp(m: &McpConfig) -> String {
    let e = exe();
    let addr = crate::mcp::address(m);
    let state = format!(
        "Right now: the link is {}; assistants may {}; other computers {}.",
        if m.enabled { "ON" } else { "OFF" },
        if m.allow_control { "look and change things" } else { "only look" },
        if m.allow_network { "may connect (with the access key)" } else { "may not connect" }
    );
    let mut out = format!(
        "Roomtone on {platform}: running without a window and connecting an AI assistant (MCP)
{line}

{state}

1. Switch the assistant link on
   In the app: Settings › Assistant link (MCP) › Let AI assistants use Roomtone.
   Or here:    {e} --mcp-on
   Optional:   {e} --mcp-read-only      (assistants can only look)
               {e} --mcp-port 8790      (change the HTTP port)

2a. Assistants that start Roomtone themselves (stdin/stdout)
   Most desktop assistants. Add this to their MCP settings
   ({cfg_file} for Claude Desktop):

{setup}

   Claude Code:  claude mcp add roomtone -- {e} --mcp

2b. Assistants that connect to a web address (HTTP)
   Keep Roomtone running, either the app's window or:
       {e} --headless
   Address:  {addr}
   Header:   Authorization: Bearer <access key>   (see: {e} --mcp-info)
   Claude Code:  claude mcp add --transport http roomtone {addr} --header \"Authorization: Bearer <access key>\"
   Quick test:   {curl}
",
        platform = platform(),
        line = "=".repeat(70),
        cfg_file = assistant_config_file(),
        setup = stdio_setup().lines().map(|l| format!("     {l}")).collect::<Vec<_>>().join("\n"),
        curl = curl_test(&addr),
    );
    out.push_str(&keep_running(&e));
    out.push_str(&format!(
        "
4. Good to know on {platform}
{notes}
   Settings live in {dir} (config.json; it holds the access key, so keep it private).
   Run only one copy with the HTTP link at a time: they would share the port.
   Turning the link off in Settings (or with --mcp-off) stops assistants straight away,
   including a copy already running with --headless.
",
        platform = platform(),
        dir = data_dir(),
        notes = platform_notes(),
    ));
    out
}

fn curl_test(addr: &str) -> String {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    if cfg!(target_os = "windows") {
        format!("curl.exe -X POST {addr} -H \"Authorization: Bearer <access key>\" -H \"Content-Type: application/json\" -d \"{}\"", body.replace('"', "\\\""))
    } else {
        format!("curl -X POST {addr} -H 'Authorization: Bearer <access key>' -H 'Content-Type: application/json' -d '{body}'")
    }
}

/// How to start `--headless` automatically, the usual way on each platform.
fn keep_running(e: &str) -> String {
    let raw = exe_raw();
    if cfg!(target_os = "macos") {
        format!(
            "
3. Keep it running in the background (optional)
   Save this as ~/Library/LaunchAgents/com.roomtone.headless.plist:

     <?xml version=\"1.0\" encoding=\"UTF-8\"?>
     <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
     <plist version=\"1.0\"><dict>
       <key>Label</key><string>com.roomtone.headless</string>
       <key>ProgramArguments</key><array><string>{raw}</string><string>--headless</string></array>
       <key>RunAtLoad</key><true/>
       <key>KeepAlive</key><true/>
     </dict></plist>

   Then:  launchctl load ~/Library/LaunchAgents/com.roomtone.headless.plist
   Stop:  launchctl unload ~/Library/LaunchAgents/com.roomtone.headless.plist
"
        )
    } else if cfg!(target_os = "windows") {
        format!(
            "
3. Keep it running in the background (optional)
   Start it when you sign in, with Task Scheduler (in PowerShell):

     $a = New-ScheduledTaskAction -Execute '{raw}' -Argument '--headless'
     $t = New-ScheduledTaskTrigger -AtLogOn
     Register-ScheduledTask -TaskName 'Roomtone' -Action $a -Trigger $t

   Stop:   Stop-ScheduledTask -TaskName 'Roomtone'
   Remove: Unregister-ScheduledTask -TaskName 'Roomtone'
   (Or run `{e} --headless` in a terminal window and leave it open.)
"
        )
    } else {
        format!(
            "
3. Keep it running in the background (optional)
   Save this as ~/.config/systemd/user/roomtone.service:

     [Unit]
     Description=Roomtone speaker controller (headless)
     After=network-online.target

     [Service]
     ExecStart={raw} --headless
     Restart=on-failure

     [Install]
     WantedBy=default.target

   Then:  systemctl --user daemon-reload && systemctl --user enable --now roomtone
   Logs:  journalctl --user -u roomtone -f
   (To keep it running after you log out: loginctl enable-linger $USER)
"
        )
    }
}

fn platform_notes() -> &'static str {
    if cfg!(target_os = "macos") {
        "   macOS asks before an app may use the local network. If no speakers appear, allow
   Roomtone (or the Terminal you started it from) in System Settings › Privacy &
   Security › Local Network."
    } else if cfg!(target_os = "windows") {
        "   Windows Firewall may ask whether Roomtone may use the network the first time it
   runs. Allow it on private networks, or the speakers can't be found (it listens for
   them on UDP port 1900) and can't fetch music from this computer (TCP port 52880)."
    } else {
        "   If a firewall is on (ufw, firewalld), allow UDP port 1900 so speakers can be found,
   TCP port 52880 so they can fetch music from this computer, and the HTTP link's port
   if other computers should connect."
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn help_lists_every_option() {
        let text = general();
        for opt in [
            "--demo",
            "--lang",
            "--theme",
            "--view",
            "--screenshot",
            "--headless",
            "--mcp",
            "--mcp-on",
            "--mcp-off",
            "--mcp-read-only",
            "--mcp-allow-control",
            "--mcp-port",
            "--mcp-info",
            "--help mcp",
        ] {
            assert!(text.contains(opt), "--help is missing {opt}");
        }
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains(REPO_URL));
    }

    #[test]
    fn assistant_guide_for_this_platform() {
        let m = crate::config::McpConfig { enabled: true, port: 8790, ..Default::default() };
        let text = mcp(&m);
        assert!(text.contains(platform()));
        assert!(text.contains("http://127.0.0.1:8790/mcp"));
        assert!(text.contains("claude mcp add"));
        assert!(text.contains("the link is ON"));
        let setup: serde_json::Value = serde_json::from_str(&stdio_setup()).unwrap();
        assert_eq!(setup["mcpServers"]["roomtone"]["args"][0], "--mcp");
    }
}
