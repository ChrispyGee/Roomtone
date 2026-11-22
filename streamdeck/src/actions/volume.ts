import { action, type DialDownEvent, type DialRotateEvent, type KeyDownEvent, type KeyUpEvent, type TouchTapEvent, type Action } from "@elgato/streamdeck";

import { volumeLabel, volumeStep } from "../format.ts";
import type { NowPlaying } from "../roomtone.ts";
import { RoomtoneAction, type RoomSettings } from "./base.ts";

const ID = "io.github.chrispygee.roomtone";

interface StepSettings extends RoomSettings {
  step?: number | string;
}

/** Hold a volume key and it keeps going: after a short wait, a step every 150 ms. */
const HOLD_DELAY_MS = 500;
const HOLD_EVERY_MS = 150;

abstract class VolumeKey extends RoomtoneAction<StepSettings> {
  private readonly holds = new Map<string, { delay: ReturnType<typeof setTimeout>; repeat?: ReturnType<typeof setInterval> }>();

  protected abstract direction(): 1 | -1;

  protected override follows(): boolean {
    return true;
  }

  protected override async draw(a: Action<StepSettings>, now: NowPlaying | undefined): Promise<void> {
    if (a.isKey()) await a.setTitle(now ? volumeLabel(now.volume, now.muted) : "");
  }

  override async onKeyDown(ev: KeyDownEvent<StepSettings>): Promise<void> {
    const step = volumeStep(ev.payload.settings.step) * this.direction();
    const room = ev.payload.settings.room;
    const press = () => this.act(ev.action, () => this.client.changeVolume(step, room));
    await press();
    const hold = { delay: setTimeout(() => {
      hold.repeat = setInterval(() => void press(), HOLD_EVERY_MS);
    }, HOLD_DELAY_MS) } as { delay: ReturnType<typeof setTimeout>; repeat?: ReturnType<typeof setInterval> };
    this.holds.set(ev.action.id, hold);
  }

  override onKeyUp(ev: KeyUpEvent<StepSettings>): void {
    this.release(ev.action.id);
  }

  override onWillDisappear(ev: Parameters<RoomtoneAction<StepSettings>["onWillDisappear"]>[0]): void {
    this.release(ev.action.id);
    super.onWillDisappear(ev);
  }

  private release(id: string): void {
    const hold = this.holds.get(id);
    if (!hold) return;
    clearTimeout(hold.delay);
    if (hold.repeat) clearInterval(hold.repeat);
    this.holds.delete(id);
  }
}

@action({ UUID: `${ID}.volumeup` })
export class VolumeUp extends VolumeKey {
  protected override direction(): 1 | -1 {
    return 1;
  }
}

@action({ UUID: `${ID}.volumedown` })
export class VolumeDown extends VolumeKey {
  protected override direction(): 1 | -1 {
    return -1;
  }
}

/** Mute or unmute; shows a crossed-out speaker while muted. */
@action({ UUID: `${ID}.mute` })
export class Mute extends RoomtoneAction {
  // Each key may follow a different room, so each remembers its own.
  private readonly seen = new Map<string, NowPlaying | undefined>();

  protected override forgetState(id: string): void {
    this.seen.delete(id);
  }

  protected override follows(): boolean {
    return true;
  }

  protected override async draw(a: Action<RoomSettings>, now: NowPlaying | undefined): Promise<void> {
    this.seen.set(a.id, now);
    if (a.isKey()) {
      await a.setState(now?.muted ? 1 : 0);
    }
  }

  override async onKeyDown(ev: KeyDownEvent<RoomSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.setMute(!(this.seen.get(ev.action.id)?.muted ?? false), ev.payload.settings.room));
  }
}

/** The dial (Stream Deck +): turn for volume, press to mute, tap the screen to play or pause. */
@action({ UUID: `${ID}.volumedial` })
export class VolumeDial extends RoomtoneAction<StepSettings> {
  private readonly seen = new Map<string, NowPlaying | undefined>();

  protected override forgetState(id: string): void {
    this.seen.delete(id);
  }

  protected override follows(): boolean {
    return true;
  }

  protected override async draw(a: Action<StepSettings>, now: NowPlaying | undefined, settings: StepSettings): Promise<void> {
    this.seen.set(a.id, now);
    if (!a.isDial()) return;
    await a.setFeedback({
      title: settings.room?.trim() || now?.room || "Volume",
      value: now ? volumeLabel(now.volume, now.muted) : "–",
      indicator: now ? now.volume : 0,
    });
  }

  protected override async problem(a: Action<StepSettings>, problem: Parameters<RoomtoneAction["problem"]>[1]): Promise<void> {
    if (a.isDial()) await a.setFeedback({ title: "Roomtone", value: problem.kind === "key" || problem.kind === "nokey" ? "Key?" : "Off", indicator: 0 });
  }

  override async onDialRotate(ev: DialRotateEvent<StepSettings>): Promise<void> {
    const change = ev.payload.ticks * volumeStep(ev.payload.settings.step);
    await this.act(ev.action, () => this.client.changeVolume(change, ev.payload.settings.room));
  }

  override async onDialDown(ev: DialDownEvent<StepSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.setMute(!(this.seen.get(ev.action.id)?.muted ?? false), ev.payload.settings.room));
  }

  override async onTouchTap(ev: TouchTapEvent<StepSettings>): Promise<void> {
    await this.act(ev.action, () => this.client.playback("toggle", ev.payload.settings.room));
  }
}

