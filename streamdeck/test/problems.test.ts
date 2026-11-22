import assert from "node:assert/strict";
import { test } from "node:test";

import { ProblemMemory } from "../src/problems.ts";

test("an error on a button is wiped once, when things work again", () => {
  const memory = new ProblemMemory();
  assert.equal(memory.clear("a"), false, "nothing to wipe on a button that never had a problem");
  memory.mark("a");
  memory.mark("b");
  assert.equal(memory.clear("a"), true);
  assert.equal(memory.clear("a"), false, "only once");
  memory.forget("b");
  assert.equal(memory.clear("b"), false, "a button that went away is forgotten");
});
