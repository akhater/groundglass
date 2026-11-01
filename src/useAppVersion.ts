// The version of this build, for the screen to show. Without it, nothing shows.
import { useEffect, useState } from "react";
import * as camera from "./camera";

export function useAppVersion() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void camera.appVersion().then(setVersion, () => {});
  }, []);

  return version;
}
