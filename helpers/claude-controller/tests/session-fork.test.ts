/** Checks exact fork boundaries and canonical UUID mapping independently of native storage details. */
import { expect, test } from "bun:test";

import { forkBefore } from "../src/session-fork";
import type { SessionArchive } from "../src/session-archive";
import type { PromptSources } from "../src/prompt-sources";
import type { SessionApi } from "../src/sessions";
import type { ObjectValue } from "../src/protocol";

function fixture(extra = false) {
  const source: ObjectValue[] = [
    { type: "user", uuid: "human", message: { role: "user", content: "same" }, origin: { kind: "human" } },
    { type: "user", uuid: "automatic", message: { role: "user", content: "same" } },
    { type: "user", uuid: "cut", message: { role: "user", content: "cut" }, origin: { kind: "human" } }];
  const copied: ObjectValue[] = [
    { ...source[0], uuid: "new-human", message: { content: "same", role: "user" }, is_meta: false },
    { ...source[1], uuid: "new-auto", message: { content: "same", role: "user" }, isCompletedLocalCommand: false }];
  copied.forEach((record, index) => { record.forkedFrom = { sessionId: "source", messageUuid: source[index]!.uuid }; });
  if (extra) copied.push({ ...source[2], uuid: "EXTRA" });
  const writes: unknown[] = [];
  const sources = { source: (message: ObjectValue) => message.origin ? "human" : "automatic",
    inputKind: (uuid: string) => uuid === "human" ? "steer" : undefined,
    saveAll: (records: { uuid: string; source: string; inputKind?: string }[]) =>
      records.forEach(record => writes.push([record.uuid, record.source, record.inputKind])) } as unknown as PromptSources;
  const api = { forkSession: async () => ({ sessionId: "fork" }) } as unknown as SessionApi;
  const archive = async (_dir: string, id: string) => ({ entries: (offset: number, limit: number) =>
    (id === "source" ? source : copied).slice(offset, offset + limit),
    forkCut: (uuid: string) => ({ sessionId: "source", uuid, records: new Map() }),
    forkEntries: () => (offset: number, limit: number) => copied.slice(offset, offset + limit) }) as unknown as SessionArchive;
  return { api, sources, archive, writes, copied };
}

test("fork compares canonical content despite key order and absent public-derived flags", async () => {
  const { api, sources, archive, writes } = fixture();
  expect(await forkBefore(api, sources, new AbortController().signal, "/project", "source", "cut", archive))
    .toEqual({ sessionId: "fork", fresh: false });
  expect(writes).toEqual([["new-human", "human", "steer"], ["new-auto", "automatic", undefined]]);
});

test("fork refuses extra future rows even when every prefix record maps correctly", async () => {
  const { api, sources, archive, writes } = fixture(true);
  expect(forkBefore(api, sources, new AbortController().signal, "/project", "source", "cut", archive))
    .rejects.toMatchObject({ code: "fork_mapping_failed" });
  expect(writes).toEqual([]);
});

test("missing or changed prefix records do not publish any prompt classifications", async () => {
  for (const change of ["missing", "content", "mapping"]) {
    const { api, sources, archive, writes, copied } = fixture();
    if (change === "missing") copied.pop();
    if (change === "content") copied[1]!.message = { role: "user", content: "changed" };
    if (change === "mapping") delete copied[1]!.forkedFrom;
    expect(forkBefore(api, sources, new AbortController().signal, "/project", "source", "cut", archive))
      .rejects.toMatchObject({ code: "fork_mapping_failed" });
    expect(writes).toEqual([]);
  }
});

test("missing exact anchor can wait for disk flush and can be canceled without a stale fallback", async () => {
  const { api, sources, archive } = fixture();
  let reads = 0;
  const delayed = async (dir: string, id: string) => {
    if (id === "source" && reads++ === 0) return { entries: () => [] } as unknown as SessionArchive;
    return archive(dir, id);
  };
  expect(await forkBefore(api, sources, new AbortController().signal, "/project", "source", "cut", delayed))
    .toMatchObject({ sessionId: "fork" });
  const controller = new AbortController();
  setTimeout(() => controller.abort(new Error("stopped")), 20);
  expect(forkBefore(api, sources, controller.signal, "/project", "source", "absent", archive)).rejects.toThrow("stopped");
});
