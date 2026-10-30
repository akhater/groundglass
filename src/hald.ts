// Reads a HaldCLUT: a 3D LUT stored as a square picture, the form many film
// looks come in. Its pixels, in file order, are a .cube's rows (red changing
// fastest), so a look gives the same colours in either format.

import type { Cube } from "./cube";

/**
 * Larger tables are reduced to this. A level 12 Hald has 144 nodes: as a .cube
 * that is about 60 MB of text to store and parse on a phone, against 6 MB here.
 * Three film looks at that level, at 300,000 colours: 1.9 of 255 levels apart at most, 0.13 on average.
 */
const MOST_NODES = 65;

type Pixels = Pick<ImageData, "width" | "height" | "data">;

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

export function parseHald({ width, height, data }: Pixels, fileName: string): Cube {
  if (width !== height) throw new Error(`This PNG is ${width}x${height}, but a HaldCLUT is square.`);
  const nodes = Math.round(Math.cbrt(width * height));
  if (nodes < 2 || nodes ** 3 !== width * height) {
    throw new Error(`This PNG is ${width}x${height}, which is not a HaldCLUT size such as 512x512.`);
  }

  const size = Math.min(nodes, MOST_NODES);
  const step = (nodes - 1) / (size - 1);
  const axis = Array.from({ length: size }, (_, node) => {
    const low = Math.min(Math.floor(node * step), nodes - 2);
    return { low, t: node * step - low };
  });
  const read = (r: number, g: number, b: number, channel: number) => data[(r + g * nodes + b * nodes * nodes) * 4 + channel];

  const values = new Float32Array(size ** 3 * 3);
  let count = 0;
  for (const b of axis) {
    for (const g of axis) {
      for (const r of axis) {
        for (let channel = 0; channel < 3; channel++) {
          const edge = (gi: number, bi: number) => lerp(read(r.low, gi, bi, channel), read(r.low + 1, gi, bi, channel), r.t);
          const face = (bi: number) => lerp(edge(g.low, bi), edge(g.low + 1, bi), g.t);
          values[count++] = lerp(face(b.low), face(b.low + 1), b.t) / 255;
        }
      }
    }
  }

  return { name: fileName.replace(/\.png$/i, ""), size, domainMin: [0, 0, 0], domainMax: [1, 1, 1], values };
}

/** Decodes the file to its stored values: a colour profile must not shift them. */
export async function readHald(file: File): Promise<Cube> {
  const bitmap = await createImageBitmap(file, { colorSpaceConversion: "none", premultiplyAlpha: "none" });
  const context = new OffscreenCanvas(bitmap.width, bitmap.height).getContext("2d", { willReadFrequently: true })!;
  context.drawImage(bitmap, 0, 0);
  const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height);
  bitmap.close();
  return parseHald(pixels, file.name);
}
