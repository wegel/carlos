/** Tests the real official SDK against a deterministic CLI with no model calls. */
import { afterEach, expect, test } from "bun:test";

import { Peer, type Frame } from "./peer";

const peers: Peer[] = [];
afterEach(async () => { for (const peer of peers.splice(0)) await peer.dispose(); });
async function peer(): Promise<Peer> {
  const result = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(result);
  return result;
}
const sdkEvent = (frame: Frame) => frame.type === "event" && frame.event === "sdk";

test("real SDK starts native CLI with explicit settings and controls the same process", async () => {
  const cli = await peer();
  expect((await cli.send("hello")).result).toMatchObject({ protocolVersion: 1, sdkVersion: "0.3.322" });
  expect((await cli.start({ model: "sonnet", effort: "high", permissionMode: "plan" })).result.models[0].value).toBe("fake-model");
  expect((await cli.send("model.set", { model: "opus" })).ok).toBe(true);
  expect((await cli.send("effort.set", { effort: "max" })).ok).toBe(true);
  expect((await cli.send("context")).result).toMatchObject({ usedTokens: 123, source: "fake-cli" });
  expect((await cli.send("interrupt")).ok).toBe(true);
  const frames = await cli.cliFrames();
  expect(frames.filter((frame) => frame.argv)).toHaveLength(1);
  expect(frames[0]).toMatchObject({ cwd: cli.directory, emit_session_state: "1" });
  expect(frames[0]!.argv).toEqual(expect.arrayContaining(["--setting-sources=user,project,local", "--permission-mode=plan"]));
  expect(frames.some((frame) => frame.request?.subtype === "set_model" && frame.request.model === "opus")).toBe(true);
  expect(frames.some((frame) => frame.request?.settings?.effortLevel === "max")).toBe(true);
});

test("prompt preserves UUID, automatic origin, and priority; child events survive main result", async () => {
  const cli = await peer();
  await cli.start();
  const uuid = crypto.randomUUID();
  expect((await cli.prompt("hello", { uuid, origin: { kind: "auto-continuation" }, priority: "next" })).result.uuid).toBe(uuid);
  const replay = await cli.wait((frame) => sdkEvent(frame) && frame.message.type === "user");
  expect(replay.message).toMatchObject({ uuid, origin: { kind: "auto-continuation" }, priority: "next" });
  const child = await cli.wait((frame) => sdkEvent(frame) && frame.message.subtype === "task_notification");
  expect(child.message).toMatchObject({ task_id: "child-1", tool_use_id: "parent-tool", future_field: { retained: true } });
  const resultIndex = cli.frames.findIndex((frame) => sdkEvent(frame) && frame.message.type === "result");
  expect(cli.frames.indexOf(child)).toBeGreaterThan(resultIndex);
  expect((await cli.prompt("duplicate", { uuid })).error.code).toBe("duplicate_prompt");
});

test("two native permissions, question, and dialog remain independently answerable during controls", async () => {
  const cli = await peer();
  await cli.start({ supportedDialogKinds: ["test-dialog"] });
  await cli.prompt("callbacks");
  for (const id of ["permission-1", "permission-2", "question", "dialog"]) {
    await cli.wait((frame) => frame.type === "callback" && frame.callbackId === id);
  }
  expect((await cli.send("context")).ok).toBe(true);
  expect((await cli.send("interrupt")).ok).toBe(true);
  expect((await cli.send("callback.respond", { callbackId: "permission-1", answer: { behavior: "invalid" } })).ok).toBe(false);
  expect((await cli.send("callback.respond", { callbackId: "permission-1", answer: { behavior: "allow", updatedPermissions: [] } })).error.code).toBe("unsupported_answer");
  expect((await cli.send("callback.respond", { callbackId: "permission-2", answer: { behavior: "allow", updatedInput: { command: "echo changed" } } })).ok).toBe(true);
  expect((await cli.send("callback.respond", { callbackId: "permission-1", answer: { behavior: "deny", message: "No" } })).ok).toBe(true);
  expect((await cli.send("callback.respond", { callbackId: "question", answer: { action: "accept", content: { value: 3 } } })).ok).toBe(true);
  expect((await cli.send("callback.respond", { callbackId: "dialog", answer: { behavior: "cancelled" } })).ok).toBe(true);
  await cli.send("context"); // A CLI reply fences the preceding callback writes.
  const responses = (await cli.cliFrames()).filter((frame) => frame.type === "control_response");
  expect(responses.map((frame) => frame.response.request_id).sort()).toEqual(["dialog", "permission-1", "permission-2", "question"]);
  expect(responses.find((frame) => frame.response.request_id === "permission-2")!.response.response.updatedInput.command).toBe("echo changed");
});

