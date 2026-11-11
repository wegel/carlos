/** Exercises official SDK session readers and mutations against isolated native-format fixtures. */
import { afterEach, expect, test } from "bun:test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

import { Peer, type Frame } from "./peer";

const peers: Peer[] = [];
afterEach(async () => { for (const peer of peers.splice(0)) await peer.dispose(); });

async function fixture(count = 3): Promise<{ cli: Peer; id: string; path: string; records: Frame[] }> {
  const cli = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(cli);
  const id = crypto.randomUUID();
  const directory = join(cli.directory, "claude", "projects", cli.directory.replace(/[^a-zA-Z0-9]/g, "-"));
  await mkdir(directory, { recursive: true });
  const records: Frame[] = [];
  for (let index = 0; index < count; index++) {
    records.push({ type: "user", uuid: crypto.randomUUID(), parentUuid: records.at(-1)?.uuid ?? null,
      cwd: cli.directory, sessionId: id, version: "2.1.322", timestamp: new Date().toISOString(),
      origin: { kind: "human" }, message: { role: "user", content: `Prompt ${index} é🦊` } });
    records.push({ type: "assistant", uuid: crypto.randomUUID(), parentUuid: records.at(-1)?.uuid,
      cwd: cli.directory, sessionId: id, version: "2.1.322", timestamp: new Date().toISOString(),
      message: { role: "assistant", content: [{ type: "text", text: `Reply ${index}` }] } });
  }
  const path = join(directory, id + ".jsonl");
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  return { cli, id, path, records };
}

const params = (cli: Peer, sessionId: string, extra: Frame = {}) => ({ cwd: cli.directory, sessionId, ...extra });

test("session listing includes programmatic entries; bounded pages preserve raw fields and Unicode rename", async () => {
  const { cli, id, records, path } = await fixture(1000);
  records[0]!.entrypoint = "sdk-ts";
  records[0]!.futureMetadata = { retained: true };
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  const listed = await cli.send("sessions.list", { cwd: cli.directory, limit: 1 });
  expect(listed.result.sessions[0].sessionId).toBe(id);
  const first = await cli.send("sessions.read", params(cli, id));
  expect(first.result.messages).toHaveLength(64);
  expect(first.result.messages[0]).toMatchObject({ source: "human", message: { futureMetadata: { retained: true } } });
  const next = await cli.send("sessions.read", params(cli, id, { offset: first.result.nextOffset, snapshotId: first.result.snapshotId }));
  expect(next.result.messages[0].message.uuid).toBe(records[64]!.uuid);
  expect((await cli.send("sessions.rename", params(cli, id, { title: "Renommé é🦊" }))).result.customTitle).toBe("Renommé é🦊");
  expect((await cli.send("sessions.info", params(cli, id))).result.customTitle).toBe("Renommé é🦊");
  expect((await cli.send("sessions.read", params(cli, id, { snapshotId: "stale" }))).error.code).toBe("stale_snapshot");
});

test("official fork excludes selected prompt, remaps UUIDs, and keeps source bytes unchanged", async () => {
  const { cli, id, path, records } = await fixture();
  const original = await readFile(path, "utf8");
  const fork = await cli.send("sessions.forkBefore", params(cli, id, { beforeMessageId: records[2]!.uuid }));
  expect(fork.ok).toBe(true);
  const messages = (await cli.send("sessions.read", params(cli, fork.result.sessionId))).result.messages;
  expect(messages).toHaveLength(2);
  expect(messages[0].message.uuid).not.toBe(records[0]!.uuid);
  expect(messages[0].source).toBe("human");
  expect(messages[0].message.message).toEqual(records[0]!.message);
  expect(await readFile(path, "utf8")).toBe(original);
  const empty = await cli.send("sessions.forkBefore", params(cli, id, { beforeMessageId: records[0]!.uuid }));
  expect(empty.result).toMatchObject({ fresh: true });
  expect(empty.result.sessionId).not.toBe(id);
});

