// The two kinds of control in the side bands. A tool (an aid drawn over the
// picture) is an icon with its setting under it, lit when on, with a dot when
// tapping it opens choices. A setting (the camera's exposure, the look) is its
// name small and its value large.
import { Icon, type IconName } from "./icons";

type ToolProps = {
  icon: IconName;
  label: string;
  /** What it is set to, shown under the icon: "98%", "Thirds", "×5", "Off". None: the icon says it. */
  value?: string;
  on: boolean;
  /** Undefined for a plain toggle; true or false for a tool with choices, whether they are open. */
  open?: boolean;
  disabled?: boolean;
  onClick: () => void;
};

export function Tool({ icon, label, value, on, open, disabled, onClick }: ToolProps) {
  return (
    <button
      className={`tool${open !== undefined ? " opens" : ""}`}
      aria-label={value === undefined ? label : `${label}: ${value}`}
      aria-pressed={on}
      aria-expanded={open}
      disabled={disabled}
      onClick={onClick}
    >
      <Icon name={icon} />
      {/* Empty without a value, keeping the icon in line with the others. */}
      <span aria-hidden={value === undefined}>{value ?? " "}</span>
    </button>
  );
}

type SettingProps = {
  label: string;
  value: string;
  open: boolean;
  disabled?: boolean;
  /** Kept as it is by ETTR: a lock in the corner. */
  pinned?: boolean;
  /** Flashes: ETTR found it at the end of its range. */
  flash?: boolean;
  onClick: () => void;
};

export function Setting({ label, value, open, disabled, pinned, flash, onClick }: SettingProps) {
  return (
    <button
      className={`setting${flash ? " limit" : ""}`}
      aria-label={`${label}: ${value}${pinned ? ", pinned" : ""}`}
      aria-expanded={open}
      disabled={disabled}
      onClick={onClick}
    >
      <small>
        {label}
        {pinned && <Icon name="pin" />}
      </small>
      <strong>{value}</strong>
    </button>
  );
}
