import assert from "node:assert/strict";
import { test } from "node:test";

import { parseNowPlaying, parseRoomNames, parseSceneNames, RoomtoneClient, RoomtoneError } from "../src/roomtone.ts";

const nowPlayingText = JSON.stringify({ state: "playing", title: "Slow Motion Summer", artist: "The Paper Lanterns", album: "Golden Hour Drive", group_volume: 30, muted: false, lead_room: "Living Room" });

/** A pretend Roomtone: records what it was sent and answers like the real assistant link does. */
function fakeRoomtone(answer: (tool: string, args: Record<string, unknown>) => string | Response) {
  const seen: { tool: string; args: Record<string, unknown>; auth: string | null; session: string | null }[] = [];
  let initialized = 0;
  const fetchFn = (async (_url: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body));
    const headers = new Headers(init.headers);
    if (body.method === "initialize") {
      initialized++;
      return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, result: {} }), { headers: { "Mcp-Session-Id": "session-1" } });
    }
    seen.push({ tool: body.params.name, args: body.params.arguments, auth: headers.get("Authorization"), session: headers.get("Mcp-Session-Id") });
    const out = answer(body.params.name, body.params.arguments);
    if (out instanceof Response) return out;
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, result: { content: [{ type: "text", text: out }], isError: false } }));
  }) as unknown as typeof fetch;
  return { fetchFn, seen, initializations: () => initialized };
}

const connection = () => ({ host: "127.0.0.1", port: 8790, key: "secret" });

test("a button press is a tool call with the access key and the session", async () => {
  const roomtone = fakeRoomtone(() => "ok");
  const client = new RoomtoneClient(connection, roomtone.fetchFn);
  await client.playback("toggle");
  await client.changeVolume(-5, "Kitchen");
  assert.equal(roomtone.initializations(), 1, "it says hello once");
  assert.deepEqual(roomtone.seen.map((s) => [s.tool, s.args]), [
    ["playback", { action: "toggle" }],
    ["set_volume", { change: -5, whole_group: true, room: "Kitchen" }],
  ]);
  assert.ok(roomtone.seen.every((s) => s.auth === "Bearer secret" && s.session === "session-1"));
});

test("a blank room means the room selected in Roomtone", async () => {
  const roomtone = fakeRoomtone(() => "ok");
  const client = new RoomtoneClient(connection, roomtone.fetchFn);
  await client.setMute(true, "   ");
  await client.setMute(false, " Den ");
  assert.deepEqual(roomtone.seen.map((s) => s.args), [{ muted: true, whole_group: true }, { muted: false, whole_group: true, room: "Den" }]);
});

test("now playing is read from Roomtone's answer", async () => {
  const client = new RoomtoneClient(connection, fakeRoomtone(() => nowPlayingText).fetchFn);
  assert.deepEqual(await client.nowPlaying(), { state: "playing", title: "Slow Motion Summer", artist: "The Paper Lanterns", album: "Golden Hour Drive", volume: 30, muted: false, room: "Living Room" });
  assert.equal(parseNowPlaying("Nothing is selected."), undefined);
  assert.equal(parseNowPlaying("{}"), undefined);
  assert.equal(parseNowPlaying(JSON.stringify({ state: "paused", group_volume: 250, muted: true }))?.volume, 100, "volume is kept between 0 and 100");
});

test("what went wrong is said in words a person can act on", async () => {
  const status = (code: number) => new Response("{}", { status: code });
  const kinds: [Response | "network", string][] = [[status(401), "key"], [status(503), "off"], [status(403), "refused"], ["network", "unreachable"]];
  for (const [what, kind] of kinds) {
    const fetchFn = (async () => {
      if (what === "network") throw new TypeError("fetch failed");
      return what.clone();
    }) as unknown as typeof fetch;
    await assert.rejects(new RoomtoneClient(connection, fetchFn).playback("play"), (e: unknown) => e instanceof RoomtoneError && e.kind === kind && e.message.length > 10, kind);
  }
});

test("a blank access key is said so without asking Roomtone", async () => {
  let asked = 0;
  const fetchFn = (async () => (asked++, new Response("{}"))) as unknown as typeof fetch;
  await assert.rejects(new RoomtoneClient(() => ({ host: "127.0.0.1", port: 8790, key: "  " }), fetchFn).playback("play"), (e: unknown) => e instanceof RoomtoneError && e.kind === "nokey");
  assert.equal(asked, 0);
});

