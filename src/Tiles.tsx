// Choices laid out all at once, as tiles: zebra levels, peaking, guides, crops,
// looks. It stays open, so comparing choices is one tap each; tapping the
// picture or the cross closes it.
import type { ReactNode } from "react";
import { Icon } from "./icons";

export type Tile = { value: string; label: string; art?: ReactNode };

type Props = {
  label: string;
  tiles: Tile[];
  value: string;
  onPick: (value: string) => void;
  onClose: () => void;
  /** Beside the title, e.g. Rotate. */
  action?: ReactNode;
  /** Before the tiles, e.g. a tile to add looks. */
  first?: ReactNode;
  /** Which band asked: the tiles open on that side of the picture. */
  side: "assists" | "camera";
};

export function Tiles({ label, tiles, value, onPick, onClose, action, first, side }: Props) {
  return (
    <div className={`picker tiles ${side}`} role="dialog" aria-label={label}>
      <header>
        <h2>{label}</h2>
        {action}
        <button className="close" onClick={onClose} aria-label={`Close ${label}`}>
          <Icon name="close" />
        </button>
      </header>
      <div className="grid">
        {first}
        {tiles.map((tile) => (
          <button key={tile.value} className="tile" aria-pressed={tile.value === value} onClick={() => onPick(tile.value)}>
            {tile.art}
            <span>{tile.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
