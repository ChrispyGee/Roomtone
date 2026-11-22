import { action, type KeyDownEvent, type Action } from "@elgato/streamdeck";

import { client } from "../context.ts";
import { CoverCache, trackKey } from "../cover.ts";
import { wrapTitle } from "../format.ts";
import type { NowPlaying } from "../roomtone.ts";
import { RoomtoneAction, type RoomSettings } from "./base.ts";

const ID = "io.github.chrispygee.roomtone";

/** Play or pause; shows a pause symbol while music plays. */
@action({ UUID: `${ID}.playpause` })
export class PlayPause extends RoomtoneAction {
  protected override follows(): boolean {
    return true;
  }
  protected override async draw(a: Action<RoomSettings>, now: NowPlaying | undefined): Promise<void> {
    if (a.isKey()) {
      await a.setState(now?.state === "playing" ? 1 : 0);
    }
  }
  override async onKeyDown(ev: KeyDownEvent<RoomSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.playback("toggle", ev.payload.settings.room));
  }
}

@action({ UUID: `${ID}.next` })
export class Next extends RoomtoneAction {
  override async onKeyDown(ev: KeyDownEvent<RoomSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.playback("next", ev.payload.settings.room));
  }
}

@action({ UUID: `${ID}.previous` })
export class Previous extends RoomtoneAction {
  override async onKeyDown(ev: KeyDownEvent<RoomSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.playback("previous", ev.payload.settings.room));
  }
}

/** Pause every room. */
@action({ UUID: `${ID}.pauseall` })
export class PauseAll extends RoomtoneAction {
  override async onKeyDown(ev: KeyDownEvent<RoomSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.pauseAll(ev.payload.settings.room));
  }
}

interface NowPlayingSettings extends RoomSettings {
  titleOnCover?: boolean;
}

/** Shows the cover of the song that is playing (or its title, if there is no cover); pressing it plays or pauses. */
@action({ UUID: `${ID}.nowplaying` })
export class NowPlayingKey extends RoomtoneAction<NowPlayingSettings> {
  private readonly covers = new CoverCache();

  protected override follows(): boolean {
    return true;
  }

  protected override forgetState(id: string): void {
    this.covers.forget(id);
  }

  protected override async draw(a: Action<NowPlayingSettings>, now: NowPlaying | undefined, settings: NowPlayingSettings): Promise<void> {
    if (!a.isKey()) return;
    await a.setState(now?.state === "playing" ? 1 : 0);
    const title = now && now.title ? wrapTitle(now.title, 9, 3) : "Nothing\nplaying";
    const cover = now && now.title ? await this.covers.get(a.id, trackKey(now), () => client.coverArt(settings.room)) : undefined;
    if (cover) {
      await a.setImage(cover);
      await a.setTitle(settings.titleOnCover === true ? title : "");
    } else {
      await a.setImage();
      await a.setTitle(title);
    }
  }

  override async onKeyDown(ev: KeyDownEvent<NowPlayingSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.playback("toggle", ev.payload.settings.room));
  }
}

interface SceneSettings extends RoomSettings {
  scene?: string;
}

/** Apply one of Roomtone's scenes (named in the button's settings). */
@action({ UUID: `${ID}.scene` })
export class Scene extends RoomtoneAction<SceneSettings> {
  protected override follows(): boolean {
    return true;
  }
  protected override async draw(a: Action<SceneSettings>, _now: NowPlaying | undefined, settings: SceneSettings): Promise<void> {
    if (a.isKey()) await a.setTitle(wrapTitle(settings.scene?.trim() || "Pick a\nscene", 9, 3));
  }
  override async onKeyDown(ev: KeyDownEvent<SceneSettings>): Promise<void> {
    const scene = ev.payload.settings.scene?.trim();
    if (!scene) {
      await ev.action.showAlert();
      return;
    }
    if (await this.act(ev.action, () => this.client.applyScene(scene))) await ev.action.showOk();
  }
}