test("CLI sends only declared dialogs; absent and unsupported declarations do not claim support", async () => {
  for (const supportedDialogKinds of [undefined, [], ["other-dialog"]]) {
    const cli = await peer();
    await cli.start(supportedDialogKinds === undefined ? {} : { supportedDialogKinds });
    await cli.prompt("callbacks");
    await cli.send("context");
    expect(cli.frames.filter((frame) => frame.type === "callback" && frame.kind === "dialog")).toHaveLength(0);
    const initialize = (await cli.cliFrames()).find((frame) => frame.request?.subtype === "initialize");
    expect(initialize!.request.supportedDialogKinds).toEqual(supportedDialogKinds ?? []);
  }
});

test("undeclared incoming dialog reports its ID without settling it and leaves controls responsive", async () => {
  const cli = await peer();
  await cli.start({ supportedDialogKinds: ["test-dialog"] });
  await cli.prompt("undeclared-dialog");
  expect(await cli.wait((frame) => frame.error?.code === "unsupported_dialog")).toMatchObject({
    event: "error", callbackId: "undeclared-dialog", dialogKind: "undeclared" });
  expect((await cli.send("interrupt")).ok).toBe(true);
  const response = (await cli.cliFrames()).find((frame) => frame.type === "control_response" && frame.response.request_id === "undeclared-dialog");
  expect(response!.response.subtype).toBe("error");
  expect(cli.frames.some((frame) => frame.type === "callback" && frame.callbackId === "undeclared-dialog")).toBe(false);
  expect((await cli.send("shutdown")).ok).toBe(true);
});

test("invalid dialog declarations are rejected before CLI launch", async () => {
  const cli = await peer();
  for (const supportedDialogKinds of [null, "dialog", [""], ["same", "same"], [" padded "]]) {
    expect((await cli.start({ supportedDialogKinds })).error.code).toBe("invalid_request");
  }
  expect((await cli.start()).ok).toBe(true);
});

test("concurrent duplicate ID does not release or replace the active request", async () => {
  const cli = await peer();
  await cli.start();
  const pending = cli.send("model.set", { model: "no-response" });
  await cli.wait((frame) => frame.event === "stderr" && frame.text.includes("parked"));
  cli.raw({ v: 1, id: "request-2", method: "hello", params: {} });
  await cli.wait((frame) => frame.id === "request-2" && frame.error?.code === "duplicate_id");
  cli.raw({ v: 1, id: "request-2", method: "hello", params: {} });
  await cli.send("context");
  expect(cli.frames.filter((frame) => frame.id === "request-2" && frame.error?.code === "duplicate_id")).toHaveLength(2);
  await cli.send("shutdown");
  await pending;
  expect((await cli.wait((frame) => frame.id === "request-2" && frame.error?.code === "closed")).ok).toBe(false);
});

