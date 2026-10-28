// A row of choices over the bottom of the picture: tap one to use it. It stays
// open, so stepping through values (or comparing looks) is one tap each.
import { useEffect, useRef, type ReactNode } from "react";

export type Choice = { value: string; label: string };

type Props = {
  label: string;
  choices: Choice[];
  value: string;
  onPick: (value: string) => void;
  onClose: () => void;
  /** Shown after the choices, e.g. a button to add looks. */
  extra?: ReactNode;
};

export function Strip({ label, choices, value, onPick, onClose, extra }: Props) {
  const list = useRef<HTMLDivElement>(null);
  const first = useRef(true);
  const index = choices.findIndex((choice) => choice.value === value);

  // Keep the current choice in the middle, also when the camera changes it.
  useEffect(() => {
    const row = list.current;
    const current = row?.querySelector<HTMLElement>('[aria-pressed="true"]');
    if (!row || !current) return;
    const left = current.offsetLeft - (row.clientWidth - current.offsetWidth) / 2;
    row.scrollTo({ left, behavior: first.current ? "instant" : "smooth" });
    first.current = false;
  }, [value]);

  const step = (by: number) => {
    const next = choices[index + by];
    if (next) onPick(next.value);
  };

  return (
    <div className="strip" role="dialog" aria-label={label}>
      <button className="step" onClick={() => step(-1)} disabled={index <= 0} aria-label={`Previous ${label}`}>
        ‹
      </button>
      <div className="choices" ref={list}>
        {choices.map((choice) => (
          <button key={choice.value} aria-pressed={choice.value === value} onClick={() => onPick(choice.value)}>
            {choice.label}
          </button>
        ))}
        {extra}
      </div>
      <button
        className="step"
        onClick={() => step(1)}
        disabled={index < 0 || index >= choices.length - 1}
        aria-label={`Next ${label}`}
      >
        ›
      </button>
      <button className="step" onClick={onClose} aria-label={`Close ${label}`}>
        ✕
      </button>
    </div>
  );
}
