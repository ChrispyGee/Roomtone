# How Roomtone talks to the speakers

Notes on the local protocols the app uses. Everything here was worked out from the speakers' own published service descriptions and their replies, not from any other app's code. When you learn something new about the speakers, add it here. The code lives in `src/net/` (speakers) and `src/sources/smapi.rs` (streaming services).

S1 (firmware 11.x) and S2 speakers expose **the same local services**, so one client drives both. Differences are noted where they matter.

## 1. Finding speakers

- **SSDP** `M-SEARCH` to `239.255.255.250:1900` (and the broadcast address), with `ST: urn:schemas-upnp-org:device:ZonePlayer:1` and `MX: 1`. Replies carry a `LOCATION` such as `http://<ip>:1400/xml/device_description.xml`.
- **Device description** (`/xml/device_description.xml`) gives `modelName`, `displayVersion` (e.g. `11.15`, `16.4`) and `softwareVersion` (the build, e.g. `57.19-41110`).
- **Telling S1 from S2** (`net::discovery::system_from`): display version major below 12, or build major 57 or lower, means S1. Anything newer is S2.
- **Topology**: `ZoneGroupTopology#GetZoneGroupState` returns XML of groups (`ZoneGroup Coordinator=…`) and members (`ZoneGroupMember UUID=… Location=… ZoneName=… Invisible=…`). Invisible members (bonded surrounds and subs) are hidden.
- **Household id**: `DeviceProperties#GetHouseholdID`. S1 and S2 speakers are always in different households.
- **Manual add**: the person can type an IP; the app reads that speaker's description and topology directly.

## 2. Control (UPnP/SOAP on port 1400)

Each call is an HTTP POST to `http://<ip>:1400<control path>` with `SOAPACTION: "<service urn>#<Action>"` (see `net::soap`).

| Service (control path) | Actions used |
| --- | --- |
| AVTransport (`/MediaRenderer/AVTransport/Control`) | `Play`, `Pause`, `Stop`, `Next`, `Previous`, `Seek` (`REL_TIME` and `TRACK_NR`), `SetAVTransportURI`, `AddURIToQueue`, `RemoveTrackFromQueue`, `RemoveAllTracksFromQueue`, `ReorderTracksInQueue`, `SaveQueue`, `SetPlayMode`, `GetTransportSettings`, `GetCrossfadeMode`, `SetCrossfadeMode`, `GetTransportInfo`, `GetPositionInfo`, `GetMediaInfo`, `ConfigureSleepTimer`, `GetRemainingSleepTimerDuration`, `BecomeCoordinatorOfStandaloneGroup` |
| RenderingControl (`/MediaRenderer/RenderingControl/Control`) | `GetVolume`, `SetVolume`, `GetMute`, `SetMute`, `SetBass`, `SetTreble`, `GetLoudness`, `SetLoudness`, `GetEQ`, `SetEQ` (`NightMode`, `DialogLevel`) |
| GroupRenderingControl (`/MediaRenderer/GroupRenderingControl/Control`) | `GetGroupVolume`, `SetGroupVolume` (after `SnapshotGroupVolume`), `GetGroupMute`, `SetGroupMute` |
| ContentDirectory (`/MediaServer/ContentDirectory/Control`) | `Browse` (`BrowseDirectChildren`), `CreateObject` (add favorite), `DestroyObject` (remove favorite or playlist) |
| ZoneGroupTopology (`/ZoneGroupTopology/Control`) | `GetZoneGroupState`, `CheckForUpdate`, `BeginSoftwareUpdate` |
| DeviceProperties (`/DeviceProperties/Control`) | `GetHouseholdID`, `GetZoneAttributes`, `SetZoneAttributes` (rename), `GetLEDState`, `SetLEDState` |
| AlarmClock (`/AlarmClock/Control`) | `ListAlarms`, `CreateAlarm`, `UpdateAlarm`, `DestroyAlarm` |
| MusicServices (`/MusicServices/Control`) | `ListAvailableServices` (the services this system offers, with their numbers and API addresses) |
| SystemProperties (`/SystemProperties/Control`) | `GetString` (the device id used for streaming sign-in) |

**Grouping**: a room joins a group by `SetAVTransportURI` to `x-rincon:<coordinator uuid>`, and leaves with `BecomeCoordinatorOfStandaloneGroup`. A group's id is its coordinator's uuid.

**Volume**: group volume changes move members proportionally on the speakers; the app mirrors that in its optimistic update (`backend::scale_volume`).