test("SDK-hidden hook suffix validates exactly after plain or compacted history without changing normal projection", async () => {
  for (const compact of [false, true]) {
    const { cli, id, path, records } = await fixture(2);
    const prefix = records.slice(0, 2);
    if (compact) {
      const boundary = { ...records[1], type: "system", subtype: "compact_boundary", uuid: crypto.randomUUID(),
        parentUuid: null, logicalParentUuid: records[1]!.uuid, message: undefined };
      const summary = { ...records[0], uuid: crypto.randomUUID(), parentUuid: boundary.uuid,
        origin: undefined, isCompactSummary: true, isVisibleInTranscriptOnly: true };
      prefix.push(boundary, summary, { ...records[1], uuid: crypto.randomUUID(), parentUuid: summary.uuid });
    }
    const attachment = { ...records[1], type: "attachment", uuid: crypto.randomUUID(), parentUuid: prefix.at(-1)!.uuid,
      message: undefined, attachment: { type: "deferred_tools_record", source_uuid: prefix.at(-1)!.uuid } };
    const hook = { ...records[1], type: "system", subtype: "stop_hook_summary", uuid: crypto.randomUUID(), parentUuid: attachment.uuid,
      message: undefined, hookCount: 1, hookInfos: [{ command: "true", status: "completed" }], preventedContinuation: false };
    const secondHook = { ...hook, uuid: crypto.randomUUID(), parentUuid: hook.uuid, hookCount: 2 };
    records[2]!.parentUuid = secondHook.uuid;
    await writeFile(path, [...prefix, attachment, hook, secondHook, ...records.slice(2)].map(record => JSON.stringify(record)).join("\n") + "\n");
    const original = await readFile(path, "utf8");
    const wrongProject = join(cli.directory, "claude", "projects", "-different-project");
    await mkdir(wrongProject);
    await writeFile(join(wrongProject, id + ".jsonl"), records.slice(0, 2).map(record => JSON.stringify(record)).join("\n") + "\n");
    const read = await cli.send("sessions.read", params(cli, id));
    expect(read.result.messages.filter((row: Frame) => row.message.subtype === "stop_hook_summary")).toHaveLength(2);
    const fork = await cli.send("sessions.forkBefore", params(cli, id, { beforeMessageId: records[2]!.uuid }));
    expect(fork.ok, JSON.stringify(fork)).toBe(true);
    const copied = (await cli.send("sessions.read", params(cli, fork.result.sessionId))).result.messages;
    expect(copied).toHaveLength(prefix.length);
    expect(copied.some((row: Frame) => row.message.subtype === "stop_hook_summary")).toBe(false);
    const file = (await readFile(join(dirname(path), fork.result.sessionId + ".jsonl"), "utf8"))
      .trim().split("\n").map(line => JSON.parse(line)) as Frame[];
    const hooks = file.filter(row => row.subtype === "stop_hook_summary");
    expect(hooks.map(row => row.forkedFrom.messageUuid)).toEqual([hook.uuid, secondHook.uuid]);
    expect(hooks.map(row => row.hookCount)).toEqual([1, 2]);
    expect(hooks.every(row => row.preventedContinuation === false)).toBe(true);
    expect(await readFile(path, "utf8")).toBe(original);
  }
});

