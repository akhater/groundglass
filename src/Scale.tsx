// An exposure setting's values as a scale beside the picture: upright in a
// side band, across under the picture on a tall screen. Tap a value, or step
// with - and +. It stays open, so stepping through values is one tap each.
import { useEffect, useRef, type ReactNode } from "react";
import { Icon } from "./icons";

type Props = {
  label: string;
  values: string[];
  value: string;
  onPick: (value: string) => void;
  onClose: () => void;
  /** Beside the title, e.g. the ETTR lock. */
  action?: ReactNode;
};

export function Scale({ label, values, value, onPick, onClose, action }: Props) {
  const list = useRef<HTMLDivElement>(null);
  const first = useRef(true);
  const index = values.indexOf(value);

  // Keep the current value in the middle, also when the camera changes it.
  useEffect(() => {
    const row = list.current;
    const current = row?.querySelector<HTMLElement>('[aria-pressed="true"]');
    if (!row || !current) return;
    row.scrollTo({
      left: current.offsetLeft - (row.clientWidth - current.offsetWidth) / 2,
      top: current.offsetTop - (row.clientHeight - current.offsetHeight) / 2,
      behavior: first.current ? "instant" : "smooth",
    });
    first.current = false;
  }, [value]);

  const step = (by: number) => {
    const next = values[index + by];
    if (next !== undefined) onPick(next);
  };

  return (
    <div className="picker dial camera" role="dialog" aria-label={label}>
      <header>
        <h2>{label}</h2>
        {action}
        <button className="close" onClick={onClose} aria-label={`Close ${label}`}>
          <Icon name="close" />
        </button>
      </header>
      <div className="steps">
        <button className="step" onClick={() => step(-1)} disabled={index <= 0} aria-label={`Previous ${label}`}>
          <Icon name="minus" />
        </button>
        <div className="values" ref={list}>
          {values.map((v) => (
            <button key={v} aria-pressed={v === value} onClick={() => onPick(v)}>
              {v}
            </button>
          ))}
        </div>
        <button
          className="step"
          onClick={() => step(1)}
          disabled={index < 0 || index >= values.length - 1}
          aria-label={`Next ${label}`}
        >
          <Icon name="plus" />
        </button>
      </div>
    </div>
  );
}