## 3. Browse ids (ContentDirectory)

| Id | Contents |
| --- | --- |
| `FV:2` | The system's Favorites |
| `SQ:` | Saved playlists (`SQ:<n>` one playlist) |
| `Q:0` | The queue |
| `A:ARTIST`, `A:ALBUMARTIST`, `A:ALBUM`, `A:TRACKS`, `A:GENRE`, `A:COMPOSER`, `A:PLAYLISTS` | Music library (append `:<term>` to search, `/<name>` to open) |
| `S:` | Shared folders |
| `R:0/0` | Saved radio stations |

The app adds ids of its own for other sources, all handled in `run_common`, never sent to speakers: `LOCAL:…` (this computer), `RADIO:…`, `POD:…`, `IA:…`, `SVC:<sid>/<id>` (streaming services), `ARTIST:<name>` (an artist everywhere), `ALL:…` (combined library lists), `FVSVC:<sid>` (favorites from one service) and `search:<source>:<term>`.

## 4. What the speakers can play (addresses)

| Address | Means |
| --- | --- |
| `x-rincon-queue:<uuid>#0` | The group's queue |
| `x-rincon:<uuid>` | Join that coordinator's group |
| `x-rincon-stream:<uuid>` | Another speaker's line-in |
| `x-sonos-htastream:<uuid>:spdif` | A home-theatre speaker's TV input |
| `x-rincon-buzzer:0` | The alarm chime |
| `x-rincon-mp3radio:<url>` | Plain internet radio (also used for `http` streams on S1) |
| `x-sonosapi-stream:<id>?sid=<n>&flags=…&sn=<account>` | A service's live station |
| `x-sonosapi-radio:<id>?sid=…` | A service's radio/programme |
| `x-sonosapi-hls:` / `x-sonosapi-hls-static:` | HLS streams from a service |
| `x-sonos-http:<id>.mp4?sid=…&sn=…` | A track from a service |
| `x-sonos-spotify:<id>?sid=9&…` | A Spotify track |
| `x-rincon-cpcontainer:<prefix><id>?sid=…` | A service's album or playlist as a whole (prefix `1004206c` album, `1006206c` playlist, `10052064` artist) |
| `http://<this computer>:52880/t/<code>` | A song served from this computer (`sources::local`) |

Metadata goes with every address as DIDL-Lite (`net::didl`). Service items carry `<desc id="cdudn">SA_RINCON<sid*256+7>_X_#Svc<sid*256+7>-0-Token</desc>`, which also tells which service a favorite came from (`sources::from_service`).

**Account number (`sn=`)**: the system can hold several accounts per service; `sn` picks one. It isn't published anywhere on S2, so the app reads it from what's playing or from saved items, and otherwise tries 1–6 (`backend::play_from_service`).

**S1 and https**: S1 speakers can't play some `https` streams, so the app serves them through this computer (`relays` in `songs.json`).

## 5. Streaming services (SMAPI)

Each service has its own SOAP API address, listed by `ListAvailableServices`. Calls go to that address with a `credentials` header carrying the speaker-system's `deviceId` and `householdId`, plus a `loginToken` once signed in (`sources::smapi`).

| Step | Call |
| --- | --- |
| Start sign-in | `getAppLink` (services with `AppLink` auth) or `getDeviceLinkCode` → a `regUrl` web page and a `linkCode` |
| Finish sign-in | `getDeviceAuthToken` (polled every 5 s for up to 6 minutes; `NOT_LINKED_RETRY` means "not yet") → `authToken` + `privateKey` |
| Browse | `getMetadata` (`id=root` for the home, 100 at a time, up to 600) |
| Search | `search` with the service's categories (from its presentation map), e.g. `artist`, `album`, `track`, `playlist` |
| Renewed sign-in | A `TokenRefreshRequired` fault carries new `authToken`/`privateKey`; the app retries once and saves them |

Services seen refusing third-party sign-in (2026-11-11): Apple Music (`getAppLink` returns an empty, encrypted link), YouTube Music (HTTP 403, unregistered caller), SoundCloud (`Client.NOT_AUTHORIZED`). Don't work around these by pretending to be another app.

## 6. Software updates

