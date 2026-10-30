import assert from "node:assert/strict";
import { test } from "node:test";
import { formatCube, parseCube, type Cube } from "../src/cube.ts";
import { parseHald } from "../src/hald.ts";

type Rgb = [number, number, number];
type Look = (r: number, g: number, b: number) => Rgb;

const ONE_CODE = 1 / 255;
const identity: Look = (r, g, b) => [r, g, b];
/** Not symmetric in any two channels, as a look isn't: a mix-up of channels shows. */
const lopsided: Look = (r, g, b) => [r, g * 0.5, 1 - b];
/** Something like a film look: a toe and shoulder, warm shadows, a little crosstalk. */
const filmic: Look = (r, g, b) => {
  const curve = (x: number) => x * x * (3 - 2 * x);
  const luma = 0.3 * r + 0.6 * g + 0.1 * b;
  const mix = (x: number) => curve(x) * 0.85 + luma * 0.15;
  return [Math.min(1, mix(r) + 0.04), mix(g), Math.max(0, mix(b) - 0.03)];
};

/** A HaldCLUT as a decoder hands it over: RGBA bytes in file order, red changing fastest. */
function hald(size: number, look: Look) {
  const width = Math.round(Math.sqrt(size ** 3));
  const data = new Uint8ClampedArray(width * width * 4);
  let at = 0;
  for (let b = 0; b < size; b++) {
    for (let g = 0; g < size; g++) {
      for (let r = 0; r < size; r++) {
        const [red, green, blue] = look(r / (size - 1), g / (size - 1), b / (size - 1));
        data[at++] = red * 255;
        data[at++] = green * 255;
        data[at++] = blue * 255;
        data[at++] = 255;
      }
    }
  }
  return { width, height: width, data };
}

/** The same look as a .cube file, written the way LUT tools write it. */
function cubeFile(size: number, look: Look) {
  const rows = [`LUT_3D_SIZE ${size}`];
  for (let b = 0; b < size; b++) {
    for (let g = 0; g < size; g++) {
      for (let r = 0; r < size; r++) {
        rows.push(look(r / (size - 1), g / (size - 1), b / (size - 1)).map((v) => v.toFixed(6)).join(" "));
      }
    }
  }
  return rows.join("\n");
}

const row = (cube: Cube, index: number) => Array.from(cube.values.slice(index * 3, index * 3 + 3));

function assertClose(actual: ArrayLike<number>, expected: ArrayLike<number>, within: number) {
  assert.equal(actual.length, expected.length);
  for (let i = 0; i < actual.length; i++) {
    assert.ok(Math.abs(actual[i] - expected[i]) <= within, `value ${i}: ${actual[i]} is not ${expected[i]}`);
  }
}

/** The look at a colour between the nodes, as the screen's 3D texture blends it. */
function sample(cube: Cube, color: Rgb): Rgb {
  const n = cube.size;
  const node = (r: number, g: number, b: number, c: number) => cube.values[(r + g * n + b * n * n) * 3 + c];
  const [fr, fg, fb] = color.map((v) => v * (n - 1));
  const [r0, g0, b0] = [fr, fg, fb].map((v) => Math.min(Math.floor(v), n - 2));
  const [tr, tg, tb] = [fr - r0, fg - g0, fb - b0];
  const channel = (c: number) => {
    const along = (g: number, b: number) => node(r0, g, b, c) * (1 - tr) + node(r0 + 1, g, b, c) * tr;
    const front = along(g0, b0) * (1 - tg) + along(g0 + 1, b0) * tg;
    const back = along(g0, b0 + 1) * (1 - tg) + along(g0 + 1, b0 + 1) * tg;
    return front * (1 - tb) + back * tb;
  };
  return [channel(0), channel(1), channel(2)];
}

test("an 8x8 Hald is a LUT of size 4 over the full domain", () => {
  const cube = parseHald(hald(4, identity), "Portra.png");
  assert.equal(cube.size, 4);
  assert.equal(cube.values.length, 64 * 3);
  assert.deepEqual(cube.domainMin, [0, 0, 0]);
  assert.deepEqual(cube.domainMax, [1, 1, 1]);
});

test("the look is named after the file", () => {
  assert.equal(parseHald(hald(4, identity), "Portra.png").name, "Portra");
  assert.equal(parseHald(hald(4, identity), "Portra.PNG").name, "Portra");
});

