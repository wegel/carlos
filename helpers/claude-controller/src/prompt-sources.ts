/** Stores Carlos-owned prompt authorship outside native Claude transcripts. */
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

import type { ObjectValue, Prompt } from "./protocol";

export type InputKind = "prompt" | "steer";
export type PromptSource = "human" | "automatic" | "unknown" | "system";
/** Facts to publish together after every fork record has passed validation. */
export type SourceRecord = { uuid: string; source: PromptSource; inputKind?: InputKind };

export class PromptSources {
  private database?: Database;
  private closed = false;

  remember(uuid: string, kind: Prompt["origin"]["kind"], inputKind?: InputKind): void {
    this.save(uuid, kind === "human" ? "human" : kind === "auto-continuation" ? "automatic" : "unknown", inputKind);
  }

  source(message: ObjectValue): PromptSource {
    if (message.type !== "user" || message.parent_tool_use_id || message.parent_agent_id ||
      message.is_meta === true || message.isMeta === true || message.isCompactSummary === true || message.isCompletedLocalCommand === true) return "system";
    if (message.turnOrigin === "human") return "human";
    const origin = message.origin as ObjectValue | undefined;
    if (origin?.kind === "human") return "human";
    if (origin?.kind && origin.kind !== "unclassified") return "automatic";
    return (this.open().query("SELECT source FROM prompts WHERE uuid = ?").get(String(message.uuid)) as
      { source: PromptSource } | null)?.source ?? "unknown";
  }

  inputKind(uuid: string): InputKind | undefined {
    const value = (this.open().query("SELECT kind FROM prompt_inputs WHERE uuid = ?").get(uuid) as { kind: string } | null)?.kind;
    return value === "prompt" || value === "steer" ? value : undefined;
  }

  save(uuid: string, source: PromptSource, inputKind?: InputKind): void {
    this.saveAll([{ uuid, source, inputKind }]);
  }

  /** Publish a validated bounded batch atomically, including captured input kinds. */
  saveAll(records: SourceRecord[]): void {
    if (records.length > 100000) throw new Error("Too many prompt source records");
    this.open().transaction(() => {
      for (const { uuid, source, inputKind } of records) {
        this.open().query("INSERT OR REPLACE INTO prompts(uuid, source) VALUES (?, ?)").run(uuid, source);
        if (source === "human" && inputKind !== undefined) {
          this.open().query("INSERT OR REPLACE INTO prompt_inputs(uuid, kind) VALUES (?, ?)").run(uuid, inputKind);
        }
      }
    })();
  }

  close(): void { this.closed = true; this.database?.close(); this.database = undefined; }

  private open(): Database {
    if (this.closed) throw new Error("Prompt source store is closed");
    if (this.database) return this.database;
    const config = resolve(process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), ".claude"));
    const namespace = createHash("sha256").update(config).digest("hex");
    const folder = join(process.env.XDG_STATE_HOME ?? join(homedir(), ".local", "state"), "carlos");
    mkdirSync(folder, { recursive: true, mode: 0o700 });
    const database = new Database(join(folder, `claude-sources-${namespace}.sqlite`), { create: true });
    database.exec("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS prompts(uuid TEXT PRIMARY KEY, source TEXT NOT NULL); CREATE TABLE IF NOT EXISTS prompt_inputs(uuid TEXT PRIMARY KEY, kind TEXT NOT NULL)");
    this.database = database;
    return database;
  }
}
