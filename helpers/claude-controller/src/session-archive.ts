/** Projects SDK-imported opaque entries read-only; native Claude files remain SDK-owned. */
import { Database } from "bun:sqlite";
import { importSessionToStore, type SessionStoreEntry } from "@anthropic-ai/claude-agent-sdk";
import { randomUUID } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ProtocolError, type ObjectValue } from "./protocol";
import type { SessionApi } from "./sessions";

type Row = { data: string; uuid: string | null };

export class SessionArchive {
  readonly id = randomUUID();
  private folder = mkdtempSync(join(tmpdir(), "carlos-history-"));
  private database = new Database(join(this.folder, "entries.sqlite"));
  private bytes = 0;
  private projectionBytes = 0;

  constructor(readonly sessionId: string, readonly cwd: string) {
    this.database.exec(`CREATE TABLE entries(seq INTEGER PRIMARY KEY, subpath TEXT NOT NULL, uuid TEXT,
      data TEXT NOT NULL, UNIQUE(subpath,uuid)); CREATE TABLE branch(position INTEGER PRIMARY KEY, uuid TEXT NOT NULL)`);
  }

  async read(api: SessionApi): Promise<void> {
    await importSessionToStore(this.sessionId, { load: async () => null,
      append: async (key, entries) => this.append(key.subpath ?? "", entries) },
      { dir: this.cwd, includeSubagents: true, batchSize: 64 });
    const seen = new Set<string>();
    const expanded = await this.expand(api, undefined, new Set<number>());
    for (const message of expanded) {
      if (seen.has(String(message.uuid))) continue;
      seen.add(String(message.uuid));
      if (seen.size > 100000) throw new ProtocolError("history_too_large", "Native history exceeds 100000 visible entries");
      this.database.query("UPDATE entries SET data=? WHERE subpath='' AND uuid=?")
        .run(JSON.stringify({ ...this.entry(String(message.uuid))?.entry, ...message,
          origin: this.entry(String(message.uuid))?.entry.origin ?? message.origin }), String(message.uuid));
      this.database.query("INSERT INTO branch VALUES (?, ?)").run(seen.size, String(message.uuid));
    }
  }

  private async expand(api: SessionApi, cutoff: number | undefined, cuts: Set<number>): Promise<ObjectValue[]> {
    const messages = await api.getSessionMessages(this.sessionId, { dir: this.cwd, includeSystemMessages: true,
      sessionStore: { append: async () => { throw new Error("History cache is read-only"); },
        load: async () => (this.database.query("SELECT data FROM entries WHERE subpath='' AND seq<=? ORDER BY seq")
          .all(cutoff ?? Number.MAX_SAFE_INTEGER) as Row[]).map(row => {
            this.projectionBytes += Buffer.byteLength(row.data);
            if (this.projectionBytes > 512 * 1024 * 1024) throw new ProtocolError("history_too_large", "Native history exceeds the 512 MiB projection budget");
            return JSON.parse(row.data);
          }) } });
    if (messages.length > 100000) throw new ProtocolError("history_too_large", "Native history exceeds 100000 visible entries");
    for (const message of messages) {
      const raw = this.entry(message.uuid)?.entry;
      if (raw?.subtype !== "compact_boundary" || typeof raw.logicalParentUuid !== "string") continue;
      const parent = this.entry(raw.logicalParentUuid);
      if (!parent) throw new ProtocolError("history_changed", "The earlier compacted history is unavailable");
      if (cuts.has(parent.seq) || cuts.size >= 256 || (cutoff !== undefined && parent.seq >= cutoff)) {
        throw new ProtocolError("invalid_history", "Native compaction history contains an invalid ancestry link");
      }
      cuts.add(parent.seq);
      return [...await this.expand(api, parent.seq, cuts), ...messages as unknown as ObjectValue[]];
    }
    return messages as unknown as ObjectValue[];
  }

  private entry(uuid: string): { entry: ObjectValue; seq: number } | undefined {
    const row = this.database.query("SELECT data,seq FROM entries WHERE subpath='' AND uuid=?").get(uuid) as
      { data: string; seq: number } | null;
    return row ? { entry: JSON.parse(row.data), seq: row.seq } : undefined;
  }

  entries(offset: number, limit: number, conversationOnly = false): ObjectValue[] {
    return (this.database.query(`SELECT e.data FROM branch b JOIN entries e ON e.uuid=b.uuid AND e.subpath=''
      ${conversationOnly ? "WHERE json_extract(e.data, '$.type') IN ('user','assistant','system')" : ""}
      ORDER BY b.position ASC LIMIT ? OFFSET ?`).all(limit, offset) as Row[]).map(row => JSON.parse(row.data));
  }

  metadata(offset: number, limit: number, subpath = ""): ObjectValue[] {
    return (this.database.query(`SELECT data FROM entries WHERE subpath=? AND
      (? != '' OR uuid IS NULL) ORDER BY seq LIMIT ? OFFSET ?`).all(subpath, subpath, limit, offset) as Row[])
      .map(row => JSON.parse(row.data));
  }

  children(offset: number, limit: number): ObjectValue[] {
    return this.database.query(`SELECT subpath, count(*) AS records FROM entries WHERE subpath != ''
      GROUP BY subpath ORDER BY subpath LIMIT ? OFFSET ?`).all(limit, offset) as ObjectValue[];
  }

  close(): void { this.database.close(); rmSync(this.folder, { recursive: true, force: true }); }

  private append(subpath: string, entries: SessionStoreEntry[]): void {
    const insert = this.database.query(`INSERT INTO entries(subpath, uuid, data) VALUES (?, ?, ?)
      ON CONFLICT(subpath,uuid) DO UPDATE SET data=excluded.data`);
    this.database.transaction(() => {
      for (const entry of entries) {
        const data = JSON.stringify({ ...entry,
          session_id: this.sessionId, parent_tool_use_id: entry.parent_tool_use_id ?? null,
          parent_agent_id: entry.parent_agent_id ?? (subpath || null) });
        this.bytes += Buffer.byteLength(data);
        if (this.bytes > 256 * 1024 * 1024) throw new ProtocolError("history_too_large", "Native history exceeds the 256 MiB read cache limit");
        insert.run(subpath, entry.uuid ?? null, data);
      }
    })();
  }
}
