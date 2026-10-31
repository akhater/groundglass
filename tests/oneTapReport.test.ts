import assert from "node:assert/strict";
import { test } from "node:test";
import { oneTapReport } from "../src/oneTapReport.ts";

const nothing = { woke: false, wifiStarted: false, joined: false, address: null, registration: null, failure: null };
const joined = { ...nothing, woke: true, wifiStarted: true, joined: true, address: "192.168.1.2" };
const states = (report: ReturnType<typeof oneTapReport>) => report.steps.map((step) => step.state);

test("a step that failed is marked, and the ones after it were never reached", () => {
  const report = oneTapReport({ ...nothing, failure: "Turn Bluetooth on, then tap again." });
  assert.deepEqual(states(report), ["failed", "skipped", "skipped", "skipped"]);
  assert.equal(report.message, "Turn Bluetooth on, then tap again.");
});

test("each step Bluetooth reached is done", () => {
  const report = oneTapReport({ ...nothing, woke: true, wifiStarted: true, failure: "Tap again." });
  assert.deepEqual(states(report), ["done", "done", "failed", "skipped"]);
});

test("the protocol that answered is named and needs no message once live", () => {
  const report = oneTapReport(joined, { protocol: "PTP/IP", live: true });
  assert.deepEqual(states(report), ["done", "done", "done", "done"]);
  assert.equal(report.steps[3].label, "PTP/IP answered");
  assert.equal(report.message, null);
  assert.equal(report.detail, null);
});

test("a protocol that answers but won't start live view says what to do, with the camera's words", () => {
  const report = oneTapReport(joined, { protocol: "PTP/IP", live: false, error: "the camera answered 0x2002" });
  assert.deepEqual(states(report), ["done", "done", "done", "done"]);
  assert.match(report.message!, /live view.*tap again/i);
  assert.equal(report.detail, "the camera answered 0x2002");
});

test("when nothing answers, the report lists what the camera does offer", () => {
  const survey = { services: ["ICPO-SmartPhoneEOSSystemService"], ports: [15740, 49152] };
  const report = oneTapReport(joined, { live: false, error: "no answer", survey });
  assert.deepEqual(states(report), ["done", "done", "done", "failed"]);
  assert.match(report.message!, /tap again/i);
  assert.equal(report.detail, "no answer · ICPO-SmartPhoneEOSSystemService · ports 15740, 49152");
});

test("a survey that found nothing adds nothing to the detail", () => {
  const report = oneTapReport(joined, { live: false, error: "no answer", survey: { services: [], ports: [] } });
  assert.equal(report.detail, "no answer");
});

test("a camera that refuses did answer: the steps and the sentence both say so", () => {
  const survey = { services: ["ICPO-SmartPhoneEOSSystemService"], ports: [15740] };
  const report = oneTapReport(joined, { protocol: "PTP/IP", refused: 2, live: false, error: "refused to pair, reason 0x2", survey });
  assert.deepEqual(states(report), ["done", "done", "done", "done"]);
  assert.equal(report.steps[3].label, "PTP/IP answered");
  assert.match(report.message!, /refused/i);
  assert.doesNotMatch(report.message!, /nothing answered/i);
  assert.equal(report.detail, "refused to pair, reason 0x2 · ICPO-SmartPhoneEOSSystemService · ports 15740");
});

test("a camera that doesn't know this client says to pair, not to restart it", () => {
  const report = oneTapReport(joined, { protocol: "PTP/IP", refused: 1, live: false, error: "refused to pair, reason 0x1" });
  assert.match(report.message!, /forget the camera.*pairing screen.*tap again/i);
  assert.doesNotMatch(report.message!, /off and on/i);
});

test("how the Bluetooth registration went leads the detail, even when Wi-Fi never started", () => {
  const registration = "id ok, name ok, mode refused";
  assert.equal(oneTapReport({ ...nothing, registration, failure: "Tap again." }).detail, registration);
  const report = oneTapReport({ ...joined, registration }, { live: false, error: "no answer" });
  assert.equal(report.detail, `${registration} · no answer`);
});
