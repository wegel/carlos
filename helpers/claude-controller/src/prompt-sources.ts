/** Stores Carlos-owned prompt authorship outside native Claude transcripts. */
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

import type { ObjectValue, Prompt } from "./protocol";

export type PromptSource = "human" | "automatic" | "unknown" | "system";

export class PromptSources {
  private database?: Database;
  private closed = false;

  remember(uuid: string, kind: Prompt["origin"]["kind"]): void {
    this.save(uuid, kind === "human" ? "human" : kind === "auto-continuation" ? "automatic" : "unknown");
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

  save(uuid: string, source: PromptSource): void {
    this.open().query("INSERT OR REPLACE INTO prompts(uuid, source) VALUES (?, ?)").run(uuid, source);
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
    database.exec("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS prompts(uuid TEXT PRIMARY KEY, source TEXT NOT NULL)");
    this.database = database;
    return database;
  }
}
