// The parts that every Roomtone button does the same way.

import { SingletonAction, type DidReceiveSettingsEvent, type WillAppearEvent, type WillDisappearEvent } from "@elgato/streamdeck";
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

  /** A button that remembers what it last saw lets go of it here. */
  protected forgetState(_id: string): void {}

  /** Things work again: take the error off the button's face (back to its own title, or the one the person typed). */
  protected async recovered(action: Action<S>): Promise<void> {
    if (this.problems.clear(action.id) && action.isKey()) await action.setTitle();
  }

  /** (Re)start following the room in `settings`; the one before is let go. */
  private start(action: Action<S>, settings: S): void {
    if (!this.follows()) return;
    monitor.follow(action.id, settings.room, async (now, problem) => {
      const current = await action.getSettings().catch(() => settings);
      if (problem) await this.problem(action, problem);
      else {
        await this.recovered(action);
        await this.draw(action, now, current);
      }
    });
  }

  override async onWillAppear(ev: WillAppearEvent<S>): Promise<void> {
    this.start(ev.action, ev.payload.settings);
  }

  /** The person changed this button's settings (another room, say): follow that. */
  override onDidReceiveSettings(ev: DidReceiveSettingsEvent<S>): void {
    this.start(ev.action, ev.payload.settings);
  }

  override onWillDisappear(ev: WillDisappearEvent<S>): void {
    monitor.unfollow(ev.action.id);
    this.problems.forget(ev.action.id);
    this.forgetState(ev.action.id);
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
