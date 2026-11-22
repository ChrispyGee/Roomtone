// Asks Roomtone what is playing every couple of seconds, but only while some button that shows it
// is on screen, and only once for each room however many buttons share it.

import type { NowPlaying, RoomtoneClient } from "./roomtone.ts";
import { RoomtoneError } from "./roomtone.ts";

export type Listener = (now: NowPlaying | undefined, problem: RoomtoneError | undefined) => void;

interface Watched {
  room: string | undefined;
  listener: Listener;
}

export class Monitor {
  private readonly watchers = new Set<Watched>();
  private timer: ReturnType<typeof setInterval> | undefined;
  private busy = false;
  private readonly client: Pick<RoomtoneClient, "nowPlaying">;
  private readonly everyMs: number;

  constructor(client: Pick<RoomtoneClient, "nowPlaying">, everyMs = 2000) {
    this.client = client;
    this.everyMs = everyMs;
  }

  /** Start telling `listener` what is playing in `room` (blank: the room Roomtone has selected). Returns how to stop. */
  watch(room: string | undefined, listener: Listener): () => void {
    const entry: Watched = { room: room?.trim() || undefined, listener };
    this.watchers.add(entry);
    if (this.timer === undefined) this.timer = setInterval(() => void this.refresh(), this.everyMs);
    void this.refresh();
    return () => {
      this.watchers.delete(entry);
      if (this.watchers.size === 0 && this.timer !== undefined) {
        clearInterval(this.timer);
        this.timer = undefined;
      }
    };
  }

  /** Ask now (after a button press, say), without waiting for the next turn. */
  async refresh(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      const rooms = new Set([...this.watchers].map((w) => w.room ?? ""));
      for (const room of rooms) {
        let now: NowPlaying | undefined;
        let problem: RoomtoneError | undefined;
        try {
          now = await this.client.nowPlaying(room || undefined);
        } catch (e) {
          problem = e instanceof RoomtoneError ? e : new RoomtoneError(String(e), "failed");
        }
        for (const w of [...this.watchers]) {
          if ((w.room ?? "") === room) w.listener(now, problem);
        }
      }
    } finally {
      this.busy = false;
    }
  }

  get watching(): number {
    return this.watchers.size;
  }
}
