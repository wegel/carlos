/** Exercises published task/optional-usage controls through the real SDK and a local fake CLI. */
import { afterEach, expect, test } from "bun:test";
import { Peer } from "./peer";
import { quotaWindows } from "../src/quota-windows";
import type { SDKControlGetUsageResponse } from "@anthropic-ai/claude-agent-sdk";

const peers: Peer[] = [];
afterEach(async () => { for (const peer of peers.splice(0)) await peer.dispose(); });

test("per-task stop is declared explicitly and acknowledgements do not fabricate task completion", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE); peers.push(peer);
  await peer.start({ perTaskStopAffordance: true });
  expect((await peer.send("task.stop", { taskId: "child-native" })).result).toEqual({ taskId: "child-native", stopRequested: true });
  expect((await peer.send("task.stop", { taskId: "fail-task" })).ok).toBe(false);
  expect((await peer.send("context")).ok).toBe(true);
  const controls = await peer.cliFrames();
  expect(controls.find(frame => frame.request?.subtype === "initialize")!.request.perTaskStopAffordance).toBe(true);
  expect(controls.find(frame => frame.request?.subtype === "stop_task")!.request.task_id).toBe("child-native");
  expect(peer.frames.some(frame => frame.message?.subtype === "task_notification")).toBe(false);
});

test("optional usage skips transcript behavior scans and keeps percent units and model buckets", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE); peers.push(peer);
  await peer.start();
  const reply = await peer.send("usage.optional");
  expect(reply.result.available).toBe(true);
  expect(reply.result.windows).toEqual(expect.arrayContaining([
    { key: "five_hour", label: "five hour", usedPercent: 25, resetsAt: 4070908800 },
    { key: "model:Fable", label: "Fable", usedPercent: 7, resetsAt: 4070995200 },
  ]));
  const control = (await peer.cliFrames()).find(frame => frame.request?.subtype === "get_usage");
  expect(control!.request.skip_behaviors).toBe(true);
  expect((await peer.send("interrupt")).ok).toBe(true);
});

test("missing malformed quota values stay absent rather than appearing as zero", () => {
  const report = { rate_limits_available: true, rate_limits: {
    five_hour: { utilization: null, resets_at: "2099-01-01T00:00:00Z" },
    seven_day: { utilization: 0.4, resets_at: null },
    seven_day_sonnet: { utilization: 101, resets_at: "2099-01-01T00:00:00Z" },
    seven_day_opus: { utilization: 0, resets_at: "2099-01-01T00:00:00Z" },
  }} as SDKControlGetUsageResponse;
  expect(quotaWindows(report)).toHaveLength(1);
  expect(quotaWindows(report)[0]!.usedPercent).toBe(0);
  report.rate_limits_available = false; expect(quotaWindows(report)).toEqual([]);
});

test("a parked optional usage request leaves callbacks and controls live and shutdown settles it", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE); peers.push(peer);
  await peer.start();
  await peer.prompt("park-usage");
  await peer.prompt("callbacks");
  await peer.wait(frame => frame.type === "callback" && frame.callbackId === "permission-1");
  const pending = peer.send("usage.optional");
  await peer.wait(frame => frame.event === "stderr" && frame.text.includes("optional usage parked"));
  expect((await peer.send("usage.optional")).result.reason).toBe("read_pending");
  expect((await peer.send("task.stop", { taskId: "child" })).ok).toBe(true);
  expect((await peer.send("callback.respond", { callbackId: "permission-1", answer: { behavior: "deny", message: "No" } })).ok).toBe(true);
  expect((await peer.send("interrupt")).ok).toBe(true);
  expect((await peer.send("shutdown")).ok).toBe(true);
  expect((await pending).error.code).toBe("closed");
});
