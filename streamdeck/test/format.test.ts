import assert from "node:assert/strict";
import { test } from "node:test";

import { problemLabel, volumeLabel, volumeStep, wrapTitle } from "../src/format.ts";

test("a song title is broken to fit a key", () => {
  assert.equal(wrapTitle("Slow Motion Summer", 9, 3), "Slow\nMotion\nSummer");
  assert.equal(wrapTitle("Hi", 9, 3), "Hi");
  assert.equal(wrapTitle("", 9, 3), "");
  assert.equal(wrapTitle("Supercalifragilistic", 8, 3), "Supercal\nifragili\nstic");
  assert.equal(wrapTitle("A very long song title that will not fit on a key at all", 8, 2).split("\n").length, 2);
  assert.ok(wrapTitle("A very long song title that will not fit on a key at all", 8, 2).endsWith("…"));
  for (const line of wrapTitle("Everything Everywhere All At Once Forever", 9, 3).split("\n")) assert.ok([...line].length <= 9, line);
});

test("the volume step is a sensible whole number", () => {
  assert.equal(volumeStep(undefined), 5);
  assert.equal(volumeStep("10"), 10);
  assert.equal(volumeStep(0), 1);
  assert.equal(volumeStep(99), 25);
  assert.equal(volumeStep("abc"), 5);
  assert.equal(volumeStep(2.6), 3);
});

test("labels", () => {
  assert.equal(volumeLabel(42.4, false), "42");
  assert.equal(volumeLabel(42, true), "Muted");
  assert.equal(volumeLabel(120, false), "100");
  assert.ok(problemLabel("key").length > 0 && problemLabel("unreachable").includes("Roomtone"));
});
