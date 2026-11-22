// Covers for the Now Playing key: asked for once for each song, not every time the key is redrawn.

import type { NowPlaying } from "./roomtone.ts";

/** What makes one song different from the next, for deciding when to fetch a new cover. */
export function trackKey(now: NowPlaying): string {
  return [now.room, now.title, now.artist, now.album].join("\u001f");
}

type Fetcher = () => Promise<string | undefined>;

export class CoverCache {
  private readonly seen = new Map<string, { key: string; cover: string | undefined }>();

  /** The cover (a data address) for `id`'s song `key`, fetched only when the song is new. `undefined`: none. */
  async get(id: string, key: string, fetcher: Fetcher): Promise<string | undefined> {
    const known = this.seen.get(id);
    if (known && known.key === key) return known.cover;
    // Remember the key first, so a slow answer isn't asked for again by the next refresh.
    this.seen.set(id, { key, cover: undefined });
    let cover: string | undefined;
    try {
      cover = await fetcher();
    } catch {
      // The plugin's other calls will say what is wrong with the connection; a cover is optional.
      cover = undefined;
    }
    if (this.seen.get(id)?.key === key) this.seen.set(id, { key, cover });
    return cover;
  }

  forget(id: string): void {
    this.seen.delete(id);
  }
}
