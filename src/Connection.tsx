// The camera's address and Start / Stop. Before Start it sits in the middle of
// the picture; while live it opens from the status badge at the top.
import type { Status } from "./liveView";

type Props = {
  running: boolean;
  address: string;
  onAddress: (address: string) => void;
  cameraDisplayOn: boolean;
  onCameraDisplay: (on: boolean) => void;
  status: Status | null;
  onStart: () => void;
  onStop: () => void;
};

export function Connection(props: Props) {
  const screenToggle = (
    <label className="toggle">
      <input
        type="checkbox"
        checked={props.cameraDisplayOn}
        onChange={(e) => props.onCameraDisplay(e.target.checked)}
      />
      Keep the camera's screen on
    </label>
  );

  if (props.running) {
    return (
      <div className="panel sheet" role="dialog" aria-label="Connection">
        <p className="address">{props.address}</p>
        <p>{label(props.status)}</p>
        {screenToggle}
        <button className="secondary" onClick={props.onStop}>
          Stop live view
        </button>
      </div>
    );
  }

  return (
    <form
      className="panel connect"
      onSubmit={(e) => {
        e.preventDefault();
        props.onStart();
      }}
    >
      <h1>Groundglass</h1>
      <input
        value={props.address}
        onChange={(e) => props.onAddress(e.target.value)}
        placeholder="Camera address, e.g. 192.168.1.2:8080"
        aria-label="Camera address"
        inputMode="url"
      />
      <button type="submit" className="primary" disabled={!props.address.trim()}>
        Start
      </button>
      {screenToggle}
    </form>
  );
}

/** The status on the picture, kept short; tapping it opens the connection panel. */
export function StatusBadge({ status, open, onToggle }: { status: Status | null; open: boolean; onToggle: () => void }) {
  return (
    <button className={`status ${status?.state ?? "connecting"}`} aria-expanded={open} onClick={onToggle}>
      {status?.state === "live" ? `${status.fps} fps` : status?.state === "lost" ? "Retrying…" : "Connecting…"}
    </button>
  );
}

function label(status: Status | null): string {
  switch (status?.state) {
    case "live":
      return `Live · ${status.fps} fps`;
    case "lost":
      return `${status.message} Retrying…`;
    default:
      return "Connecting…";
  }
}
