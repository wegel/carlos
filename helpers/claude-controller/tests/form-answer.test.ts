/** Checks typed forms and proves slow server patterns cannot block native controls. */
import { afterEach, expect, test } from "bun:test";

import { formIssue, validateForm } from "../src/form-answer";
import { matchesFormat } from "../src/formats";
import { validateAnswerIsolated } from "../src/answer-process";
import { Peer } from "./peer";

const peers: Peer[] = [];
afterEach(async () => { for (const peer of peers.splice(0)) await peer.dispose(); });

const schema = (rule: unknown, name = "answer") => ({ type: "object", properties: { [name]: rule }, required: [name] });

test("MCP string formats accept valid mailboxes, URIs, calendar dates and leap seconds", () => {
  for (const [format, value] of [["email", "a@b"], ["email", '"a b"@example.com'], ["uri", "urn:example:animal:ferret:nose"],
    ["uri", "mailto:a@b"], ["date", "2024-02-29"], ["date-time", "2016-12-31T23:59:60Z"], ["date-time", "2026-01-01T00:00:00+05:30"]]) {
    expect(matchesFormat(value!, format!)).toBe(true);
  }
  for (const [format, value] of [["email", "a..b@example.com"], ["email", " a@b"], ["uri", " https://example.com"],
    ["uri", "https://example.com/a b"], ["date", "2025-02-29"], ["date-time", "2026-01-01T00:00:00"], ["date-time", "2026-01-01T12:00:61Z"]]) {
    expect(matchesFormat(value!, format!)).toBe(false);
  }
});

test("typed values and titled single/multi enums retain their native values", () => {
  const requested = { type: "object", properties: {
    age: { type: "integer", minimum: 18, maximum: 100 }, subscribed: { type: "boolean" },
    color: { type: "string", oneOf: [{ const: "#f00", title: "Red" }, { const: "#0f0", title: "Green" }] },
    colors: { type: "array", minItems: 1, maxItems: 2, items: { anyOf: [{ const: "#f00", title: "Red" }, { const: "#0f0", title: "Green" }] } },
  }, required: ["age", "subscribed", "color", "colors"] };
  expect(() => validateForm(requested, { age: 30, subscribed: false, color: "#f00", colors: ["#f00", "#0f0"] })).not.toThrow();
  for (const age of ["30", 17, 101, 30.5]) {
    expect(() => validateForm(requested, { age, subscribed: false, color: "#f00", colors: ["#f00"] })).toThrow();
  }
  expect(() => validateForm(schema({ type: "array", items: { type: "string", enum: ["x"] } }), { answer: ["x", "x"] })).toThrow("duplicate");
});

test("integer answers reject lost precision while finite number fields keep native number semantics", () => {
  const rounded = JSON.parse('{"answer":9007199254740993}');
  expect(() => validateForm(schema({ type: "integer" }), rounded)).toThrow("lose precision");
  expect(() => validateForm(schema({ type: "integer" }), { answer: 9007199254740991 })).not.toThrow();
  expect(() => validateForm(schema({ type: "number" }), rounded)).not.toThrow();
});

test("unknown keywords and ambiguous or incorrectly typed enum schemas remain unsupported", () => {
  for (const rule of [{ type: "object" }, { type: "string", enum: [{ const: "x", title: "X" }] },
    { type: "string", enum: ["x"], oneOf: [{ const: "y", title: "Y" }] }, { type: "string", oneOf: ["x"] },
    { type: "string", enum: ["x"], enumNames: [] }, { type: "string", not: { const: "x" } },
    { type: "array", items: { type: "number", enum: ["x"] } }, { type: "array", items: { enum: ["x"] } },
    { type: "array", items: { type: "string", enum: ["x"], anyOf: [{ const: "y", title: "Y" }] } }]) {
    expect(formIssue(schema(rule))).toBeDefined();
  }
});

test("prototype names do not satisfy missing required or declared fields", () => {
  for (const name of ["constructor", "toString", "__proto__"]) {
    expect(() => validateForm(schema({ type: "string" }, name), {})).toThrow("required");
    expect(() => validateForm(schema({ type: "string" }, name), { [name]: "ok" })).not.toThrow();
    expect(() => validateForm({ type: "object", properties: {}, required: [name] }, {})).toThrow("known fields");
    expect(() => validateForm({ type: "object", properties: {} }, { [name]: "no" })).toThrow("Unknown field");
  }
});

