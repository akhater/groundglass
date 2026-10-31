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

/** What happened on the camera's Wi-Fi once joined. */
export type Answer = { protocol?: string; live: boolean; error?: string; survey?: Survey };

/** The first step not reached is the one that failed; the rest were never tried. */
function steps(reached: [string, boolean][]): ReportStep[] {
  const failed = reached.findIndex(([, done]) => !done);
  return reached.map(([label, done], i) => ({
    label,
    state: done ? "done" : i === failed ? "failed" : "skipped",
  }));
}

function detail({ error, survey }: Answer): string | null {
  const parts = [
    error,
    ...(survey?.services ?? []),
    survey?.ports.length ? `ports ${survey.ports.join(", ")}` : undefined,
  ];
  return parts.filter(Boolean).join(" · ") || null;
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
    detail: answer ? detail(answer) : null,
  };
  if (!answer || answer.live) return report;
  const message = answer.protocol
    ? "Live view didn't start: turn the camera off and on, then tap again."
    : "Nothing answered on the camera's Wi-Fi: turn the camera off and on, then tap again.";
  return { ...report, message };
}
