/** Rejects missing, altered or wrong-branch rows while proving only an imported hidden system suffix. */
import { expect, test } from "bun:test";

import { captureCut, forkTail, type IndexedEntry } from "../src/session-fork-tail";
import type { ObjectValue } from "../src/protocol";

function fixture() {
  const original: ObjectValue[] = [
    { type: "assistant", uuid: "leaf", parentUuid: null, message: { role: "assistant", content: "answer" } },
    { type: "system", subtype: "stop_hook_summary", uuid: "hook-one", parentUuid: "leaf", hookCount: 1, hookErrors: [] },
    { type: "attachment", uuid: "attachment", parentUuid: "hook-one", attachment: { type: "queued_command", source_uuid: "hook-one" } },
    { type: "system", subtype: "stop_hook_summary", uuid: "hook-two", parentUuid: "attachment", hookCount: 2,
      hookInfos: [{ command: "true", status: "completed" }], preventedContinuation: false, slug: "source-name" }];
  const source = new Map(original.map((entry, index) => [String(entry.uuid), { entry, seq: index + 1 }]));
  const copied = new Map(original.map((entry, index) => {
    const record: ObjectValue = { ...structuredClone(entry), uuid: "new-" + entry.uuid,
      parentUuid: entry.parentUuid ? "new-" + entry.parentUuid : null,
      sessionId: "fork", timestamp: "new time", forkedFrom: { sessionId: "source", messageUuid: entry.uuid } };
    delete record.slug;
    if (record.type === "attachment") (record.attachment as ObjectValue).source_uuid = "new-hook-one";
    return [String(record.uuid), { entry: record, seq: index + 1 }] as [string, IndexedEntry];
  }));
  const cut = captureCut("source", "hook-two", uuid => source.get(uuid));
  const candidates = [copied.get("new-hook-two")!];
  const prove = (leaf = "new-leaf") => forkTail(candidates, leaf, "source", uuid => copied.get(uuid), uuid => cut.records.get(uuid));
  return { copied, source, prove, candidates };
}

test("hidden hook summaries retain opaque facts and transparent mapped attachment links in source order", () => {
  const { prove } = fixture();
  expect(prove().map(record => record.uuid)).toEqual(["new-hook-one", "new-hook-two"]);
});

test("a changed opaque system payload is rejected even without a message field", () => {
  const { copied, prove } = fixture();
  copied.get("new-hook-two")!.entry.preventedContinuation = true;
  expect(() => prove()).toThrow("changed the forked system");
});

test("missing records, foreign mappings, wrong branches, bad order and hidden conversation rows are rejected", () => {
  for (const change of ["missing", "mapping", "branch", "order", "user", "assistant", "attachment", "reference"]) {
    const { copied, prove } = fixture();
    const hook = copied.get("new-hook-one")!;
    if (change === "missing") copied.delete("new-hook-one");
    if (change === "mapping") hook.entry.forkedFrom = { sessionId: "foreign", messageUuid: "hook-one" };
    if (change === "branch") hook.entry.parentUuid = "new-hook-two";
    if (change === "order") hook.seq = 100;
    if (change === "user" || change === "assistant") hook.entry.type = change;
    if (change === "attachment") (copied.get("new-attachment")!.entry.attachment as ObjectValue).type = "changed";
    if (change === "reference") (copied.get("new-attachment")!.entry.attachment as ObjectValue).source_uuid = "hook-one";
    expect(() => prove()).toThrow();
  }
});

test("duplicate or absent mapped cuts and a suffix ending at another canonical leaf are rejected", () => {
  const { prove, candidates } = fixture();
  expect(() => prove("another-leaf")).toThrow("not a main-thread system suffix");
  candidates.push({ ...candidates[0]!, entry: { ...candidates[0]!.entry, uuid: "duplicate-mapping" } });
  expect(() => prove()).toThrow("one exact cut record");
  candidates.length = 0;
  expect(() => prove()).toThrow("one exact cut record");
});
