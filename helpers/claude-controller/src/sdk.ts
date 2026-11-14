/** Connects the controller to the official SDK and the explicitly selected native CLI. */
import { query, type Options, type Query, type SDKUserMessage } from "@anthropic-ai/claude-agent-sdk";

import { Callbacks } from "./callbacks";
import { InputQueue } from "./input-queue";
import { formIssue } from "./form-answer";
import { NativeOutput } from "./native-output";
import { ProtocolError, type Output, type Start } from "./protocol";

export type QueryFactory = typeof query;

export function startQuery(start: Start, input: InputQueue<SDKUserMessage>,
  callbacks: Callbacks, emit: (output: Output) => void, native: NativeOutput, factory: QueryFactory): Query {
  const stderr = (text: string) => emit({ v: 1, type: "event", event: "stderr", text });
  const options: Options = {
    pathToClaudeCodeExecutable: start.executable,
    cwd: start.cwd,
    settingSources: start.settingSources,
    systemPrompt: { type: "preset", preset: "claude_code" },
    includePartialMessages: true,
    ...(start.contextLimit === undefined ? {} : { settings: { autoCompactWindow: start.contextLimit } }),
    env: { ...process.env, CLAUDE_CODE_EMIT_SESSION_STATE_EVENTS: "1" },
    stderr,
    spawnClaudeCodeProcess: (options) => native.spawn(options, stderr),
    canUseTool: (toolName, input, { signal, requestId, ...metadata }) =>
      callbacks.ask("permission", requestId, { toolName, input, ...metadata }, signal),
    onElicitation: (request, { signal, requestId }) => callbacks.ask("elicitation", requestId,
      { ...request, ...(request.mode === "url" ? { urlHost: urlHost(request.url) } : { formIssue: formIssue(request.requestedSchema) }) }, signal),
    supportedDialogKinds: start.supportedDialogKinds,
    onUserDialog: async (request, { signal, requestId }) => {
      if (!start.supportedDialogKinds.includes(request.dialogKind)) {
        emit({ v: 1, type: "event", event: "error", callbackId: requestId, dialogKind: request.dialogKind,
          error: { code: "unsupported_dialog", message: "The CLI requested a dialog this host did not declare" } });
        throw new ProtocolError("unsupported_dialog", `Undeclared dialog kind ${request.dialogKind}`);
      }
      return callbacks.ask("dialog", requestId, request, signal);
    },
  };
  // Undefined selections preserve the native CLI's settings and defaults.
  for (const key of ["model", "effort", "permissionMode", "allowDangerouslySkipPermissions", "resume", "continue", "sessionId", "perTaskStopAffordance"] as const) {
    if (start[key] !== undefined) Object.assign(options, { [key]: start[key] });
  }
  return factory({ prompt: input, options });
}

export const officialQuery = query;

function urlHost(value: string | undefined): string | undefined {
  try { return new URL(value ?? "").host; } catch { return undefined; }
}
