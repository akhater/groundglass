// The camera's address and Start / Stop. Before Start it sits in the middle of
// the picture; while live it opens from the status badge at the top.
import { useState } from "react";
import type { Status } from "./liveView";
import { canConnectOverBluetooth, openReleasePage, type Battery } from "./camera";
import { Icon } from "./icons";
import type { OneTapReport } from "./oneTapReport";

type Props = {
  running: boolean;
  address: string;
  onAddress: (address: string) => void;
  /** Addresses used lately, newest first. */
  recent: string[];
  cameraDisplayOn: boolean;
  onCameraDisplay: (on: boolean) => void;
  status: Status | null;
  /** A newer published version, opening its release page. */
  newRelease: string | null;
  /** One-tap connect: working, what to do on the camera meanwhile, and how the last one went. */
  oneTap: { working: boolean; hint: string; report: OneTapReport | null; start: () => void; hideReport: () => void };
  onStart: () => void;
  onStop: () => void;
};

export function Connection(props: Props) {
  const [listOpen, setListOpen] = useState(false);
  const { report, hideReport } = props.oneTap;
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
        {report && <Report report={report} onHide={hideReport} />}
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
      {/* The panel is only as tall as the picture: a report takes these places, so its sentence stays on screen. */}
      {report ? (
        <Report report={report} onHide={hideReport} />
      ) : (
        <>
          <div className="connect-title">
            <h1>Groundglass</h1>
            {props.newRelease && (
              <button
                type="button"
                className="new-release"
                aria-label={`Groundglass ${props.newRelease} is out`}
                onClick={() => void openReleasePage()}
              >
                <Icon name="newRelease" />
                {props.newRelease}
              </button>
            )}
          </div>
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
        </>
      )}
      <div className="start-row">
        <button type="submit" className="primary" disabled={!props.address.trim() || props.oneTap.working}>
          Start
        </button>
        <button
          type="button"
          className="secondary"
          aria-label="Connect over Bluetooth"
          aria-busy={props.oneTap.working}
          disabled={!canConnectOverBluetooth || props.oneTap.working}
          onClick={props.oneTap.start}
        >
          <Icon name="bluetooth" />
        </button>
      </div>
      {props.oneTap.working && <p role="status">{props.oneTap.hint || "Connecting…"}</p>}
      {screenToggle}
    </form>
  );
}

/** How far one-tap connect got: each step, then what to do. */
function Report({ report, onHide }: { report: OneTapReport; onHide: () => void }) {
  return (
    <button type="button" className="one-tap" title="Tap to hide" role="status" onClick={onHide}>
      <span className="one-tap-steps">
        {report.steps.map((step) => (
          <span key={step.label} className={step.state}>
            {step.label}
          </span>
        ))}
      </span>
      {report.message && <span>{report.message}</span>}
      {report.detail && <span className="detail">{report.detail}</span>}
    </button>
  );
}

type BadgeProps = { status: Status | null; battery: Battery | null; open: boolean; onToggle: () => void };

/** Under this, the battery shows red, as the camera's does. */
const BATTERY_LOW = 20;

/** The status on the picture, kept short, with the camera's battery; tapping it opens the connection panel. */
export function StatusBadge({ status, battery, open, onToggle }: BadgeProps) {
  return (
    <button className={`status ${status?.state ?? "connecting"}`} aria-expanded={open} onClick={onToggle}>
      {status?.state === "live" ? `${status.fps} fps` : status?.state === "lost" ? "Retrying…" : "Connecting…"}
      {battery !== null && (
        <span
          className={`battery ${battery.percent <= BATTERY_LOW ? "low" : ""}`}
          aria-label={`Battery ${battery.exact ? "" : "about "}${battery.percent}%`}
        >
          <i>
            <b style={{ width: `${battery.percent}%` }} />
          </i>
          {battery.exact && `${battery.percent}%`}
        </span>
      )}
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