test("an identity Hald gives an identity LUT", () => {
  const cube = parseHald(hald(64, identity), "identity.png");
  assertClose(cube.values, parseCube(cubeFile(64, identity), "identity.cube").values, ONE_CODE / 2 + 1e-6);
});

test("pixels fill the LUT with red changing fastest, then green, then blue", () => {
  const cube = parseHald(hald(4, identity), "identity.png");
  assert.deepEqual(row(cube, 1), [85 / 255, 0, 0].map(Math.fround));
  assert.deepEqual(row(cube, 4), [0, 85 / 255, 0].map(Math.fround));
  assert.deepEqual(row(cube, 16), [0, 0, 85 / 255].map(Math.fround));
});

test("a Hald lines up with a .cube of the same look, row for row", () => {
  const fromPng = parseHald(hald(4, lopsided), "look.png");
  const fromCube = parseCube(cubeFile(4, lopsided), "look.cube");
  assertClose(fromPng.values, fromCube.values, ONE_CODE / 2 + 1e-6);
});

test("the alpha byte of each pixel is skipped", () => {
  const image = hald(4, () => [0.2, 0.4, 0.6]);
  const cube = parseHald(image, "flat.png");
  assert.deepEqual(row(cube, 63), [51 / 255, 102 / 255, 153 / 255].map(Math.fround));
});

test("an image that is not square is refused, saying its size", () => {
  const image = { width: 640, height: 480, data: new Uint8ClampedArray(640 * 480 * 4) };
  assert.throws(() => parseHald(image, "photo.png"), /640x480.*square/);
});

test("a square image of no Hald size is refused, saying its size", () => {
  const image = { width: 900, height: 900, data: new Uint8ClampedArray(900 * 900 * 4) };
  assert.throws(() => parseHald(image, "photo.png"), /900x900/);
});

test("a single pixel is refused: a LUT needs two nodes on each side", () => {
  const image = { width: 1, height: 1, data: new Uint8ClampedArray(4) };
  assert.throws(() => parseHald(image, "dot.png"), /1x1/);
});

test("a level 12 Hald is kept at 65 nodes, as the common large .cube", () => {
  const cube = parseHald(hald(144, identity), "film.png");
  assert.equal(cube.size, 65);
  assertClose(cube.values, parseCube(cubeFile(65, identity), "identity.cube").values, ONE_CODE / 2 + 1e-6);
});

test("reducing a level 12 Hald keeps its corners exact", () => {
  const image = hald(144, lopsided);
  const cube = parseHald(image, "film.png");
  assert.deepEqual(row(cube, 0), [0, 0, 255 / 255].map(Math.fround));
  assert.deepEqual(row(cube, 64), [1, 0, 1].map(Math.fround));
  assert.deepEqual(row(cube, 64 * 65 * 65 + 64 * 65 + 64), [1, 128 / 255, 0].map(Math.fround));
});

test("a look gives the same output as a .cube and as a Hald, level 8", () => {
  const asCube = parseCube(cubeFile(64, filmic), "film.cube");
  const asHald = parseCube(formatCube(parseHald(hald(64, filmic), "film.png")), "film.cube");
  assert.equal(asHald.size, 64);
  assertClose(asHald.values, asCube.values, ONE_CODE / 2 + 0.00006);
});

test("a look gives the same output as a .cube and as a Hald, level 12", () => {
  const asCube = parseCube(cubeFile(129, filmic), "film.cube");
  const asHald = parseCube(formatCube(parseHald(hald(144, filmic), "film.png")), "film.cube");
  for (let i = 0; i < 2000; i++) {
    const color: Rgb = [((i * 37) % 251) / 250, ((i * 61) % 241) / 240, ((i * 89) % 239) / 238];
    assertClose(sample(asHald, color), sample(asCube, color), ONE_CODE);
  }
});

test("a written LUT reads back as the same table", () => {
  const cube = parseHald(hald(16, filmic), "film.png");
  const back = parseCube(formatCube(cube), "film.cube");
  assert.equal(back.size, 16);
  assertClose(back.values, cube.values, 0.00005 + 1e-6);
});

test("a written LUT keeps its domain", () => {
  const cube = parseCube(`DOMAIN_MIN 0 0.5 0\nDOMAIN_MAX 2 1 1\n${cubeFile(2, identity)}`, "wide.cube");
  const back = parseCube(formatCube(cube), "wide.cube");
  assert.deepEqual(back.domainMin, [0, 0.5, 0]);
  assert.deepEqual(back.domainMax, [2, 1, 1]);
});
