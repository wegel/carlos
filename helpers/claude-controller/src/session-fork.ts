/** Forks before an exact public message anchor and carries Carlos-owned source records across UUID changes. */
import { createHash, randomUUID } from "node:crypto";

import { ProtocolError, type ObjectValue } from "./protocol";
import type { InputKind, PromptSource, PromptSources, SourceRecord } from "./prompt-sources";
import type { SessionArchive } from "./session-archive";
import type { ForkCut } from "./session-fork-tail";
import type { SessionApi } from "./sessions";

type Anchor = { uuid: string; fingerprint: string; source: PromptSource; inputKind?: InputKind };

export async function forkBefore(api: SessionApi, sources: PromptSources, signal: AbortSignal,
  dir: string, sessionId: string, beforeId: string, archive: (dir: string, id: string) => Promise<SessionArchive>): Promise<unknown> {
  const deadline = Date.now() + 5000;
  let anchor: { prefix: Anchor[]; cut?: ForkCut } | undefined;
  while (!anchor) {
    signal.throwIfAborted();
    anchor = findAnchor(await archive(dir, sessionId), sources, beforeId);
    if (anchor) break;
    if (Date.now() >= deadline) throw new ProtocolError("anchor_unavailable",
      "The selected prompt has not reached native session history; the original session was kept");
    await new Promise<void>((resolve, reject) => {
      const abort = () => { clearTimeout(timer); reject(signal.reason); };
      const timer = setTimeout(() => { signal.removeEventListener("abort", abort); resolve(); }, 50);
      signal.addEventListener("abort", abort, { once: true });
    });
  }
  const { prefix, cut } = anchor;
  if (!prefix.length) return { sessionId: randomUUID(), fresh: true };
  signal.throwIfAborted();
  const fork = await api.forkSession(sessionId, { dir, upToMessageId: prefix.at(-1)!.uuid });
  const forked = await archive(dir, fork.sessionId);
  const entries = forked.forkEntries(cut!);
  sources.saveAll(validate(entries, prefix, sessionId));
  return { ...fork, fresh: false };
}

function validate(entries: (offset: number, limit: number) => ObjectValue[], prefix: Anchor[], sessionId: string): SourceRecord[] {
  const saved: SourceRecord[] = [];
  for (let offset = 0; offset < prefix.length; offset += 64) {
    const messages = entries(offset, 64);
    for (let index = 0; index < Math.min(messages.length, prefix.length - offset); index++) {
      const message = messages[index]! as unknown as ObjectValue;
      const original = prefix[offset + index]!;
      const mapped = message.forkedFrom as ObjectValue | undefined;
      if (mapped?.sessionId !== sessionId || mapped.messageUuid !== original.uuid) {
        throw new ProtocolError("fork_mapping_failed", "The SDK fork refers to a different source record");
      }
      if (fingerprint(message) !== original.fingerprint) throw new ProtocolError("fork_mapping_failed",
        "The SDK changed the forked history unexpectedly; the original session was kept");
      if (original.source !== "system") saved.push({ uuid: String(message.uuid), source: original.source, inputKind: original.inputKind });
    }
    if (messages.length < Math.min(64, prefix.length - offset)) throw new ProtocolError("fork_mapping_failed", "Forked history is incomplete");
  }
  if (entries(prefix.length, 1).length !== 0) throw new ProtocolError("fork_mapping_failed",
    "The SDK fork retained messages beyond the selected cut; the original session was kept");
  return saved;
}

function findAnchor(archive: SessionArchive, sources: PromptSources, beforeId: string): { prefix: Anchor[]; cut?: ForkCut } | undefined {
  const prefix: Anchor[] = [];
  for (let offset = 0; ; offset += 64) {
    const messages = archive.entries(offset, 64, true);
    for (const message of messages) {
      if (message.uuid === beforeId) {
        if (!["human", "unknown"].includes(sources.source(message))) throw new ProtocolError("invalid_anchor", "Select a main-thread prompt to edit");
        return { prefix, cut: prefix.length ? archive.forkCut(prefix.at(-1)!.uuid) : undefined };
      }
      if (!["user", "assistant", "system"].includes(String(message.type))) continue;
      if (prefix.length >= 100000) throw new ProtocolError("history_too_large", "The native history exceeds the safe fork index limit");
      const source = sources.source(message);
      prefix.push({ uuid: String(message.uuid), fingerprint: fingerprint(message), source,
        inputKind: source === "human" ? sources.inputKind(String(message.uuid)) : undefined });
    }
    if (messages.length < 64) return undefined;
  }
}

function fingerprint(message: ObjectValue): string {
  return createHash("sha256").update(canonical([message.type, message.message,
    message.parent_tool_use_id ?? null, message.parent_agent_id ?? null])).digest("hex");
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b))
    .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(",")}}`;
  return JSON.stringify(value) ?? "null";
}
