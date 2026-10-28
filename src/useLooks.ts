// Looks: the .cube files in the app's looks folder, the chosen one (parsed for
// the frame loop) and adding new ones.
import { useEffect, useRef, useState } from "react";
import * as camera from "./camera";
import { parseCube, type Cube } from "./cube";
import { remember, remembered } from "./storage";

const LOOK_KEY = "groundglass.look";

export function useLooks() {
  const [names, setNames] = useState<string[]>([]);
  const [chosen, choose] = useState(() => remembered(LOOK_KEY) ?? "");
  const [error, setError] = useState("");
  // The frame loop reads this on every frame, so changing it needs no restart.
  const cube = useRef<Cube | null>(null);

  useEffect(() => {
    camera.listLuts().then(setNames, (e) => setError(String(e)));
  }, []);

  useEffect(() => {
    remember(LOOK_KEY, chosen);
    cube.current = null;
    if (!chosen) return;
    let current = true;
    camera
      .readLut(chosen)
      .then((text) => {
        if (current) cube.current = parseCube(text, chosen);
        setError("");
      })
      .catch((e) => setError(`${chosen}: ${e}`));
    return () => {
      current = false;
    };
  }, [chosen]);

  /** Checks each file parses, then keeps a copy in the app's looks folder. */
  async function add(files: FileList | null) {
    const failed: string[] = [];
    for (const file of Array.from(files ?? [])) {
      try {
        const text = await file.text();
        parseCube(text, file.name);
        await camera.saveLut(file.name, text);
      } catch (e) {
        failed.push(`${file.name}: ${e instanceof Error ? e.message : e}`);
      }
    }
    setError(failed.join("\n"));
    setNames(await camera.listLuts());
  }

  return { names, chosen, choose, error, cube, add };
}

export const lookLabel = (name: string) => name.replace(/\.cube$/i, "");
