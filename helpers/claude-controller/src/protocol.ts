/** Defines Carlos's versioned local protocol, independent of the Rust UI protocol. */
import { isAbsolute } from "node:path";

import type { Options, SDKUserMessage } from "@anthropic-ai/claude-agent-sdk";

export const VERSION = 1;
export const SDK_VERSION = "0.3.322";
export type ObjectValue = Record<string, unknown>;
export type Start = {
  executable: string;
  cwd: string;
  settingSources: NonNullable<Options["settingSources"]>;
  supportedDialogKinds: string[];
  perTaskStopAffordance?: boolean;
  model?: string;
  contextLimit?: number;
  effort?: Options["effort"];
  permissionMode?: Options["permissionMode"];
  allowDangerouslySkipPermissions?: boolean;
  resume?: string;
  sessionId?: string;
  continue?: boolean;
};
export type Prompt = {
  uuid: NonNullable<SDKUserMessage["uuid"]>;
  text: string;
  origin: { kind: "human" | "auto-continuation" | "unclassified" };
  priority: "now" | "next" | "later";
  inputKind?: "prompt" | "steer";
};
export type Request = { v: 1; id: string; method: string; params: ObjectValue };
export type Output = ObjectValue & { v: 1; type: "reply" | "event" | "callback" };

export class ProtocolError extends Error {
  constructor(readonly code: string, message: string) { super(message); }
}

export function object(value: unknown, label: string): ObjectValue {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new ProtocolError("invalid_request", `${label} must be an object`);
  }
  return value as ObjectValue;
}

export function string(value: unknown, label: string): string {
  if (typeof value !== "string" || !value.length) {
    throw new ProtocolError("invalid_request", `${label} must be a nonempty string`);
  }
  return value;
}

export function choice<T extends string>(value: unknown, values: readonly T[], label: string): T {
  if (!values.includes(value as T)) {
    throw new ProtocolError("invalid_request", `${label} must be one of: ${values.join(", ")}`);
  }
  return value as T;
}

export function parseRequest(line: string): Request {
  let parsed: unknown;
  try { parsed = JSON.parse(line); }
  catch { throw new ProtocolError("invalid_json", "Expected a JSON object on one line"); }
  const request = object(parsed, "request");
  if (request.v !== VERSION) throw new ProtocolError("unsupported_version", "Expected protocol version 1");
  return { v: 1, id: string(request.id, "id"), method: string(request.method, "method"),
    params: object(request.params ?? {}, "params") };
}

export function parseStart(params: ObjectValue, isAbsolutePath: (path: string) => boolean = isAbsolute): Start {
  const executable = string(params.executable, "executable");
  const cwd = string(params.cwd, "cwd");
  if (!isAbsolutePath(executable) || !isAbsolutePath(cwd)) {
    throw new ProtocolError("invalid_request", "executable and cwd must be absolute paths");
  }
  const settingSources = params.settingSources;
  if (!Array.isArray(settingSources)) throw new ProtocolError("invalid_request", "settingSources must be explicit");
  const result: Start = { executable, cwd,
    settingSources: settingSources.map((s) => choice(s, ["user", "project", "local"], "setting source")),
    supportedDialogKinds: parseDialogKinds(params.supportedDialogKinds === undefined ? [] : params.supportedDialogKinds) };
  if (params.perTaskStopAffordance !== undefined) {
    if (typeof params.perTaskStopAffordance !== "boolean") throw new ProtocolError("invalid_request", "perTaskStopAffordance must be boolean");
    result.perTaskStopAffordance = params.perTaskStopAffordance;
  }
  if (params.model !== undefined) result.model = string(params.model, "model");
  if (params.contextLimit !== undefined) {
    if (typeof params.contextLimit !== "number" || !Number.isSafeInteger(params.contextLimit)
      || params.contextLimit < 100000 || params.contextLimit > 1000000) {
      throw new ProtocolError("invalid_request", "Choose a context size from 100k to 1M tokens");
    }
    result.contextLimit = params.contextLimit;
  }
  if (params.sessionId !== undefined) {
    result.sessionId = string(params.sessionId, "sessionId");
    if (!/^[0-9a-f-]{36}$/i.test(result.sessionId)) throw new ProtocolError("invalid_request", "sessionId must be a UUID");
  }
  if (params.resume !== undefined) result.resume = string(params.resume, "resume");
  if (params.continue !== undefined) {
    if (typeof params.continue !== "boolean") throw new ProtocolError("invalid_request", "continue must be boolean");
    result.continue = params.continue;
  }
  if (result.sessionId && (result.resume || result.continue)) throw new ProtocolError("invalid_request", "A new sessionId cannot resume a session");
  if (result.resume && result.continue) throw new ProtocolError("invalid_request", "Choose resume or continue");
  if (params.effort !== undefined) result.effort = parseEffort(params.effort);
  if (params.permissionMode !== undefined) result.permissionMode = parseMode(params.permissionMode);
  if (params.allowDangerouslySkipPermissions !== undefined) {
    if (typeof params.allowDangerouslySkipPermissions !== "boolean") {
      throw new ProtocolError("invalid_request", "allowDangerouslySkipPermissions must be a boolean");
    }
    result.allowDangerouslySkipPermissions = params.allowDangerouslySkipPermissions;
  }
  if (result.permissionMode === "bypassPermissions" && !result.allowDangerouslySkipPermissions) {
    throw new ProtocolError("invalid_request", "bypassPermissions requires explicit allowDangerouslySkipPermissions: true");
  }
  return result;
}

export function parsePrompt(params: ObjectValue): Prompt {
  const uuid = string(params.uuid, "uuid");
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(uuid)) {
    throw new ProtocolError("invalid_request", "uuid must be a UUID");
  }
  const prompt: Prompt = { uuid: uuid as Prompt["uuid"], text: string(params.text, "text"),
    priority: choice(params.priority, ["now", "next", "later"], "priority"),
    origin: { kind: choice(object(params.origin, "origin").kind,
      ["human", "auto-continuation", "unclassified"], "origin.kind") } };
  if (params.inputKind !== undefined) {
    prompt.inputKind = choice<"prompt" | "steer">(params.inputKind, ["prompt", "steer"], "inputKind");
    if (prompt.origin.kind !== "human" || (prompt.inputKind === "steer" && prompt.priority !== "now")) {
      throw new ProtocolError("invalid_request", "Only a human now input can be classified as a steer");
    }
  }
  return prompt;
}

export function parseEffort(value: unknown): NonNullable<Options["effort"]> {
  return choice(value, ["low", "medium", "high", "xhigh", "max"], "effort");
}

export function parseMode(value: unknown): NonNullable<Options["permissionMode"]> {
  return choice(value, ["default", "acceptEdits", "bypassPermissions", "plan", "dontAsk", "auto"], "permissionMode");
}

function parseDialogKinds(value: unknown): string[] {
  if (!Array.isArray(value) || value.length > 128) {
    throw new ProtocolError("invalid_request", "supportedDialogKinds must be an array of at most 128 names");
  }
  const kinds = value.map((kind) => {
    const name = string(kind, "dialog kind");
    if (name.length > 128 || name.trim() !== name) {
      throw new ProtocolError("invalid_request", "Dialog kind names must be trimmed and at most 128 characters");
    }
    return name;
  });
  if (new Set(kinds).size !== kinds.length) throw new ProtocolError("invalid_request", "Dialog kinds must be unique");
  return kinds;
}
