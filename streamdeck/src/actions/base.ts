// The parts that every Roomtone button does the same way.

import { SingletonAction, type WillAppearEvent, type WillDisappearEvent } from "@elgato/streamdeck";
import type { Action } from "@elgato/streamdeck";

import { client, monitor } from "../context.ts";
import { problemLabel } from "../format.ts";
import { ProblemMemory } from "../problems.ts";
import { RoomtoneError, type NowPlaying } from "../roomtone.ts";

/** Settings every button may have: which room to act on (blank: the room Roomtone has selected). */
export interface RoomSettings {
  room?: string;
  [key: string]: string | number | boolean | undefined;
}

export abstract class RoomtoneAction<S extends RoomSettings = RoomSettings> extends SingletonAction<S> {
  private readonly stops = new Map<string, () => void>();
  private readonly problems = new ProblemMemory();

  /** A button that wants to follow what is playing says what to do with it. */
  protected follows(): boolean {
    return false;
  }

  /** Draw the button from what is playing, or from what went wrong. */
  protected async draw(_action: Action<S>, _now: NowPlaying | undefined, _settings: S): Promise<void> {}

  protected async problem(action: Action<S>, problem: RoomtoneError): Promise<void> {
    this.problems.mark(action.id);
    if (action.isKey()) await action.setTitle(problemLabel(problem.kind));
  }

  /** Things work again: take the error off the button's face (back to its own title, or the one the person typed). */
  protected async recovered(action: Action<S>): Promise<void> {
    if (this.problems.clear(action.id) && action.isKey()) await action.setTitle();
  }

  override async onWillAppear(ev: WillAppearEvent<S>): Promise<void> {
    if (!this.follows()) return;
    const action = ev.action;
    this.stops.get(action.id)?.();
    this.stops.set(
      action.id,
      monitor.watch(ev.payload.settings.room, async (now, problem) => {
        const settings = await action.getSettings().catch(() => ev.payload.settings);
        if (problem) await this.problem(action, problem);
        else {
          await this.recovered(action);
          await this.draw(action, now, settings);
        }
      }),
    );
  }

  override onWillDisappear(ev: WillDisappearEvent<S>): void {
    this.stops.get(ev.action.id)?.();
    this.stops.delete(ev.action.id);
    this.problems.forget(ev.action.id);
  }

  /** Run a button's job; if Roomtone says no, show the alert on the button; look again shortly after. */
  protected async act(action: Action<S>, job: () => Promise<unknown>): Promise<boolean> {
    try {
      await job();
      await this.recovered(action);
      void monitor.refresh();
      return true;
    } catch (e) {
      if (action.isKey() || action.isDial()) await action.showAlert();
      if (e instanceof RoomtoneError) await this.problem(action, e);
      return false;
    }
  }

  protected get client() {
    return client;
  }
}
