/** Proves an SDK-hidden system suffix without treating abandoned raw entries as conversation history. */
import { ProtocolError, type ObjectValue } from "./protocol";

/** An opaque SDK-imported record and its order in the bounded cache. */
export type IndexedEntry = { entry: ObjectValue; seq: number };
type Lookup = (uuid: string) => IndexedEntry | undefined;
/** Keep the exact cut's parent proof alive even when another read evicts its archive. */
export type ForkCut = { sessionId: string; uuid: string; records: ReadonlyMap<string, IndexedEntry> };

/** Capture only the cut-to-message parent chain before awaiting the SDK mutation. */
export function captureCut(sessionId: string, uuid: string, find: Lookup): ForkCut {
  const records = new Map<string, IndexedEntry>();
  let current = find(uuid);
  while (current) {
    const id = current.entry.uuid;
    if (typeof id !== "string" || records.has(id) || records.size >= 100000) fail("The source cut has invalid ancestry");
    records.set(id, current);
    if (["user", "assistant"].includes(String(current.entry.type))) break;
    current = typeof current.entry.parentUuid === "string" ? find(current.entry.parentUuid) : undefined;
  }
  return { sessionId, uuid, records };
}

/** Return only mapped system rows that connect the copied cut to the canonical leaf. */
export function forkTail(candidates: IndexedEntry[], leaf: string, sessionId: string,
  forkRecord: Lookup, sourceRecord: Lookup, contains: (uuid: string) => boolean = uuid => uuid === leaf): ObjectValue[] {
  if (candidates.length !== 1) fail("The SDK fork did not preserve one exact cut record");
  const tip = candidates[0]!;
  if (contains(String(tip.entry.uuid))) return [];
  const tail: ObjectValue[] = [];
  const seen = new Set<string>();
  let current: IndexedEntry | undefined = tip;
  let nextSequence = Number.MAX_SAFE_INTEGER;
  while (current && current.entry.uuid !== leaf) {
    const record: ObjectValue = current.entry;
    const uuid = record.uuid;
    if (typeof uuid !== "string" || seen.has(uuid) || seen.size >= 100000 || current.seq >= nextSequence) {
      fail("The SDK fork's hidden suffix has invalid order or ancestry");
    }
    seen.add(uuid);
    nextSequence = current.seq;
    const mapped = record.forkedFrom as ObjectValue | undefined;
    if (mapped?.sessionId !== sessionId || typeof mapped.messageUuid !== "string") {
      fail("The SDK fork's hidden suffix refers to a different source record");
    }
    const original = sourceRecord(mapped.messageUuid)?.entry;
    if (!original || record.type !== original.type || record.isSidechain === true || record.agent_id ||
      record.parent_tool_use_id || record.parent_agent_id || !["system", "attachment"].includes(String(record.type))) {
      fail("The SDK fork's hidden suffix is not a main-thread system suffix");
    }
    const parent = typeof record.parentUuid === "string" ? forkRecord(record.parentUuid)?.entry : undefined;
    const sourceParent = transparentParent(original.parentUuid, sourceRecord);
    const parentMapping = parent?.forkedFrom as ObjectValue | undefined;
    if (parentMapping?.sessionId !== sessionId || parentMapping.messageUuid !== sourceParent) {
      fail("The SDK fork's hidden suffix changed its source parent");
    }
    if (systemPayload(record, forkRecord, sourceRecord) !== systemPayload(original)) {
      fail("The SDK changed the forked system or attachment payload");
    }
    if (record.type === "system") tail.push(record);
    current = typeof record.parentUuid === "string" ? forkRecord(record.parentUuid) : undefined;
  }
  if (!current) fail("The SDK fork's hidden suffix does not reach its canonical leaf");
  return tail.reverse();
}

function transparentParent(uuid: unknown, sourceRecord: Lookup): unknown {
  const seen = new Set<string>();
  while (typeof uuid === "string") {
    const record = sourceRecord(uuid)?.entry;
    if (record?.type !== "progress") return uuid;
    if (seen.has(uuid) || seen.size >= 100000) fail("The source cut has invalid progress ancestry");
    seen.add(uuid);
    uuid = record.parentUuid;
  }
  return uuid;
}

/** Compare opaque payloads while accounting for the SDK's named identity changes. */
export function systemPayload(record: ObjectValue, forkRecord?: Lookup, sourceRecord?: Lookup): string {
  const copy = { ...record };
  copy.isSidechain ??= false;
  // SDK 0.3.322 resets these identity/attribution fields when copying a fork.
  for (const key of ["uuid", "parentUuid", "logicalParentUuid", "sessionId", "session_id", "timestamp", "forkedFrom",
    "slug", "teamName", "agentName", "sessionKind", "sourceToolAssistantUUID", "summarizedTurnPromptUuids",
    "droppedPromptUuids", "turnAttributionKey"]) {
    delete copy[key];
  }
  if (forkRecord) {
    const sourceId = (value: unknown) => {
      if (typeof value !== "string") return value;
      const mapped = forkRecord(value)?.entry.forkedFrom as ObjectValue | undefined;
      if (mapped) {
        if (mapped.sessionId !== (record.forkedFrom as ObjectValue)?.sessionId) fail("The SDK fork has a foreign payload reference");
        return mapped.messageUuid;
      }
      if (sourceRecord?.(value)) fail("The SDK fork did not remap an owned payload reference");
      return value;
    };
    if (copy.compactMetadata && typeof copy.compactMetadata === "object") {
      const metadata = structuredClone(copy.compactMetadata) as ObjectValue;
      for (const key of ["preservedMessages", "preservedSegment"]) {
        const part = metadata[key] as ObjectValue | undefined;
        if (!part) continue;
        for (const name of ["anchorUuid", "headUuid", "tailUuid"]) if (name in part) part[name] = sourceId(part[name]);
        for (const name of ["uuids", "allUuids"]) if (Array.isArray(part[name])) part[name] = part[name].map(sourceId);
      }
      copy.compactMetadata = metadata;
    }
    if (copy.attachment && typeof copy.attachment === "object") {
      const attachment = { ...copy.attachment as ObjectValue };
      if (attachment.type === "queued_command" && "source_uuid" in attachment) attachment.source_uuid = sourceId(attachment.source_uuid);
      if (attachment.type === "deferred_tools_record" && Array.isArray(attachment.nameOnlyAnnouncements)) {
        attachment.nameOnlyAnnouncements = attachment.nameOnlyAnnouncements.map(sourceId);
      }
      copy.attachment = attachment;
    }
  }
  return canonical(copy);
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b))
    .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(",")}}`;
  return JSON.stringify(value) ?? "null";
}

function fail(message: string): never { throw new ProtocolError("fork_mapping_failed", message); }