test("bad input returns typed errors without killing the helper or starting bypass mode", async () => {
  const cli = await peer();
  cli.raw("{broken");
  expect((await cli.wait((frame) => frame.error?.code === "invalid_json")).id).toBeNull();
  cli.raw({ v: 2, id: "wrong-version", method: "hello" });
  expect((await cli.wait((frame) => frame.id === "wrong-version")).error.code).toBe("unsupported_version");
  expect((await cli.start({ permissionMode: "bypassPermissions" })).error.code).toBe("invalid_request");
  expect((await cli.start()).ok).toBe(true);
  expect((await cli.send("unknown")).error.code).toBe("unknown_method");
  expect((await cli.send("permission.set", { mode: "bypassPermissions" })).ok).toBe(false);
  expect((await cli.send("prompt", { text: "no explicit origin" })).ok).toBe(false);
  expect((await cli.send("callback.respond", { callbackId: "absent", answer: {} })).error.code).toBe("unknown_callback");
  expect((await cli.send("hello")).ok).toBe(true);
});

test("CLI EOF and CLI failure close the helper and report distinct reasons", async () => {
  for (const [text, reason] of [["eof", "sdk_eof"], ["crash", "sdk_error"]]) {
    const cli = await peer();
    await cli.start();
    await cli.prompt(text!);
    expect((await cli.wait((frame) => frame.event === "closed")).reason).toBe(reason);
    expect(await Promise.race([cli.exited, Bun.sleep(2000).then(() => "timeout")])).toBe(text === "crash" ? 1 : 0);
  }
});

test("stdin EOF and explicit shutdown terminate with pending SDK callbacks", async () => {
  for (const method of ["eof", "shutdown"]) {
    const cli = await peer();
    await cli.start();
    await cli.prompt("callbacks");
    await cli.wait((frame) => frame.type === "callback");
    if (method === "eof") cli.process.stdin.end();
    else expect((await cli.send("shutdown")).ok).toBe(true);
    expect((await cli.wait((frame) => frame.event === "closed")).reason).toBe(method === "eof" ? "stdin_eof" : "shutdown");
    expect(await Promise.race([cli.exited, Bun.sleep(2000).then(() => "timeout")])).toBe(0);
    expect(cli.frames.some((frame) => frame.event === "callback_cancelled")).toBe(true);
  }
});

test("SDK cancellation withdraws a permission and rejects a late answer", async () => {
  const cli = await peer();
  await cli.start();
  await cli.prompt("cancel-callback");
  await cli.wait((frame) => frame.event === "callback_cancelled" && frame.callbackId === "cancel-me");
  expect((await cli.send("callback.respond", { callbackId: "cancel-me", answer: { behavior: "allow" } })).error.code).toBe("unknown_callback");
});

test("shutdown settles replies during SDK startup and an unanswered control request", async () => {
  for (const duringStart of [true, false]) {
    const cli = await peer();
    if (!duringStart) await cli.start();
    const pending = duringStart
      ? cli.start({ resume: "00000000-0000-4000-8000-000000000000" })
      : cli.send("model.set", { model: "no-response" });
    await cli.wait((frame) => frame.event === "stderr" && frame.text.includes("parked"));
    expect((await cli.send("shutdown")).ok).toBe(true);
    expect((await pending).error.code).toBe("closed");
    expect(await Promise.race([cli.exited, Bun.sleep(2000).then(() => "timeout")])).toBe(0);
  }
});

test("missing executable reports an SDK error and exits without parking startup", async () => {
  const cli = await peer();
  expect((await cli.start({ executable: `${cli.directory}/missing-claude` })).ok).toBe(false);
  await cli.wait((frame) => frame.event === "closed");
  expect(await Promise.race([cli.exited, Bun.sleep(2000).then(() => "timeout")])).toBe(1);
});

test("continue delegates session choice to native CLI and conflicts with an explicit resume", async () => {
  const cli = await peer();
  expect((await cli.start({ continue: true, resume: crypto.randomUUID() })).error.code).toBe("invalid_request");
  expect((await cli.start({ continue: "yes" })).error.code).toBe("invalid_request");
  expect((await cli.start({ continue: true })).ok).toBe(true);
  expect((await cli.cliFrames())[0]!.argv).toContain("--continue");
});
