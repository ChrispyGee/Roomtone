import streamDeck from "@elgato/streamdeck";

import { client, loadGlobalSettings } from "./context.ts";
import { RoomtoneError } from "./roomtone.ts";
import { NowPlayingKey, Next, PauseAll, PlayPause, Previous, Scene } from "./actions/transport.ts";
import { Mute, VolumeDial, VolumeDown, VolumeUp } from "./actions/volume.ts";

streamDeck.logger.setLevel("info");

for (const a of [new PlayPause(), new Next(), new Previous(), new PauseAll(), new NowPlayingKey(), new Scene(), new VolumeUp(), new VolumeDown(), new Mute(), new VolumeDial()]) {
  streamDeck.actions.registerAction(a);
}

// The settings page asks the plugin to try the connection, and for the names of the rooms and scenes.
streamDeck.ui.onSendToPlugin(async (ev) => {
  const request = (ev.payload as { request?: string } | undefined)?.request;
  try {
    if (request === "check") {
      await client.check();
      await streamDeck.ui.sendToPropertyInspector({ reply: "check", ok: true, message: "Connected to Roomtone." });
    } else if (request === "rooms") {
      await streamDeck.ui.sendToPropertyInspector({ reply: "rooms", ok: true, rooms: await client.listRooms() });
    } else if (request === "scenes") {
      await streamDeck.ui.sendToPropertyInspector({ reply: "scenes", ok: true, scenes: await client.listScenes() });
    }
  } catch (e) {
    const message = e instanceof RoomtoneError ? e.message : "Something went wrong.";
    await streamDeck.ui.sendToPropertyInspector({ reply: request ?? "", ok: false, message });
  }
});

await streamDeck.connect();
await loadGlobalSettings();
