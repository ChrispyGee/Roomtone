// Text for the small screens: keys show about 8 letters across and 3 lines, the dial strip a little more.

/** Break `text` into at most `lines` lines of at most `width` letters, on spaces where it can. */
export function wrapTitle(text: string, width = 8, lines = 3): string {
  const words = text.trim().split(/\s+/).filter(Boolean);
  const out: string[] = [];
  let line = "";
  for (let word of words) {
    while (word.length > width) {
      if (line) {
        out.push(line);
        line = "";
      }
      out.push(word.slice(0, width));
      word = word.slice(width);
    }
    if (!line) line = word;
    else if (line.length + 1 + word.length <= width) line += ` ${word}`;
    else {
      out.push(line);
      line = word;
    }
  }
  if (line) out.push(line);
  if (out.length > lines) {
    const kept = out.slice(0, lines);
    const last = kept[lines - 1];
    kept[lines - 1] = (last.length >= width ? last.slice(0, width - 1) : last) + "…";
    return kept.join("\n");
  }
  return out.join("\n");
}

/** A short word for what went wrong, to show on a key. */
export function problemLabel(kind: "unreachable" | "off" | "key" | "refused" | "failed"): string {
  switch (kind) {
    case "unreachable":
      return "Roomtone\nnot\nrunning";
    case "off":
      return "Link\nis off";
    case "key":
      return "Wrong\nkey";
    case "refused":
      return "Not\nallowed";
    default:
      return "Error";
  }
}

/** The volume to show on a key or the dial. */
export function volumeLabel(volume: number, muted: boolean): string {
  return muted ? "Muted" : String(Math.max(0, Math.min(100, Math.round(volume))));
}

/** The step a volume key or dial moves by: a whole number from 1 to 25, 5 if the setting is missing or odd. */
export function volumeStep(setting: unknown): number {
  const n = typeof setting === "string" ? Number(setting) : setting;
  return typeof n === "number" && Number.isFinite(n) ? Math.max(1, Math.min(25, Math.round(n))) : 5;
}
