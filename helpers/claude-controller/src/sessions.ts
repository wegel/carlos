/** Calls published SDK session functions and returns bounded pages without rewriting Claude files. */
import { forkSession, getSessionInfo, getSessionMessages, listSessions, renameSession } from "@anthropic-ai/claude-agent-sdk";

import { ProtocolError, string, type ObjectValue } from "./protocol";
import { PromptSources } from "./prompt-sources";
import { SessionArchive } from "./session-archive";
import { forkBefore } from "./session-fork";

export const sessionApi = { forkSession, getSessionInfo, getSessionMessages, listSessions, renameSession };
export type SessionApi = typeof sessionApi;
const PAGE_BYTES = 512 * 1024;

export class Sessions {
  readonly sources = new PromptSources();
  private archives = new Map<string, SessionArchive>();
  private reading = 0;
  constructor(private signal: AbortSignal, private api: SessionApi = sessionApi) {}

  async dispatch(method: string, params: ObjectValue): Promise<unknown> {
    const dir = string(params.cwd, "cwd");
    const offset = integer(params.offset ?? 0, "offset", 0, Number.MAX_SAFE_INTEGER);
    const limit = integer(params.limit ?? 64, "limit", 1, 64);
    if (method === "sessions.list") {
      const sessions = await this.api.listSessions({ dir, offset, limit,
        includeProgrammatic: true, includeWorktrees: params.includeWorktrees !== false });
      return page(sessions, offset, limit, "sessions");
    }
    const sessionId = string(params.sessionId, "sessionId");
    if (method === "sessions.info") return await this.api.getSessionInfo(sessionId, { dir }) ?? null;
    if (method === "sessions.rename") {
      const title = string(params.title, "title").trim();
      if (!title || title.length > 4096) throw new ProtocolError("invalid_request", "Title must contain 1 to 4096 characters");
      await this.api.renameSession(sessionId, title, { dir });
      return await this.api.getSessionInfo(sessionId, { dir }) ?? null;
    }
    if (method === "sessions.children" || method === "sessions.metadata") {
      const archive = await this.archive(dir, sessionId, params.snapshotId);
      const values = method === "sessions.children" ? archive.children(offset, limit)
        : archive.metadata(offset, limit, params.subpath === undefined ? "" : string(params.subpath, "subpath"));
      return { ...page(values, offset, limit, "records", integer(params.chunkOffset ?? 0, "chunkOffset", 0, 16 * 1024 * 1024)),
        snapshotId: archive.id };
    }
    if (method === "sessions.read") {
      const archive = await this.archive(dir, sessionId, params.snapshotId);
      const messages = archive.entries(offset, limit);
      return { ...page(messages.map(message => {
        const source = this.sources.source(message);
        return { message, source, inputKind: source === "human" ? this.sources.inputKind(String(message.uuid)) : undefined };
      }),
        offset, limit, "messages", integer(params.chunkOffset ?? 0, "chunkOffset", 0, 16 * 1024 * 1024)), snapshotId: archive.id };
    }
    if (method === "sessions.forkBefore") return forkBefore(this.api, this.sources, this.signal, dir, sessionId,
      string(params.beforeMessageId, "beforeMessageId"), (dir, id) => this.archive(dir, id));
    throw new ProtocolError("unknown_method", `Unknown method ${method}`);
  }
  close(): void {
    for (const archive of this.archives.values()) archive.close();
    this.archives.clear();
    this.sources.close();
  }

  private async archive(dir: string, sessionId: string, snapshotId?: unknown): Promise<SessionArchive> {
    if (snapshotId !== undefined) {
      const archive = this.archives.get(string(snapshotId, "snapshotId"));
      if (!archive || archive.sessionId !== sessionId || archive.cwd !== dir) {
        throw new ProtocolError("stale_snapshot", "The history page belongs to a different or expired read");
      }
      return archive;
    }
    if (this.archives.size >= 2) {
      const oldest = this.archives.keys().next().value!;
      this.archives.get(oldest)!.close();
      this.archives.delete(oldest);
    }
    if (this.reading >= 2) throw new ProtocolError("session_busy", "Two history snapshots are already being read");
    this.reading++;
    const archive = new SessionArchive(sessionId, dir);
    try { await archive.read(this.api); this.signal.throwIfAborted(); }
    catch (error) { archive.close(); throw error; }
    finally { this.reading--; }
    while (this.archives.size >= 2) {
      const oldest = this.archives.keys().next().value!;
      this.archives.get(oldest)!.close();
      this.archives.delete(oldest);
    }
    this.archives.set(archive.id, archive);
    return archive;
  }
}

function integer(value: unknown, label: string, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new ProtocolError("invalid_request", `${label} must be an integer between ${min} and ${max}`);
  }
  return value;
}

function page<T>(entries: T[], offset: number, limit: number, key: string, chunkOffset = 0): ObjectValue {
  const accepted: T[] = [];
  let bytes = 0;
  for (const entry of entries) {
    const encoded = Buffer.from(JSON.stringify(entry));
    const size = encoded.byteLength + 1;
    if (size > PAGE_BYTES && accepted.length === 0) {
      if (size > 16 * 1024 * 1024) throw new ProtocolError("history_record_too_large", "A native history record exceeds the 16 MiB read limit");
      if (chunkOffset >= encoded.length) throw new ProtocolError("invalid_request", "History chunk offset exceeds the record");
      const end = Math.min(encoded.length, chunkOffset + PAGE_BYTES);
      return { [key]: [], offset, nextOffset: end === encoded.length ? offset + 1 : offset,
        chunk: { offset: chunkOffset, totalBytes: encoded.length, data: encoded.subarray(chunkOffset, end).toString("base64") },
        chunkOffset: end === encoded.length ? null : end };
    }
    if (bytes + size > PAGE_BYTES) break;
    accepted.push(entry);
    bytes += size;
  }
  return { [key]: accepted, offset, nextOffset: accepted.length < entries.length || entries.length === limit
    ? offset + accepted.length : null };
}
