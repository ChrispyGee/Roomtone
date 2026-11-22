import assert from "node:assert/strict";
import { test } from "node:test";

import { CoverCache, trackKey } from "../src/cover.ts";
import type { NowPlaying } from "../src/roomtone.ts";

const song = (title: string, room = "Kitchen"): NowPlaying => ({ state: "playing", title, artist: "A", album: "B", volume: 10, muted: false, room });

test("a cover is fetched once for each song, however often the key is redrawn", async () => {
  const cache = new CoverCache();
  let fetched = 0;
  const fetcher = async () => (fetched++, "data:image/jpeg;base64,AAAA");
  for (let i = 0; i < 5; i++) assert.equal(await cache.get("key", trackKey(song("One")), fetcher), "data:image/jpeg;base64,AAAA");
  assert.equal(fetched, 1);
  await cache.get("key", trackKey(song("Two")), fetcher);
  assert.equal(fetched, 2, "a new song asks again");
  await cache.get("key", trackKey(song("Two", "Den")), fetcher);
  assert.equal(fetched, 3, "so does the same title in another room");
});

test("a song with no cover isn't asked about again until the song changes", async () => {
  const cache = new CoverCache();
  let asked = 0;
  const none = async () => (asked++, undefined);
  assert.equal(await cache.get("key", "k1", none), undefined);
  assert.equal(await cache.get("key", "k1", none), undefined);
  assert.equal(asked, 1);
});

test("a failed fetch is no cover, and each key keeps its own", async () => {
  const cache = new CoverCache();
  assert.equal(await cache.get("a", "k", async () => { throw new Error("down"); }), undefined);
  assert.equal(await cache.get("b", "k", async () => "data:x"), "data:x");
  assert.equal(await cache.get("a", "k", async () => "data:y"), undefined, "a's answer for k is remembered");
  cache.forget("a");
  assert.equal(await cache.get("a", "k", async () => "data:y"), "data:y");
});
