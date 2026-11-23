# Privacy

Roomtone is a free, open-source controller for Sonos-compatible speakers. This page says what it does with information. The code is open, so every statement here can be checked; the full list of network connections is in [SPEC.md §9](SPEC.md).

**In short: no telemetry, no ads, no accounts of its own. Your data stays on your computer.**

## The app

- **Nothing is sent anywhere except** your speakers, the music sources you choose to use, the streaming services you sign in to and, only if you switch *Lyrics* on, lrclib.net (it is told the song's title, artist and album to find the words).
- **Your data stays on your computer.** Settings, scenes, sign-ins, the assistant access key and your listening history are files in your user folder, readable only by you. Listening history can be switched off or cleared in Settings.
- **Looking for a new version is your choice.** It is off until you switch it on. When on, Roomtone reads this project's latest release record from GitHub when it starts and about once a day; the only thing that identifies the app is its normal User-Agent with the version number. A new version is downloaded only when you press *Download*, and nothing is installed for you.
- **The assistant link** (for AI assistants on your own computer or network) is **off by default**, needs an access key, has a look-only mode and a switch for each kind of change.
- **It works offline.** You don't need the internet to control speakers on your own network.
- **This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.**

## The website

No cookies, no analytics, no advertising and no third-party fonts or scripts. The download page asks GitHub which version is newest so it can show the right file; GitHub can see your address when that happens, as it does on any GitHub page.

## Where you get Roomtone

Downloading from GitHub, Homebrew, the Microsoft Store or another channel is covered by that service's own privacy policy. Roomtone itself does not know where it came from.

## Questions

[Open an issue](https://github.com/ChrispyGee/Roomtone/issues) and ask.
