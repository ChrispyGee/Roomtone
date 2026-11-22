// Remembers which buttons are showing an error on their face, so the error can be wiped once things work.

export class ProblemMemory {
  private readonly shown = new Set<string>();

  /** A button now shows a problem. */
  mark(id: string): void {
    this.shown.add(id);
  }

  /** The button works again: true if it had a problem to wipe (so the caller resets its title). */
  clear(id: string): boolean {
    return this.shown.delete(id);
  }

  forget(id: string): void {
    this.shown.delete(id);
  }
}
