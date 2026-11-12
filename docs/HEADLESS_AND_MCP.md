# Running without a window, and connecting AI assistants (MCP)

Roomtone can run with no window and let AI assistants that speak the **Model Context Protocol** see and control your speakers: "what's playing in the kitchen?", "play my Morning Coffee playlist in the living room", "group the downstairs rooms".

> **Tip:** run `roomtone --help mcp` on the computer you're setting up. It prints these steps with that computer's real paths filled in. The app shows the same steps under **Settings › Assistant link (MCP) › How to connect**.

In the commands below, replace `roomtone` with the full path to the program if it isn't on your `PATH`:

| Platform | Typical path |
| --- | --- |
| macOS | `/Applications/Roomtone.app/Contents/MacOS/roomtone`, or wherever you built it: `…/app/target/release/roomtone` |
| Windows | `C:\Program Files\Roomtone\roomtone.exe`, or `…\app\target\release\roomtone.exe` |
| Linux | `~/.local/bin/roomtone`, or `…/app/target/release/roomtone` |

---

## 1. Switch the assistant link on (all platforms)

The link is **off** until you switch it on. Either:

- in the app: **Settings › Assistant link (MCP) › Let AI assistants use Roomtone**, or
- in a terminal: `roomtone --mcp-on`

Other settings (each saves and quits):

```sh
roomtone --mcp-read-only        # assistants may only look (status, search, lists)
roomtone --mcp-allow-control    # assistants may also play, change volume, edit the queue and group rooms
roomtone --mcp-port 8790        # the HTTP port
roomtone --mcp-off              # switch the link off again; takes effect at once, even for running copies
roomtone --mcp-info             # show the settings, the web address and the access key
```

**Allow other computers** (accept assistants elsewhere in your home) can only be switched in the app's Settings, on purpose.

## 2. Pick how the assistant connects

| Way | Use it when | Command |
| --- | --- | --- |
| **stdin/stdout** | The assistant starts programs itself (most desktop assistants, Claude Code) | `roomtone --mcp` (the assistant runs this) |
| **HTTP** | The assistant connects to an address, or several assistants share one copy | Keep the app open, or run `roomtone --headless` |

Both find your speakers by themselves. Add `--demo` to try them with made-up rooms.

---

## macOS

### Desktop assistants (stdin/stdout)
1. In the app choose **Settings › Assistant link › Copy setup**, or write it by hand:
   ```json
   {
     "mcpServers": {
       "roomtone": {
         "command": "/Applications/Roomtone.app/Contents/MacOS/roomtone",
         "args": ["--mcp"]
       }
     }
   }
   ```
2. Paste it into the assistant's MCP settings. For Claude Desktop that's `~/Library/Application Support/Claude/claude_desktop_config.json`; merge it with any `mcpServers` already there.
3. Restart the assistant.

### Claude Code
```sh
claude mcp add roomtone -- /Applications/Roomtone.app/Contents/MacOS/roomtone --mcp
# or, over HTTP while the app or --headless runs:
claude mcp add --transport http roomtone http://127.0.0.1:8790/mcp --header "Authorization: Bearer <access key>"
```

### Keep it running in the background
Save as `~/Library/LaunchAgents/com.roomtone.headless.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.roomtone.headless</string>
  <key>ProgramArguments</key><array>
    <string>/Applications/Roomtone.app/Contents/MacOS/roomtone</string><string>--headless</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict></plist>
```
```sh
launchctl load ~/Library/LaunchAgents/com.roomtone.headless.plist     # start now and at every login
launchctl unload ~/Library/LaunchAgents/com.roomtone.headless.plist   # stop
```

### Good to know
- macOS asks before an app may use the local network. If no speakers appear, allow **Roomtone**, or the **Terminal** you started it from, in **System Settings › Privacy & Security › Local Network**.
- Settings: `~/Library/Application Support/Roomtone/config.json`.

---

## Windows

### Desktop assistants (stdin/stdout)
1. In the app choose **Settings › Assistant link › Copy setup**, or write it by hand. Backslashes are doubled in JSON:
   ```json
   {
     "mcpServers": {
       "roomtone": {
         "command": "C:\\Program Files\\Roomtone\\roomtone.exe",
         "args": ["--mcp"]
       }
     }
   }
   ```
2. Paste it into the assistant's MCP settings. For Claude Desktop that's `%APPDATA%\Claude\claude_desktop_config.json`.
3. Restart the assistant.

