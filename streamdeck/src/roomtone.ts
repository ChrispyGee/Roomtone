// A small client for Roomtone's assistant link (MCP over HTTP). The plugin only ever sends tool calls
// that a person could make with a button: play, pause, next, volume, mute and "apply scene".

export interface Connection {
  host: string;
  port: number;
  key: string;
}

export const DEFAULT_CONNECTION: Connection = { host: "127.0.0.1", port: 8790, key: "" };

/** What `now_playing` tells us, cut down to what the buttons show. */
export interface NowPlaying {
  state: "playing" | "paused" | "stopped" | string;
  title: string;
  artist: string;
  album: string;
  volume: number;
  muted: boolean;
  room: string;
}

/** Something a person can read and act on: "Roomtone isn't running", "Wrong access key". */
export type ProblemKind = "unreachable" | "off" | "key" | "refused" | "failed";

export class RoomtoneError extends Error {
  readonly kind: ProblemKind;
  constructor(message: string, kind: ProblemKind) {
    super(message);
    this.kind = kind;
  }
}

type Fetch = typeof fetch;

export class RoomtoneClient {
  private session: string | undefined;
  private nextId = 1;
  private readonly connection: () => Connection;
  private readonly fetchFn: Fetch;

  constructor(connection: () => Connection, fetchFn: Fetch = fetch) {
    this.connection = connection;
    this.fetchFn = fetchFn;
  }

  private url(): string {
    const c = this.connection();
    return `http://${c.host || "127.0.0.1"}:${c.port || 8790}/mcp`;
  }

  private async post(body: unknown, withSession: boolean): Promise<Response> {
    const c = this.connection();
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      Authorization: `Bearer ${c.key}`,
    };
    if (withSession && this.session) headers["Mcp-Session-Id"] = this.session;
    let response: Response;
    try {
      response = await this.fetchFn(this.url(), { method: "POST", headers, body: JSON.stringify(body), signal: AbortSignal.timeout(8000) });
    } catch {
      throw new RoomtoneError("Roomtone isn't answering. Is it running, with its assistant link switched on?", "unreachable");
    }
    if (response.status === 401) throw new RoomtoneError("Roomtone refused the access key.", "key");
    if (response.status === 503) throw new RoomtoneError("Roomtone's assistant link is switched off.", "off");
    if (response.status === 403) throw new RoomtoneError("Roomtone refused this computer. Turn on Allow other computers, or use 127.0.0.1.", "refused");
    if (!response.ok && response.status !== 202) throw new RoomtoneError(`Roomtone answered ${response.status}.`, "failed");
    return response;
  }

  private async initialize(): Promise<void> {
    const response = await this.post(
      { jsonrpc: "2.0", id: this.nextId++, method: "initialize", params: { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "roomtone-streamdeck", version: "0.1.0" } } },
      false,
    );
    this.session = response.headers.get("Mcp-Session-Id") ?? undefined;
    await response.json().catch(() => undefined);
  }

  /** Call a Roomtone tool and return the text it answered with. */
  async call(tool: string, args: Record<string, unknown> = {}): Promise<string> {
    if (this.session === undefined) await this.initialize();
    const request = { jsonrpc: "2.0", id: this.nextId++, method: "tools/call", params: { name: tool, arguments: args } };
    let response: Response;
    try {
      response = await this.post(request, true);
    } catch (e) {
      // A session that Roomtone has forgotten (it restarted): start again once.
      if (e instanceof RoomtoneError && e.kind === "failed") {
        this.session = undefined;
        await this.initialize();
        response = await this.post(request, true);
      } else {
        throw e;
      }
    }
    const reply = (await response.json()) as { result?: { content?: { text?: string }[]; isError?: boolean }; error?: { message?: string } };
    if (reply.error) throw new RoomtoneError(reply.error.message ?? "Roomtone couldn't do that.", "failed");
    const text = (reply.result?.content ?? []).map((c) => c.text ?? "").join("\n");
    if (reply.result?.isError) throw new RoomtoneError(text || "Roomtone couldn't do that.", "failed");
    return text;
  }

  withRoom(args: Record<string, unknown>, room: string | undefined): Record<string, unknown> {
    return room && room.trim() ? { ...args, room: room.trim() } : args;
  }

  playback(action: "play" | "pause" | "toggle" | "next" | "previous", room?: string): Promise<string> {
    return this.call("playback", this.withRoom({ action }, room));
  }

  changeVolume(change: number, room?: string): Promise<string> {
    return this.call("set_volume", this.withRoom({ change: Math.round(change), whole_group: true }, room));
  }

  setMute(muted: boolean, room?: string): Promise<string> {
    return this.call("set_mute", this.withRoom({ muted, whole_group: true }, room));
  }

  applyScene(name: string): Promise<string> {
    return this.call("apply_scene", { name });
  }

  pauseAll(room?: string): Promise<string> {
    return this.call("pause_all", this.withRoom({}, room));
  }

  /** The names of the scenes Roomtone has (empty if none, or if it answers in a way we don't know). */
  async listScenes(): Promise<string[]> {
    return parseSceneNames(await this.call("list_scenes"));
  }

  /** The names of the rooms Roomtone knows, in alphabetical order. */
  async listRooms(): Promise<string[]> {
    return parseRoomNames(await this.call("get_status"));
  }

  /** Is Roomtone there, with the right key and the link on? Throws a `RoomtoneError` that says what's wrong. */
  async check(): Promise<void> {
    await this.call("get_status");
  }

  async nowPlaying(room?: string): Promise<NowPlaying | undefined> {
    return parseNowPlaying(await this.call("now_playing", this.withRoom({}, room)));
  }
}

/** Read `now_playing`'s reply. Anything that isn't a playing-state report gives `undefined`. */
export function parseNowPlaying(text: string): NowPlaying | undefined {
  let data: Record<string, unknown>;
  try {
    data = JSON.parse(text) as Record<string, unknown>;
  } catch {
    return undefined;
  }
  if (typeof data !== "object" || data === null || typeof data.state !== "string") return undefined;
  const str = (v: unknown) => (typeof v === "string" ? v : "");
  const volume = typeof data.group_volume === "number" ? data.group_volume : 0;
  return {
    state: data.state,
    title: str(data.title),
    artist: str(data.artist),
    album: str(data.album),
    volume: Math.max(0, Math.min(100, Math.round(volume))),
    muted: data.muted === true,
    room: str(data.lead_room),
  };
}

/** `list_scenes` answers one scene a line: "Movie night: Living Room at 20; plays Jazz". Take the names. */
export function parseSceneNames(text: string): string[] {
  if (/^no scenes/i.test(text.trim())) return [];
  return text
    .split("\n")
    .map((line) => line.split(":")[0].trim())
    .filter((name) => name.length > 0);
}

/** The room names in `get_status`'s answer (every room is in some group), without repeats, sorted. */
export function parseRoomNames(text: string): string[] {
  let data: { systems?: { groups?: { rooms?: unknown[] }[] }[] };
  try {
    data = JSON.parse(text);
  } catch {
    return [];
  }
  const names = new Set<string>();
  for (const system of data.systems ?? []) {
    for (const group of system.groups ?? []) {
      for (const room of group.rooms ?? []) if (typeof room === "string" && room.trim()) names.add(room.trim());
    }
  }
  return [...names].sort((a, b) => a.localeCompare(b));
}