test("read-only archive keeps the selected branch before compaction and excludes abandoned prompts", async () => {
  const { cli, id, path, records } = await fixture();
  const abandoned = { ...records[2], uuid: crypto.randomUUID(), parentUuid: records[1]!.uuid,
    message: { role: "user", content: "ABANDONED" } };
  const boundary = { type: "system", subtype: "compact_boundary", uuid: crypto.randomUUID(), parentUuid: null,
    logicalParentUuid: records[3]!.uuid, cwd: cli.directory, sessionId: id, timestamp: new Date().toISOString(), content: "Compacted" };
  const summary = { ...records[4], uuid: crypto.randomUUID(), parentUuid: boundary.uuid,
    isCompactSummary: true, isVisibleInTranscriptOnly: true, origin: undefined };
  await writeFile(path, [...records.slice(0, 4), abandoned, boundary, summary].map(record => JSON.stringify(record)).join("\n") + "\n");
  const read = await cli.send("sessions.read", params(cli, id));
  expect(read.result.messages).toHaveLength(6);
  expect(read.result.messages.some((row: Frame) => row.message.uuid === abandoned.uuid)).toBe(false);
  expect(read.result.messages[4].message).toMatchObject({ subtype: "compact_boundary", logicalParentUuid: records[3]!.uuid });
  expect(read.result.messages[5].source).toBe("system");
  const fork = await cli.send("sessions.forkBefore", params(cli, id, { beforeMessageId: records[2]!.uuid }));
  expect(fork.ok).toBe(true);
  expect((await cli.send("sessions.read", params(cli, fork.result.sessionId))).result.messages).toHaveLength(2);
});

test("large native records use bounded chunks and reconstruct every Unicode byte", async () => {
  const { cli, id, path, records } = await fixture(1);
  records[0]!.message.content = "é🦊".repeat(180000);
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  let reply = await cli.send("sessions.read", params(cli, id));
  const chunks: Buffer[] = [];
  for (;;) {
    const chunk = reply.result.chunk;
    expect(chunk.totalBytes).toBeGreaterThan(512 * 1024);
    expect(Buffer.byteLength(JSON.stringify(reply))).toBeLessThan(1024 * 1024);
    chunks.push(Buffer.from(chunk.data, "base64"));
    if (reply.result.chunkOffset === null) break;
    reply = await cli.send("sessions.read", params(cli, id, { snapshotId: reply.result.snapshotId,
      offset: reply.result.nextOffset, chunkOffset: reply.result.chunkOffset }));
  }
  expect(JSON.parse(Buffer.concat(chunks).toString()).message.message.content).toBe(records[0]!.message.content);
  expect(reply.result.nextOffset).toBe(1);
});

test("Carlos source records survive restart and UUID-remapped forks without guessing from identical text", async () => {
  const { cli, id, path, records } = await fixture(3);
  await cli.start();
  await cli.prompt("identical", { uuid: records[0]!.uuid, origin: { kind: "human" }, priority: "later" });
  await cli.prompt("identical", { uuid: records[2]!.uuid, origin: { kind: "auto-continuation" }, priority: "later" });
  records[0]!.isQueuedCommand = true;
  delete records[0]!.origin;
  delete records[2]!.origin;
  records[0]!.message.content = records[2]!.message.content = "identical";
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  await cli.send("shutdown");
  await cli.exited;
  const restarted = new Peer(cli.directory, process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(restarted);
  const read = await restarted.send("sessions.read", params(restarted, id));
  expect(read.result.messages[0].source).toBe("human");
  expect(read.result.messages[2].source).toBe("automatic");
  const fork = await restarted.send("sessions.forkBefore", params(restarted, id, { beforeMessageId: records[4]!.uuid }));
  expect(fork.ok).toBe(true);
  const copied = await restarted.send("sessions.read", params(restarted, fork.result.sessionId));
  expect(copied.result.messages[0].source).toBe("human");
  expect(copied.result.messages[2].source).toBe("automatic");
});

test("SDK-imported child and opaque metadata remain separately readable without creating main-thread prompts", async () => {
  const { cli, id, path, records } = await fixture(1);
  const directory = join(path.slice(0, -6), "subagents");
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, "agent-child.jsonl"), JSON.stringify({ ...records[0], isSidechain: true,
    agentId: "child", futureChild: { retained: true } }) + "\n");
  await writeFile(path, JSON.stringify({ type: "future-metadata", value: { retained: true } }) + "\n", { flag: "a" });
  const read = await cli.send("sessions.read", params(cli, id));
  expect(read.result.messages).toHaveLength(2);
  const children = await cli.send("sessions.children", params(cli, id, { snapshotId: read.result.snapshotId }));
  expect(children.result.records).toHaveLength(1);
  const child = await cli.send("sessions.metadata", params(cli, id, { snapshotId: read.result.snapshotId,
    subpath: children.result.records[0].subpath }));
  expect(child.result.records[0]).toMatchObject({ futureChild: { retained: true }, isSidechain: true });
  const metadata = await cli.send("sessions.metadata", params(cli, id, { snapshotId: read.result.snapshotId }));
  expect(metadata.result.records.some((record: Frame) => record.type === "future-metadata" && record.value.retained)).toBe(true);
});

