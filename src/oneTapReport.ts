// How far one-tap connect got, in plain words for the connection screen.
import type { BluetoothLink, Survey } from "./camera";

export type StepState = "done" | "failed" | "skipped";
export type ReportStep = { label: string; state: StepState };
export type OneTapReport = {
  steps: ReportStep[];
  /** What to do next, when it didn't reach live view. */
  message: string | null;
  /** The camera's own answer, for working out what it wants. */
  detail: string | null;
};

/**
 * What happened on the camera's Wi-Fi once joined. `refused`: the reason it gave for turning the pairing down.
 * `attempts`: how each GUID order went.
 */
export type Answer = { protocol?: string; refused?: number; live: boolean; error?: string; attempts?: string[]; survey?: Survey };

/** The camera doesn't accept this client: only pairing again can change that. */
const REJECTED_INITIATOR = 1;

/** The first step not reached is the one that failed; the rest were never tried. */
function steps(reached: [string, boolean][]): ReportStep[] {
  const failed = reached.findIndex(([, done]) => !done);
  return reached.map(([label, done], i) => ({
    label,
    state: done ? "done" : i === failed ? "failed" : "skipped",
  }));
}

function detail(link: BluetoothLink, answer?: Answer): string | null {
  const { error, attempts, survey } = answer ?? {};
  const parts = [
    link.registration,
    ...(attempts ?? []),
    error,
    ...(survey?.services ?? []),
    survey?.ports.length ? `ports ${survey.ports.join(", ")}` : undefined,
  ];
  return parts.filter(Boolean).join(" · ") || null;
}

function nextStep({ protocol, refused }: Answer): string {
  if (refused === REJECTED_INITIATOR) {
    return "The camera doesn't know this app yet: forget the camera in the phone's Bluetooth settings, open its pairing screen, then tap again.";
  }
  if (refused !== undefined) return "The camera answered but refused Groundglass: turn it off and on, then tap again.";
  if (protocol) return "Live view didn't start: turn the camera off and on, then tap again.";
  return "Nothing answered on the camera's Wi-Fi: turn the camera off and on, then tap again.";
}

export function oneTapReport(link: BluetoothLink, answer?: Answer): OneTapReport {
  const report = {
    steps: steps([
      ["Camera awake", link.woke],
      ["Wi-Fi started", link.wifiStarted],
      ["Joined", link.joined],
      [answer?.protocol ? `${answer.protocol} answered` : "Protocol answered", !!answer?.protocol],
    ]),
    message: link.failure,
    detail: detail(link, answer),
  };
  if (!answer || answer.live) return report;
  return { ...report, message: nextStep(answer) };
}