test("a tool that fails says why", async () => {
  const fetchFn = (async (_u: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body));
    if (body.method === "initialize") return new Response(JSON.stringify({ result: {} }));
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, result: { content: [{ type: "text", text: "No scene called Party." }], isError: true } }));
  }) as unknown as typeof fetch;
  await assert.rejects(new RoomtoneClient(connection, fetchFn).applyScene("Party"), /No scene called Party/);
});

test("a session Roomtone has forgotten is started again once", async () => {
  let calls = 0;
  let hellos = 0;
  const fetchFn = (async (_u: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body));
    if (body.method === "initialize") {
      hellos++;
      return new Response(JSON.stringify({ result: {} }), { headers: { "Mcp-Session-Id": `s${hellos}` } });
    }
    calls++;
    if (calls === 1) return new Response("{}", { status: 400 });
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, result: { content: [{ type: "text", text: "done" }] } }));
  }) as unknown as typeof fetch;
  assert.equal(await new RoomtoneClient(connection, fetchFn).pauseAll(), "done");
  assert.equal(hellos, 2);
});

test("scene names are the start of each line", () => {
  assert.deepEqual(parseSceneNames("Movie night: Living Room at 20; plays Jazz\nGood night: Bedroom"), ["Movie night", "Good night"]);
  assert.deepEqual(parseSceneNames("No scenes saved yet. Make one in the app's Scenes page."), []);
  assert.deepEqual(parseSceneNames(""), []);
});

test("room names are every room of every system, once, in order", async () => {
  const status = JSON.stringify({ status: "ok", systems: [
    { groups: [{ rooms: ["Living Room", "Kitchen"] }, { rooms: ["Den"] }] },
    { groups: [{ rooms: ["Office", "Kitchen"] }, { rooms: ["  "] }, {}] },
    {},
  ] });
  assert.deepEqual(parseRoomNames(status), ["Den", "Kitchen", "Living Room", "Office"]);
  assert.deepEqual(parseRoomNames("not json"), []);
  assert.deepEqual(parseRoomNames("{}"), []);
  assert.deepEqual(await new RoomtoneClient(connection, fakeRoomtone(() => status).fetchFn).listRooms(), ["Den", "Kitchen", "Living Room", "Office"]);
});

/** A Roomtone that answers `get_cover_art` with what it is given. */
function coverRoomtone(answer: Record<string, unknown> | { error: { code: number; message: string } }) {
  const asked: Record<string, unknown>[] = [];
  const fetchFn = (async (_u: string, init: RequestInit) => {
    const body = JSON.parse(String(init.body));
    if (body.method === "initialize") return new Response(JSON.stringify({ result: {} }), { headers: { "Mcp-Session-Id": "s" } });
    asked.push(body.params);
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, ...answer }));
  }) as unknown as typeof fetch;
  return { fetchFn, asked };
}

test("a cover comes back as an address a key can show", async () => {
  const roomtone = coverRoomtone({ result: { content: [{ type: "image", data: "/9j/AAA=", mimeType: "image/jpeg" }, { type: "text", text: "Cover of Song in Kitchen" }], isError: false } });
  const client = new RoomtoneClient(connection, roomtone.fetchFn);
  assert.equal(await client.coverArt("Kitchen", 144), "data:image/jpeg;base64,/9j/AAA=");
  assert.deepEqual(roomtone.asked[0], { name: "get_cover_art", arguments: { size: 144, room: "Kitchen" } });
});

test("no cover, or a Roomtone without the tool, is no cover, not an error", async () => {
  const none = coverRoomtone({ result: { content: [{ type: "text", text: "Kitchen has no cover picture right now." }], isError: true } });
  assert.equal(await new RoomtoneClient(connection, none.fetchFn).coverArt(), undefined);
  const old = coverRoomtone({ error: { code: -32602, message: "Unknown tool: get_cover_art" } });
  assert.equal(await new RoomtoneClient(connection, old.fetchFn).coverArt(), undefined);
  const textOnly = coverRoomtone({ result: { content: [{ type: "text", text: "fine" }], isError: false } });
  assert.equal(await new RoomtoneClient(connection, textOnly.fetchFn).coverArt(), undefined);
});

test("a wrong key is still a problem when asking for a cover", async () => {
  const fetchFn = (async () => new Response("{}", { status: 401 })) as unknown as typeof fetch;
  await assert.rejects(new RoomtoneClient(connection, fetchFn).coverArt(), (e: unknown) => e instanceof RoomtoneError && e.kind === "key");
});
