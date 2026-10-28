// The camera's address and Start / Stop. Before Start it sits in the middle of
// the picture; while live it opens from the status at the top.
type Props = {
  running: boolean;
  address: string;
  onAddress: (address: string) => void;
  cameraDisplayOn: boolean;
  onCameraDisplay: (on: boolean) => void;
  status: string;
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
        <p>{props.status}</p>
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