### Claude Code (PowerShell)
```powershell
claude mcp add roomtone -- "C:\Program Files\Roomtone\roomtone.exe" --mcp
claude mcp add --transport http roomtone http://127.0.0.1:8790/mcp --header "Authorization: Bearer <access key>"
```

### Keep it running in the background
Start it at sign-in with Task Scheduler (PowerShell):
```powershell
$a = New-ScheduledTaskAction -Execute 'C:\Program Files\Roomtone\roomtone.exe' -Argument '--headless'
$t = New-ScheduledTaskTrigger -AtLogOn
Register-ScheduledTask -TaskName 'Roomtone' -Action $a -Trigger $t
Start-ScheduledTask -TaskName 'Roomtone'        # start now
Stop-ScheduledTask -TaskName 'Roomtone'         # stop
Unregister-ScheduledTask -TaskName 'Roomtone'   # remove
```
Or run `roomtone.exe --headless` in a terminal window and leave it open.

### Good to know
- The first time it runs, Windows Firewall may ask whether Roomtone may use the network. **Allow it on private networks**: speakers are found on UDP port 1900 and fetch music from this computer on TCP port 52880.
- Settings: `%APPDATA%\Roomtone\config.json`.
- Language "Automatic" uses English on Windows unless `LANG` or `ROOMTONE_LANG` is set; pick a language in Settings instead.
- Quick test in PowerShell: `curl.exe -X POST http://127.0.0.1:8790/mcp -H "Authorization: Bearer <key>" -H "Content-Type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}"`

---

## Linux

### Desktop assistants (stdin/stdout)
Add to the assistant's MCP settings (for Claude-style apps usually `~/.config/Claude/claude_desktop_config.json`):
```json
{ "mcpServers": { "roomtone": { "command": "/home/you/.local/bin/roomtone", "args": ["--mcp"] } } }
```

### Claude Code
```sh
claude mcp add roomtone -- ~/.local/bin/roomtone --mcp
claude mcp add --transport http roomtone http://127.0.0.1:8790/mcp --header "Authorization: Bearer <access key>"
```

### Keep it running in the background (systemd user service)
Save as `~/.config/systemd/user/roomtone.service`:
```ini
[Unit]
Description=Roomtone speaker controller (headless)
After=network-online.target

[Service]
ExecStart=/home/you/.local/bin/roomtone --headless
Restart=on-failure

[Install]
WantedBy=default.target
```
```sh
systemctl --user daemon-reload
systemctl --user enable --now roomtone   # start now and at login
journalctl --user -u roomtone -f         # watch its status lines
loginctl enable-linger "$USER"           # optional: keep running after you log out
```

### Good to know
- With a firewall on (ufw, firewalld), allow **UDP 1900** (finding speakers), **TCP 52880** (speakers fetching music from this computer) and the link's port if other computers should connect.
- Settings: `~/.config/roomtone/config.json`.
- `--headless` and `--mcp` need no display, so they also run on a home server or Raspberry Pi.

---

## 3. Check it works

```sh
curl -X POST http://127.0.0.1:8790/mcp \
  -H 'Authorization: Bearer <access key>' -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```
You should get a list of tools. Over stdin/stdout:
```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
              '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_status","arguments":{}}}' \
  | roomtone --mcp --demo
```

| Reply | Means |
| --- | --- |
| `-32001 … switched off` (stdio) / connection refused (HTTP) | Switch the link on (step 1) |
| `401` | Missing or wrong access key: copy it again from Settings or `--mcp-info` |
| `403` | Request came from a web page, or from another computer while *Allow other computers* is off |
| "Couldn't open port 8790" in Settings | Another program, or another copy of Roomtone, already uses the port; pick another with `--mcp-port` |
| Tools list without play/volume tools | *Assistants can change things* is off (look-only) |

## 4. Safety

- Off by default; switching it off takes effect at once.
- HTTP listens on this computer only (`127.0.0.1`) unless you allow other computers. It always requires the access key, refuses requests from web pages (checked by `Origin`, and JSON only), and with other computers off it also refuses other host names.
- Look-only mode hides and refuses every tool that changes something.
- The access key is in `config.json`, readable only by you. **New key** in Settings replaces it.

## 5. What assistants can do

See the tool table in [SPEC.md §6.3](../SPEC.md#63-tools): status, now playing, playback, volume and mute, play mode, seek, sleep timer, favorites, playlists, search and play, queue, grouping and saved groups, play along across S1/S2, line-in/TV, sound settings and alarms.