test("public SDK resolves kept segments and arbitrary kept UUID order without losing boundaries or old branches", async () => {
  for (const kind of ["segment", "messages"]) {
    const { cli, id, path, records } = await fixture(3);
    const boundary: Frame = { type: "system", subtype: "compact_boundary", uuid: crypto.randomUUID(), parentUuid: null,
      logicalParentUuid: records[5]!.uuid, cwd: cli.directory, sessionId: id, timestamp: new Date().toISOString() };
    const summary: Frame = { ...records[0], uuid: crypto.randomUUID(), parentUuid: boundary.uuid,
      isCompactSummary: true, isVisibleInTranscriptOnly: true, origin: undefined, message: { role: "user", content: "summary" } };
    boundary.compactMetadata = kind === "segment"
      ? { trigger: "manual", preTokens: 100, preservedSegment: { headUuid: records[2]!.uuid, tailUuid: records[5]!.uuid, anchorUuid: summary.uuid } }
      : { trigger: "manual", preTokens: 100, preservedMessages: { uuids: [records[2]!.uuid, records[3]!.uuid, records[0]!.uuid, records[1]!.uuid], anchorUuid: summary.uuid } };
    const post: Frame = { ...records[0], uuid: crypto.randomUUID(), parentUuid: kind === "segment" ? records[5]!.uuid : records[1]!.uuid,
      message: { role: "user", content: "post compact human" } };
    const answer: Frame = { ...records[1], uuid: crypto.randomUUID(), parentUuid: post.uuid };
    const abandoned: Frame = { ...records[0], uuid: crypto.randomUUID(), parentUuid: records[1]!.uuid,
      message: { role: "user", content: "ABANDONED" } };
    await writeFile(path, [...records.slice(0, 2), records[0], abandoned, ...records.slice(2), boundary, summary, post, answer]
      .map(record => JSON.stringify(record)).join("\n") + "\n");
    const original = await readFile(path, "utf8");
    const read = await cli.send("sessions.read", params(cli, id));
    expect(read.ok).toBe(true);
    const messages = read.result.messages;
    expect(messages.map((row: Frame) => row.message.uuid)).toEqual([...records, boundary, summary, post, answer].map(row => row.uuid));
    expect(messages.filter((row: Frame) => row.message.uuid === records[0]!.uuid)).toHaveLength(1);
    for (const anchor of [records[2]!, post]) {
      const fork = await cli.send("sessions.forkBefore", params(cli, id, { beforeMessageId: anchor.uuid }));
      expect(fork.ok, JSON.stringify({kind, anchor, fork})).toBe(true);
      const copied = await cli.send("sessions.read", params(cli, fork.result.sessionId));
      expect(copied.result.messages).toHaveLength(anchor === post ? 8 : 2);
      expect(copied.result.messages.some((row: Frame) => row.message.message?.content === "post compact human")).toBe(false);
    }
    expect(await readFile(path, "utf8")).toBe(original);
  }
});

test("concurrent completed reads retain no more than two stable snapshots", async () => {
  const { cli, id } = await fixture(100);
  const initial = await cli.send("sessions.read", params(cli, id));
  const replies = await Promise.all([cli.send("sessions.read", params(cli, id)), cli.send("sessions.read", params(cli, id))]);
  const ids = [initial, ...replies].map(reply => reply.result.snapshotId);
  const accepted = await Promise.all(ids.map(snapshotId => cli.send("sessions.read", params(cli, id, { snapshotId }))));
  expect(accepted.filter(reply => reply.ok)).toHaveLength(2);
  expect(accepted.filter(reply => !reply.ok)[0]!.error.code).toBe("stale_snapshot");
});


