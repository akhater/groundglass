// The camera's address and Start / Stop. Before Start it sits in the middle of
// the picture; while live it opens from the status badge at the top.
import { useState } from "react";
import type { Status } from "./liveView";

type Props = {
  running: boolean;
  address: string;
  onAddress: (address: string) => void;
  /** Addresses used lately, newest first. */
  recent: string[];
  cameraDisplayOn: boolean;
  onCameraDisplay: (on: boolean) => void;
  status: Status | null;
  onStart: () => void;
  onStop: () => void;
};

export function Connection(props: Props) {
  const [listOpen, setListOpen] = useState(false);
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
      <div className="address-field">
        <input
          value={props.address}
          onChange={(e) => props.onAddress(e.target.value)}
          placeholder="Camera address, e.g. 192.168.1.2:8080"
          aria-label="Camera address"
          inputMode="url"
        />
        {props.recent.length > 0 && (
          <button
            type="button"
            className="secondary"
            aria-label="Recent cameras"
            aria-expanded={listOpen}
            onClick={() => setListOpen((open) => !open)}
          >
            ▾
          </button>
        )}
      </div>
      {listOpen && (
        <div className="recent" role="listbox" aria-label="Recent cameras">
          {props.recent.map((address) => (
            <button
              key={address}
              type="button"
              role="option"
              aria-selected={address === props.address}
              onClick={() => {
                props.onAddress(address);
                setListOpen(false);
              }}
            >
              {address}
            </button>
          ))}
        </div>
      )}
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
    case "connecting":
      return status.hint ? `Connecting… ${status.hint}` : "Connecting…";
    default:
      return "Connecting…";
  }
}
