// Talks to a real Roomtone. Skipped unless ROOMTONE_LIVE_KEY is set (and ROOMTONE_LIVE_PORT for a port other than 8790).
// Use a demo Roomtone: `roomtone --headless --demo`, never real speakers.
import assert from "node:assert/strict";
import { test } from "node:test";

import { RoomtoneClient, RoomtoneError } from "../src/roomtone.ts";

const key = process.env.ROOMTONE_LIVE_KEY;
const port = Number(process.env.ROOMTONE_LIVE_PORT ?? 8790);

test("the buttons' calls work against a real (demo) Roomtone", { skip: !key }, async () => {
  const client = new RoomtoneClient(() => ({ host: "127.0.0.1", port, key: key ?? "" }));
  const before = await client.nowPlaying("Living Room");
  assert.ok(before && before.title.length > 0, "now playing reads");
  await client.changeVolume(5, "Living Room");
  const louder = await client.nowPlaying("Living Room");
  assert.equal(louder?.volume, (before?.volume ?? 0) + 5, "volume up");
  await client.changeVolume(-5, "Living Room");
  assert.equal((await client.nowPlaying("Living Room"))?.volume, before?.volume, "and back down");
  await client.playback("pause", "Living Room");
  assert.equal((await client.nowPlaying("Living Room"))?.state, "paused");
  await client.playback("toggle", "Living Room");
  assert.equal((await client.nowPlaying("Living Room"))?.state, "playing");
  await client.setMute(true, "Living Room");
  assert.equal((await client.nowPlaying("Living Room"))?.muted, true);
  await client.setMute(false, "Living Room");
  await client.check();
  assert.ok(Array.isArray(await client.listScenes()));
  await assert.rejects(new RoomtoneClient(() => ({ host: "127.0.0.1", port, key: "wrong" })).check(), (e: unknown) => e instanceof RoomtoneError && e.kind === "key");
});
