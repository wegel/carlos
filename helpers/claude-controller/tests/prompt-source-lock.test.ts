/** Keep two helpers from losing prompt labels while SQLite has a live writer. */
import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const sourceModule = new URL("../src/prompt-sources.ts", import.meta.url).pathname;

async function firstLine(stream: ReadableStream<Uint8Array>): Promise<void> {
  const reader = stream.getReader();
  const first = await reader.read();
  reader.releaseLock();
  expect(new TextDecoder().decode(first.value)).toContain("attempt");
}

test("a second helper waits for the active writer then preserves source and input kind", async () => {
  const root = mkdtempSync(join(tmpdir(), "carlos-source-lock-"));
  const env = { ...process.env, CLAUDE_CONFIG_DIR: join(root, "claude"), XDG_STATE_HOME: join(root, "state") };
  const imported = `import { PromptSources } from ${JSON.stringify(sourceModule)};`;
  const seed = Bun.spawn([process.execPath, "-e", `${imported} const s=new PromptSources(); s.save('seed','human'); s.close();`], { env, stdout: "pipe", stderr: "pipe" });
  expect(await seed.exited).toBe(0);
  const directory = join(env.XDG_STATE_HOME, "carlos");
  const path = join(directory, readdirSync(directory).find(name => name.endsWith(".sqlite"))!);
  const database = new Database(path);
  database.exec("BEGIN IMMEDIATE");
  const writer = Bun.spawn([process.execPath, "-e", `${imported}
    console.log('attempt'); const s=new PromptSources(); s.save('next','human','steer'); s.close();`], { env, stdout: "pipe", stderr: "pipe" });
  try {
    await firstLine(writer.stdout);
    // Observe refusal to fail while the lock remains held, rather than racing its release.
    const result = await Promise.race([writer.exited.then(() => "exited"), Bun.sleep(100).then(() => "waiting")]);
    expect(result).toBe("waiting");
    database.exec("COMMIT");
    expect(await writer.exited).toBe(0);
    expect(database.query("SELECT source FROM prompts WHERE uuid='next'").get()).toEqual({ source: "human" });
    expect(database.query("SELECT kind FROM prompt_inputs WHERE uuid='next'").get()).toEqual({ kind: "steer" });
  } finally {
    writer.kill();
    await writer.exited;
    database.close();
    rmSync(root, { recursive: true, force: true });
  }
});
