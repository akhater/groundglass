// Reads a 3D .cube LUT. Adapted from open-eos-control (Apache-2.0,
// bridge/open_eos_bridge/static/lut.js at 089e71a3, parseCubeLut): same
// directives, shortened. 1D/shaper LUTs are refused.

export type Cube = {
  name: string;
  size: number;
  domainMin: [number, number, number];
  domainMax: [number, number, number];
  /** RGB rows, red changing fastest, as the file lists them. */
  values: Float32Array;
};

export function parseCube(text: string, fallbackName: string): Cube {
  let name = fallbackName.replace(/\.cube$/i, "");
  let size = 0;
  let domainMin: [number, number, number] = [0, 0, 0];
  let domainMax: [number, number, number] = [1, 1, 1];
  let values: Float32Array | null = null;
  let count = 0;

  text.split(/\r?\n/).forEach((source, index) => {
    const line = source.split("#", 1)[0].trim();
    if (!line) return;
    const tokens = line.split(/\s+/);
    const directive = tokens[0].toUpperCase();
    const at = ` (line ${index + 1})`;
    if (directive === "TITLE") {
      name = line.slice(5).trim().replace(/^"|"$/g, "") || name;
    } else if (directive === "LUT_3D_SIZE") {
      size = Number(tokens[1]);
      if (!Number.isInteger(size) || size < 2 || size > 129) throw new Error(`Bad LUT_3D_SIZE${at}`);
      values = new Float32Array(size * size * size * 3);
    } else if (directive === "DOMAIN_MIN" || directive === "DOMAIN_MAX") {
      const v = tokens.slice(1, 4).map(Number) as [number, number, number];
      if (v.length !== 3 || !v.every(Number.isFinite)) throw new Error(`Bad ${directive}${at}`);
      if (directive === "DOMAIN_MIN") domainMin = v;
      else domainMax = v;
    } else if (directive === "LUT_1D_SIZE") {
      throw new Error("1D LUTs are not supported");
    } else {
      if (!values) throw new Error(`Table data before LUT_3D_SIZE${at}`);
      if (tokens.length !== 3 || count >= values.length) throw new Error(`Bad LUT row${at}`);
      for (const token of tokens) {
        const n = Number(token);
        if (!Number.isFinite(n)) throw new Error(`Bad number${at}`);
        values[count++] = n;
      }
    }
  });

  if (!values || count !== (values as Float32Array).length) throw new Error("Incomplete LUT table");
  return { name, size, domainMin, domainMax, values };
}
