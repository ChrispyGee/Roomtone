import assert from "node:assert/strict";
import { test } from "node:test";

import { Monitor } from "../src/monitor.ts";
import { RoomtoneError, type NowPlaying } from "../src/roomtone.ts";

const song = (room: string): NowPlaying => ({ state: "playing", title: `Song in ${room}`, artist: "", album: "", volume: 20, muted: false, room });

test("buttons that share a room share one question, and each hears the answer", async () => {
  const asked: (string | undefined)[] = [];
  const monitor = new Monitor({ nowPlaying: async (room) => (asked.push(room), song(room ?? "selected")) }, 60_000);
  const heard: string[] = [];
  const stopA = monitor.watch("Kitchen", (now) => heard.push(`A:${now?.title}`));
  const stopB = monitor.watch(" Kitchen ", (now) => heard.push(`B:${now?.title}`));
  const stopC = monitor.watch(undefined, (now) => heard.push(`C:${now?.title}`));
  await monitor.refresh();
  await monitor.refresh();
  assert.deepEqual([...new Set(asked)].sort(), ["Kitchen", undefined].sort(), "Kitchen once, the selected room once");
  assert.ok(heard.includes("A:Song in Kitchen") && heard.includes("B:Song in Kitchen") && heard.includes("C:Song in selected"));
  stopA();
  stopB();
  stopC();
  assert.equal(monitor.watching, 0, "nothing is asked for when no button is on screen");
});

test("a problem is passed on to the buttons that were waiting", async () => {
  const monitor = new Monitor({ nowPlaying: async () => { throw new RoomtoneError("Roomtone refused the access key.", "key"); } }, 60_000);
  const problems: string[] = [];
  const stop = monitor.watch(undefined, (_now, problem) => problems.push(problem?.kind ?? "none"));
  await monitor.refresh();
  assert.ok(problems.length >= 1 && problems.every((p) => p === "key"));
  stop();
});

test("a button that is given another room follows that one and lets the old one go", async () => {
  const asked: string[] = [];
  const monitor = new Monitor({ nowPlaying: async (room) => (asked.push(room ?? "selected"), song(room ?? "selected")) }, 60_000);
  const heard: string[] = [];
  monitor.follow("key-1", "Cinema", (now) => heard.push(now?.room ?? ""));
  await monitor.refresh();
  asked.length = 0;
  heard.length = 0;
  monitor.follow("key-1", "Office", (now) => heard.push(now?.room ?? ""));
  await monitor.refresh();
  assert.deepEqual([...new Set(asked)], ["Office"], "only the new room is asked about");
  assert.ok(heard.length >= 1 && heard.every((r) => r === "Office"));
  assert.equal(monitor.watching, 1);
  monitor.unfollow("key-1");
  assert.equal(monitor.watching, 0);
});