- `ZoneGroupTopology#CheckForUpdate(UpdateType=All, CachedOnly=0, Version="")` returns `UpdateItem`, an escaped XML element with attributes such as `Version` (a build number), `UpdateURL` and `DownloadSize`. Empty means up to date.
- `ZoneGroupTopology#BeginSoftwareUpdate(UpdateURL, Flags=0, ExtraOptions="")` makes that speaker download and install; it restarts when done.
- **Safety rule** (`backend::check_updates`, `install_update`): an offer is accepted only if its build generation matches the system (S1 ≤ 57, S2 ≥ 58). It is checked again just before installing. Never send S2 software to an S1 system.
- Not yet tried against real speakers. Whether one call per system is enough, or each speaker needs one, is an open question; the app calls every speaker in turn.

## 6a. Playlists (backup and restore)

- Reading: `ContentDirectory#Browse` of `SQ:` lists the saved playlists; browsing a playlist's id (`SQ:<n>`) lists its songs with their addresses and details.
- Creating (used by *Restore*, **not yet tried against real speakers**): `AVTransport#CreateSavedQueue(Title, EnqueuedURI, EnqueuedURIMetaData)` with the first song returns `AssignedObjectID` (`SQ:<n>`) and `NewUpdateID`; each further song is `AVTransport#AddURIToSavedQueue(ObjectID, UpdateID, EnqueuedURI, EnqueuedURIMetaData, AddAtIndex=4294967295)`, passing the previous call's `NewUpdateID`. This doesn't touch the queue that's playing.
- Favorites are created with `ContentDirectory#CreateObject` in `FV:2` (`speaker::add_favorite`); alarms with `AlarmClock#CreateAlarm`.

## 6c. Sound quality of what's playing

Read from the track's address and its DIDL-Lite `TrackMetaData` (from `GetPositionInfo`), with no extra calls (`quality::of`):

- `res@protocolInfo` third field is the MIME type: `audio/flac`, `audio/mpeg`, `audio/mp4`, `audio/ogg`, `audio/wav`… → format and whether it's lossless. **Seen on a real S1 system** for tracks from the music library (only the MIME type; no rate or depth).
- Standard UPnP `res` attributes `sampleFrequency` (Hz), `bitsPerSample`, `nrAudioChannels` and `bitrate` (bytes per second by the standard; values above 64000 are treated as bits per second: an estimate) are used if a speaker sends them.
- A `streamInfo` element, if present, is read as JSON or `key=value` pairs (`sampleRate`, `bitDepth`, `bitrate`, `codec`, `channels`); anything not understood is only listed in *Technical details*. **Not yet seen from a real speaker**: what S2 actually sends here is unconfirmed, which is why the details list and *Copy details* exist.
- Without any of these the format falls back to the song address (file extension, `x-rincon-mp3radio:` is MP3, `x-sonosapi-hls*` is AAC).

## 6b. Events (UPnP `SUBSCRIBE`)

Used by `net::events` when *Speaker updates* is on. Plain HTTP on port 1400:

| Step | Request |
| --- | --- |
| Subscribe | `SUBSCRIBE <path>` with `CALLBACK: <http://this-computer:port/>`, `NT: upnp:event`, `TIMEOUT: Second-1800` → `200` with `SID` and `TIMEOUT` |
| Renew | `SUBSCRIBE <path>` with `SID` and `TIMEOUT` (no callback) → `200` (anything else: subscribe again) |
| Cancel | `UNSUBSCRIBE <path>` with `SID` |
| Notice | The speaker sends `NOTIFY /` to the callback with `SID`, `NT: upnp:event`, `NTS: upnp:propchange`, `SEQ` and an XML property set; we answer `200` |

Paths: `/MediaRenderer/AVTransport/Event` (a group's coordinator), `/MediaRenderer/RenderingControl/Event` (each room), `/ZoneGroupTopology/Event` (one room per system). The body (`LastChange` XML) is deliberately not parsed: a notice only triggers the usual read of that group, so there is one way of reading state. Checked once against a real S1 speaker (Connect:Amp, firmware 11.16.1, 2026-11-13) with the ignored test `real_speaker_events`: all three subscriptions were accepted, each sent its first notice about 4–5 s after subscribing, the lease was 1800 s and renewing and cancelling worked. Not yet checked: S2 speakers, bonded pairs and groups, and what happens when a speaker restarts. The app counts a subscription as working only after the first notice.

## 7. Things that are guesses

Mark guesses in code comments with "estimate", and list them here.

- Which models have line-in or TV inputs (`model::Room::has_line_in` / `has_tv`) is guessed from the model name.
- Address formats for service tracks (`smapi::to_item`) were checked against common services, not every one.
- Software update behaviour (section 6).
