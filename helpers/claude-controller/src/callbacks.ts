/** Parks independent SDK questions while the controller continues reading commands. */
import type { ElicitationResult, PermissionResult, UserDialogResult } from "@anthropic-ai/claude-agent-sdk";

import { choice, object, ProtocolError, string, type ObjectValue, type Output } from "./protocol";
import { validateAnswerIsolated } from "./answer-process";

type Kind = "permission" | "elicitation" | "dialog";
type Answer = PermissionResult | ElicitationResult | UserDialogResult;
type Pending = { kind: Kind; payload: ObjectValue; validation: AbortController; answering: boolean;
  resolve: (answer: Answer) => void; dispose: () => void };

export class Callbacks {
  private pending = new Map<string, Pending>();
  private closed = false;

  constructor(private emit: (output: Output) => void) {}

  ask<T extends Answer>(kind: Kind, id: string, payload: ObjectValue, signal: AbortSignal): Promise<T> {
    if (this.pending.has(id)) throw new ProtocolError("duplicate_callback", `Callback ${id} is already pending`);
    if (this.closed || signal.aborted) return Promise.resolve(cancelAnswer(kind) as T);
    return new Promise<T>((resolve) => {
      const abort = () => this.cancel(id, "sdk_cancelled");
      this.pending.set(id, { kind, payload, validation: new AbortController(), answering: false, resolve: (answer) => resolve(answer as T),
        dispose: () => signal.removeEventListener("abort", abort) });
      signal.addEventListener("abort", abort, { once: true });
      this.emit({ v: 1, type: "callback", callbackId: id, kind, payload });
    });
  }

  async answer(params: ObjectValue): Promise<void> {
    const id = string(params.callbackId, "callbackId");
    const pending = this.pending.get(id);
    if (!pending) throw new ProtocolError("unknown_callback", `No pending callback ${id}`);
    const answer = validateAnswer(pending.kind, params.answer);
    if (pending.answering) {
      if (answer.behavior === "deny" || answer.behavior === "cancelled" || answer.action === "decline" || answer.action === "cancel") {
        pending.validation.abort();
      } else { throw new ProtocolError("answer_pending", "This answer is already being checked"); }
    }
    pending.answering = true;
    try {
      if ((pending.kind === "permission" && pending.payload.toolName === "AskUserQuestion" && answer.behavior === "allow")
        || (pending.kind === "elicitation" && answer.action === "accept")) {
        await validateAnswerIsolated(pending.kind, pending.payload, answer as ObjectValue, pending.validation.signal);
      }
    } finally { pending.answering = false; }
    if (this.pending.get(id) !== pending) throw new ProtocolError("unknown_callback", "This request was cancelled");
    this.pending.delete(id);
    pending.dispose();
    pending.validation.abort();
    pending.resolve(answer);
  }

  cancelAll(reason: string): void {
    this.closed = true;
    for (const id of this.pending.keys()) this.cancel(id, reason);
  }

  private cancel(id: string, reason: string): void {
    const pending = this.pending.get(id);
    if (!pending) return;
    this.pending.delete(id);
    pending.dispose();
    pending.validation.abort();
    pending.resolve(cancelAnswer(pending.kind));
    this.emit({ v: 1, type: "event", event: "callback_cancelled", callbackId: id, reason });
  }
}

function cancelAnswer(kind: Kind): Answer {
  if (kind === "permission") return { behavior: "deny", message: "Carlos cancelled this request" };
  if (kind === "elicitation") return { action: "cancel" };
  return { behavior: "cancelled" };
}

function validateAnswer(kind: Kind, value: unknown): Answer {
  const answer = object(value, "answer");
  if (kind === "permission") return validatePermission(answer);
  if (kind === "elicitation") {
    choice(answer.action, ["accept", "decline", "cancel"], "answer.action");
    if (answer.content !== undefined) object(answer.content, "answer.content");
    if (answer.action !== "accept" && answer.content !== undefined) {
      throw new ProtocolError("invalid_request", "Decline and cancel must not include form content");
    }
  } else {
    choice(answer.behavior, ["completed", "cancelled"], "answer.behavior");
    if (answer.behavior === "completed" && !("result" in answer)) {
      throw new ProtocolError("invalid_request", "Completed dialog requires result");
    }
  }
  return answer as Answer;
}

function validatePermission(answer: ObjectValue): PermissionResult {
  choice(answer.behavior, ["allow", "deny"], "answer.behavior");
  if (answer.behavior === "deny") string(answer.message, "answer.message");
  if (answer.interrupt !== undefined && typeof answer.interrupt !== "boolean") {
    throw new ProtocolError("invalid_request", "answer.interrupt must be a boolean");
  }
  if (answer.updatedInput !== undefined) object(answer.updatedInput, "answer.updatedInput");
  // Rule writes need their own explicit UI and validator before Carlos exposes them.
  if (answer.updatedPermissions !== undefined) {
    throw new ProtocolError("unsupported_answer", "Persistent permission updates are not supported yet");
  }
  return answer as PermissionResult;
}
