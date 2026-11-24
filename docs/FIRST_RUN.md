# Opening Roomtone for the first time

**Mac downloads from version 0.9.6 are signed and notarised by Apple and open normally. On Windows, the [Microsoft Store](https://apps.microsoft.com/detail/9MXNHZ6K9NJQ) version is signed by Microsoft; the installer on GitHub isn't signed, so Windows warns you the first time.** That is expected, and you only have to do this once. This page walks through it, and explains why.

## Why the warning

macOS and Windows show a warning for any program that isn't signed with a certificate from a company they know. Roomtone's Mac downloads now are (Apple Developer ID, from version 0.9.6). Roomtone is a free, open-source hobby project, and a free signing service for the GitHub Windows installer was declined, so the installer there stays unsigned; the Microsoft Store version is signed by Microsoft. Mac steps below apply to **versions before 0.9.6** only.

What you can do instead to be sure you have the real thing:

- **Check the download.** Every release has a `SHA256SUMS` file with each file's checksum. Download it into the same folder as the installer, then run the matching command (it should say `OK`):
  - macOS: `shasum -a 256 -c SHA256SUMS --ignore-missing`
  - Linux: `sha256sum -c SHA256SUMS --ignore-missing`
  - Windows (PowerShell): `Get-FileHash .\roomtone-vX.Y.Z-windows-x64-setup.exe` and compare the result with the line in `SHA256SUMS`.
- **Read the code or build it yourself.** It's all at <https://github.com/ChrispyGee/Roomtone>; the [README](../README.md#build-it-yourself) shows how to build it. Roomtone sends nothing anywhere except to your speakers and the music services you connect (see the privacy section of the [README](../README.md)).

## macOS (versions before 0.9.6)

From 0.9.6 the `.dmg` is signed and notarised: open it, drag Roomtone to Applications and open it. The steps below are only for older downloads.

Roomtone for Mac is built for **Apple silicon** (M1 and later). There's no Intel Mac download yet; if you have an Intel Mac you can [build it yourself](../README.md#build-it-yourself).

1. Open the `.dmg` and drag **Roomtone** into **Applications**.
2. Double-click Roomtone. macOS says it can't verify the app (or that it "was blocked to protect your Mac"). Click **Done**, not "Move to Trash".
3. Open **System Settings › Privacy & Security** and scroll down to **Security**. You'll see a message about Roomtone with an **Open Anyway** button. Click it, click **Open** again, and enter your password or use Touch ID.

From now on Roomtone opens normally. (On macOS 14 Sonoma and earlier you can instead right-click, or Control-click, Roomtone in Applications, choose **Open**, then **Open** again. Newer macOS removed that shortcut.)

Comfortable with Terminal? This removes the "downloaded from the internet" mark from Roomtone only, and then it opens normally:

```sh
xattr -dr com.apple.quarantine /Applications/Roomtone.app
```

**Installed with Homebrew** (`brew install --cask ChrispyGee/roomtone/roomtone`)? It's the same app, so the same steps apply.

If macOS says the app **"is damaged and can't be opened"**, the download was probably interrupted or changed: delete it, download it again and check the checksum (above).

### The permission questions

- **"Roomtone would like to find and connect to devices on your local network":** choose **Allow**. Without it Roomtone can't see your speakers. If you said no by mistake, switch it on in **System Settings › Privacy & Security › Local Network**.
- **"Do you want the application Roomtone to accept incoming network connections?":** choose **Allow**. Your speakers use it to fetch music from this computer's folders and to tell Roomtone what they're doing.

## Windows (10 and 11)

The simplest route is the **[Microsoft Store](https://apps.microsoft.com/detail/9MXNHZ6K9NJQ)**: install it there and none of the steps below are needed. For the installer from GitHub:

Roomtone for Windows is built for 64-bit Intel and AMD computers. It should also run on Windows on ARM through Windows' own emulation, but that hasn't been tried.

1. Run `roomtone-…-windows-x64-setup.exe`. It installs for your user only; no administrator rights are needed.
2. If you see **"Windows protected your PC"** (SmartScreen), click **More info**, then **Run anyway**.
3. The first time Roomtone starts, **Windows Defender Firewall** asks whether to let it use the network. Tick **Private networks** and click **Allow access**. Without that, Roomtone can't find your speakers or serve music from this computer.

## Linux

The `.deb` and `.rpm` packages install with `sudo apt install ./file.deb` or `sudo dnf install ./file.rpm`; there's no warning screen, but the packages aren't signed either, so your package manager may say so. If you run a firewall (ufw, firewalld), open **UDP 1900** (finding speakers) and **TCP 52880** (speakers fetching music from this computer); see the [guide](HEADLESS_AND_MCP.md).

## If it still won't open

Tell us, with your system (macOS or Windows version) and the exact words on the warning: [open an issue](https://github.com/ChrispyGee/Roomtone/issues/new/choose). Please don't turn off Gatekeeper or SmartScreen for your whole computer to get past it; the steps above only trust Roomtone.
