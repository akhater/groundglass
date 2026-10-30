import assert from "node:assert/strict";
import { test } from "node:test";
import { lookFormat } from "../src/lookFormat.ts";

test("a .cube file is a cube, whatever the case of its name", () => {
  assert.equal(lookFormat("Portra 160.cube"), "cube");
  assert.equal(lookFormat("PORTRA.CUBE"), "cube");
});

test("a .png file is a png, whatever the case of its name", () => {
  assert.equal(lookFormat("Velvia.png"), "png");
  assert.equal(lookFormat("VELVIA.PNG"), "png");
});

test("any other file is refused, naming both accepted types", () => {
  for (const name of ["photo.jpg", "notes.pdf", "look.cube.txt", "cube", "png", ""]) {
    assert.throws(() => lookFormat(name), /\.cube.*\.png/, name);
  }
});