test("SDK history projection keeps explicit raw child ownership instead of projected null parents", async () => {
  const { cli, id, path, records } = await fixture(4);
  const tags = [{ agent_id: "agent-only" }, { parent_tool_use_id: "caller-only" },
    { parent_agent_id: "nested-only" }, { parent_tool_use_id: null, parent_agent_id: null }];
  tags.forEach((tag, index) => { Object.assign(records[index * 2]!, tag); Object.assign(records[index * 2 + 1]!, tag); });
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  const read = await cli.send("sessions.read", params(cli, id));
  expect(read.ok).toBe(true);
  expect(read.result.messages).toHaveLength(8);
  tags.forEach((tag, index) => {
    expect(read.result.messages[index * 2].message).toMatchObject(tag);
    expect(read.result.messages[index * 2 + 1].message).toMatchObject(tag);
  });
  expect(read.result.messages[6].message.agent_id).toBeUndefined();
  const original = await readFile(path, "utf8");
  expect(original).toContain("nested-only");
});

test("explicit input kinds survive resume and validated fork remaps without guessing from priority or identical text", async () => {
  const { cli, id, path, records } = await fixture(5);
  await cli.start();
  const inputs = [
    { priority: "later", inputKind: "prompt" },
    { priority: "now", inputKind: "steer" },
    { priority: "later", inputKind: "prompt" },
    { priority: "now" },
  ];
  for (let index = 0; index < inputs.length; index++) {
    expect((await cli.prompt("identical é🦊", { uuid: records[index * 2]!.uuid, ...inputs[index] })).ok).toBe(true);
    delete records[index * 2]!.origin;
  }
  const rejected = await cli.prompt("identical é🦊", { uuid: records[8]!.uuid, priority: "later", inputKind: "steer" });
  expect(rejected).toMatchObject({ ok: false, error: { code: "invalid_request" } });
  for (const record of records.filter(record => record.type === "user")) record.message.content = "identical é🦊";
  await writeFile(path, records.map(record => JSON.stringify(record)).join("\n") + "\n");
  const original = await readFile(path, "utf8");
  await cli.wait(frame => frame.event === "sdk" && frame.message.type === "user" && frame.message.uuid === records[6]!.uuid);
  const nativeInputs = (await cli.cliFrames()).filter(frame => frame.type === "user");
  expect(nativeInputs).toHaveLength(4);
  expect(nativeInputs.every(frame => !("inputKind" in frame))).toBe(true);
  await cli.send("shutdown");
  await cli.exited;
  const restarted = new Peer(cli.directory, process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(restarted);
  const read = (await restarted.send("sessions.read", params(restarted, id))).result.messages;
  expect(read.filter((row: Frame) => row.message.type === "user").map((row: Frame) => row.inputKind))
    .toEqual(["prompt", "steer", "prompt", undefined, undefined]);
  expect(read.filter((row: Frame) => row.message.type === "user").every((row: Frame) => row.source === "human")).toBe(true);
  const fork = await restarted.send("sessions.forkBefore", params(restarted, id, { beforeMessageId: records[8]!.uuid }));
  expect(fork.ok).toBe(true);
  const copied = (await restarted.send("sessions.read", params(restarted, fork.result.sessionId))).result.messages;
  expect(copied).toHaveLength(8);
  expect(copied[2].message.uuid).not.toBe(records[2]!.uuid);
  expect(copied.filter((row: Frame) => row.message.type === "user").map((row: Frame) => row.inputKind))
    .toEqual(["prompt", "steer", "prompt", undefined]);
  expect(copied.every((row: Frame) => !("inputKind" in row.message))).toBe(true);
  expect(await readFile(path, "utf8")).toBe(original);
});
