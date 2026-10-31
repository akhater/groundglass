import assert from "node:assert/strict";
import { test } from "node:test";
import { AFTER_STOP_MS, restLeft } from "../src/oneTapRest.ts";

test("with no Stop there is nothing to wait for", () => {
  assert.equal(restLeft(null, 5000), 0);
});

test("a tap right after Stop waits the rest of the time", () => {
  assert.equal(restLeft(1000, 1000), AFTER_STOP_MS);
  assert.equal(restLeft(1000, 6000), AFTER_STOP_MS - 5000);
});

test("once the time has passed a tap starts at once", () => {
  assert.equal(restLeft(1000, 1000 + AFTER_STOP_MS), 0);
  assert.equal(restLeft(1000, 1000 + AFTER_STOP_MS + 1), 0);
});
