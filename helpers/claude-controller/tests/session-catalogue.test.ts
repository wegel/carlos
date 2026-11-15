/** Compact picker metadata keeps original addresses while excluding unbounded native fields. */
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { Sessions, type SessionApi } from "../src/sessions";
import { Peer } from "./peer";

test("compact saved-session pages preserve SDK offsets and Unicode without copying future fields", async () => {
  let seen: unknown;
  const api = { listSessions: async (params: unknown) => {
    seen = params;
    return [{ sessionId: "native-id", customTitle: "🦊".repeat(1000), summary: "é🦊".repeat(200000),
      cwd: "/project", createdAt: 1000, lastModified: 2000, futureNativeField: "private" }];
  } } as unknown as SessionApi;
  const sessions = new Sessions(new AbortController().signal, api);
  const result = await sessions.dispatch("sessions.list", { cwd: "/project", offset: 7, limit: 1, compact: true }) as any;
  expect(seen).toMatchObject({ dir: "/project", offset: 7, limit: 1, includeProgrammatic: true, includeWorktrees: true });
  expect(result.offset).toBe(7);
  expect(result.nextOffset).toBe(8);
  expect(result.chunk).toBeUndefined();
  expect(result.sessions[0]).toMatchObject({ sessionId: "native-id", cwd: "/project", createdAt: 1000, lastModified: 2000 });
  expect(Buffer.byteLength(result.sessions[0].customTitle)).toBe(512);
  expect(Buffer.byteLength(result.sessions[0].summary)).toBeLessThanOrEqual(4096);
  expect(result.sessions[0].summary.includes("�")).toBe(false);
  expect(result.sessions[0].futureNativeField).toBeUndefined();
});

test("compact picker metadata never truncates an oversized native address into another address", async () => {
  const api = { listSessions: async () => [{ sessionId: "x".repeat(4097), summary: "hidden" }] } as unknown as SessionApi;
  const sessions = new Sessions(new AbortController().signal, api);
  const result = await sessions.dispatch("sessions.list", { cwd: "/project", compact: true }) as any;
  expect(result.sessions[0].sessionId).toBeUndefined();
  expect(result.nextOffset).toBeNull();
});

test("compiled native reader returns compact Unicode titles without rewriting saved session text", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  try {
    const id = crypto.randomUUID();
    const directory = join(peer.directory, "claude", "projects", peer.directory.replace(/[^a-zA-Z0-9]/g, "-"));
    await mkdir(directory, { recursive: true });
    const prompt = "Original prompt é🦊";
    await writeFile(join(directory, `${id}.jsonl`), JSON.stringify({ type: "user", uuid: crypto.randomUUID(),
      parentUuid: null, cwd: peer.directory, sessionId: id, version: "2.1.322", timestamp: new Date().toISOString(),
      message: { role: "user", content: prompt } }) + "\n");
    const title = "Renommé🦊".repeat(90);
    expect((await peer.send("sessions.rename", { cwd: peer.directory, sessionId: id, title })).ok).toBe(true);
    const listed = await peer.send("sessions.list", { cwd: peer.directory, compact: true, limit: 1 });
    expect(listed.ok, JSON.stringify(listed)).toBe(true);
    expect(listed.result.sessions[0].sessionId).toBe(id);
    expect(Buffer.byteLength(listed.result.sessions[0].customTitle)).toBeLessThanOrEqual(512);
    expect(listed.result.sessions[0].customTitle.includes("�")).toBe(false);
    const saved = await peer.send("sessions.info", { cwd: peer.directory, sessionId: id });
    expect(saved.result.customTitle).toBe(title);
    const messages = await peer.send("sessions.read", { cwd: peer.directory, sessionId: id });
    expect(messages.result.messages[0].message.message.content).toBe(prompt);
  } finally {
    await peer.dispose();
  }
});
