/** Rejects stale native run preconditions without retaining task bodies or reviving evicted guards. */
import { expect, test } from "bun:test";

import { TaskRuns } from "../src/task-runs";
import { NativeOutput } from "../src/native-output";

function edge(runs: TaskRuns, id: string | undefined, subtype = "task_started", extra = {}) {
  runs.observe({ type: "system", subtype, task_id: "task", run_id: id, ...extra });
}

test("the newest native run wins and a terminal edge cannot be undone by late progress", () => {
  const runs = new TaskRuns();
  expect(() => runs.require("task", "a")).toThrow("stop was not sent");
  edge(runs, "a");
  runs.require("task", "a");
  edge(runs, "b");
  edge(runs, "a", "task_notification", { status: "completed" });
  expect(() => runs.require("task", "a")).toThrow("stop was not sent");
  runs.require("task", "b");
  edge(runs, "b", "task_updated", { patch: { status: "stopped" } });
  edge(runs, "b", "task_progress");
  expect(() => runs.require("task", "b")).toThrow("stop was not sent");
  edge(runs, "c");
  runs.require("task", "c");
});

test("membership runs follow native UTF-16 string order and missing run evidence stays unknown", () => {
  const runs = new TaskRuns();
  runs.observe({ type: "system", subtype: "background_tasks_changed", tasks: [
    { task_id: "task", run_id: "𐀀" }, { task_id: "other", run_id: "other-run" },
  ] });
  edge(runs, "\ue000");
  edge(runs, "𐀀", "task_notification", { status: "failed" });
  runs.require("task", "\ue000");
  runs.require("other", "other-run");
  edge(runs, undefined);
  edge(runs, "\ue000");
  expect(() => runs.require("task", "\ue000")).toThrow("unknown");
  edge(runs, "\ue001");
  runs.require("task", "\ue001");
  // Legacy callers explicitly omit a run precondition; no run proof is invented.
  runs.require("legacy", null);
  expect(() => runs.require("task", "")).toThrow("bounded nonempty");
});

test("the byte/count bounds disable guarded stops instead of forgetting newer run tombstones", () => {
  for (const wide of [false, true]) {
    const runs = new TaskRuns();
    edge(runs, "new");
    for (let index = 0; index < (wide ? 300 : 4097); index++) {
      runs.observe({ type: "system", subtype: "task_started", task_id: `task-${index}`, run_id: wide ? "x".repeat(4096) : "run" });
    }
    edge(runs, "old");
    expect(() => runs.require("task", "new")).toThrow("budget");
    expect(() => runs.require("task", "old")).toThrow("budget");
  }
});

test("malformed task frames cannot throw during native reads or restore an oversized run", () => {
  const runs = new TaskRuns();
  for (const value of [null, false, [], { type: "system", subtype: { toString: "unsafe" } },
    { type: "system", subtype: "task_started", task_id: "task", run_id: "run", status: { toString: "unsafe" } }]) {
    expect(() => runs.observe(value)).not.toThrow();
  }
  edge(runs, "x".repeat(4097));
  edge(runs, "run");
  expect(() => runs.require("task", "run")).toThrow("budget");
});

test("a replacement run fences stops even while its native frame waits unread in the SDK queue", () => {
  const runs = new TaskRuns();
  const native = new NativeOutput((error) => { throw error; });
  native.observeFrames(message => runs.observe(message));
  const frame = (id: string) => Buffer.from(JSON.stringify({ type: "system", subtype: "task_started",
    task_id: "task", run_id: id }) + "\n");
  native.write(frame("a"));
  runs.require("task", "a");
  native.write(frame("b"));
  expect(native.pendingEvents).toBe(2);
  expect(() => runs.require("task", "a")).toThrow("stop was not sent");
  runs.require("task", "b");
  native.destroy();
});