test("pattern and length limits count Unicode code points and retain missing optional values", () => {
  expect(() => validateForm(schema({ type: "string", minLength: 2, maxLength: 2, pattern: "^..$" }), { answer: "é🦊" })).not.toThrow();
  expect(() => validateForm(schema({ type: "string", pattern: "[" }), { answer: "x" })).toThrow("unsupported pattern");
  expect(() => validateForm({ type: "object", properties: { optional: { type: "boolean", default: false } } }, {})).not.toThrow();
});

test("AskUserQuestion accepts reordered object keys but rejects changed questions and missing answers", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(peer);
  await peer.start();
  const question = { question: "Which?", header: "Choice", options: [{ label: "A", description: "First" }, { label: "B", description: "Second" }], multiSelect: false };
  await peer.prompt("ask:" + JSON.stringify({ questions: [question] }));
  await peer.wait(frame => frame.type === "callback" && frame.callbackId === "ask");
  const reordered = { multiSelect: false, options: question.options, header: "Choice", question: "Which?" };
  expect((await peer.send("callback.respond", { callbackId: "ask", answer: { behavior: "allow", updatedInput: { questions: [reordered], answers: {} } } })).ok).toBe(false);
  expect((await peer.send("callback.respond", { callbackId: "ask", answer: { behavior: "allow", updatedInput: { questions: [{ ...reordered, question: "Changed?" }], answers: { "Changed?": "A" } } } })).ok).toBe(false);
  expect((await peer.send("callback.respond", { callbackId: "ask", answer: { behavior: "allow", updatedInput: { questions: [reordered], answers: { "Which?": "A" } } } })).ok).toBe(true);
});

async function slowForm(): Promise<Peer> {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(peer);
  await peer.start();
  await peer.prompt("form:" + JSON.stringify({ type: "object", properties: Object.fromEntries(
    Array.from({ length: 8 }, (_, i) => [`answer${i}`, { type: "string", pattern: "^(a+)+$" }])) }));
  await peer.wait(frame => frame.type === "callback" && frame.callbackId === "form");
  return peer;
}

function slowAnswer(): Record<string, string> {
  return Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`answer${i}`, "a".repeat(30000) + "!"]));
}

test("validation process is killed at its work limit", async () => {
  const signal = new AbortController().signal;
  await expect(validateAnswerIsolated("elicitation", { requestedSchema: schema({ type: "string", pattern: "^(a+)+$" }) },
    { action: "accept", content: { answer: "a".repeat(30000) + "!" } }, signal, 100)).rejects.toMatchObject({ code: "validation_limit" });
});

test("pathological pattern leaves controls responsive and the callback answerable", async () => {
  const peer = await slowForm();
  const started = Date.now();
  const checking = peer.send("callback.respond", { callbackId: "form", answer: { action: "accept", content: slowAnswer() } });
  expect((await peer.send("context")).ok).toBe(true);
  expect(Date.now() - started).toBeLessThan(800);
  expect(["validation_limit", "invalid_answer"]).toContain((await checking).error.code);
  expect(Date.now() - started).toBeLessThan(2000);
  expect((await peer.send("callback.respond", { callbackId: "form", answer: { action: "decline" } })).ok).toBe(true);
});

test("native cancellation and shutdown terminate in-flight pattern checks without waiting for their work limit", async () => {
  for (const shutdown of [false, true]) {
    const peer = await slowForm();
    const checking = peer.send("callback.respond", { callbackId: "form", answer: { action: "accept", content: slowAnswer() } });
    const started = Date.now();
    expect((await peer.send(shutdown ? "shutdown" : "model.set", shutdown ? {} : { model: "cancel-form" })).ok).toBe(true);
    expect((await checking).ok).toBe(false);
    expect(Date.now() - started).toBeLessThan(800);
    if (!shutdown) expect((await peer.send("context")).ok).toBe(true);
  }
});

test("explicit decline can supersede an in-flight form check", async () => {
  const peer = await slowForm();
  const checking = peer.send("callback.respond", { callbackId: "form", answer: { action: "accept", content: slowAnswer() } });
  const started = Date.now();
  expect((await peer.send("callback.respond", { callbackId: "form", answer: { action: "decline" } })).ok).toBe(true);
  expect((await checking).ok).toBe(false);
  expect(Date.now() - started).toBeLessThan(800);
});
