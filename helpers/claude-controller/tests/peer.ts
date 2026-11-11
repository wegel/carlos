/** Drives the helper process and records every frame for bounded protocol assertions. */
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

export type Frame = Record<string, any>;
type Waiter = { predicate: (frame: Frame) => boolean; resolve: (frame: Frame) => void };

export class Peer {
  readonly frames: Frame[] = [];
  readonly process: ChildProcessWithoutNullStreams;
  readonly exited: Promise<number | null>;
  private waiters: Waiter[] = [];
  private nextId = 0;
  private stderr = "";


  constructor(readonly directory: string, executable?: string) {
    this.process = spawn(executable ?? process.execPath, executable ? [] : ["src/main.ts"], {
      cwd: executable ? directory : resolve(import.meta.dir, ".."),
      env: { PATH: "/usr/bin:/bin", HOME: directory, CLAUDE_CONFIG_DIR: join(directory, "claude"),
        CARLOS_FAKE_CLI_LOG: join(directory, "cli.jsonl") },
    });
    this.process.stderr.on("data", (data) => { this.stderr += data; });
    createInterface({ input: this.process.stdout }).on("line", (line) => {
      const frame = JSON.parse(line);
      this.frames.push(frame);
      for (const waiter of [...this.waiters]) {
        if (!waiter.predicate(frame)) continue;
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        waiter.resolve(frame);
      }
    });
    this.exited = new Promise((resolve) => this.process.on("exit", resolve));
  }

  static async create(executable?: string): Promise<Peer> {
    return new Peer(await mkdtemp(join(tmpdir(), "carlos-controller-")), executable);
  }

  send(method: string, params: Frame = {}): Promise<Frame> {
    const id = `request-${++this.nextId}`;
    this.raw({ v: 1, id, method, params });
    return this.wait((frame) => frame.type === "reply" && frame.id === id);
  }

  raw(frame: Frame | string): void {
    this.process.stdin.write(`${typeof frame === "string" ? frame : JSON.stringify(frame)}\n`);
  }

  async start(params: Frame = {}): Promise<Frame> {
    return this.send("start", { executable: resolve(import.meta.dir, "fake-cli.py"), cwd: this.directory,
      settingSources: ["user", "project", "local"], ...params });
  }

  prompt(text: string, params: Frame = {}): Promise<Frame> {
    return this.send("prompt", { uuid: crypto.randomUUID(), text, origin: { kind: "human" }, priority: "later", ...params });
  }

  wait(predicate: (frame: Frame) => boolean): Promise<Frame> {
    const found = this.frames.find(predicate);
    if (found) return Promise.resolve(found);
    return new Promise((resolve, reject) => {
      const waiter: Waiter = { predicate, resolve: (frame) => { clearTimeout(timer); resolve(frame); } };
      const timer = setTimeout(() => {
        this.waiters.splice(this.waiters.indexOf(waiter), 1);
        reject(new Error(`Timed out waiting for frame; stderr=${this.stderr}; frames=${JSON.stringify(this.frames)}`));
      }, 5000);
      this.waiters.push(waiter);
    });
  }

  async cliFrames(): Promise<Frame[]> {
    return (await readFile(join(this.directory, "cli.jsonl"), "utf8")).trim().split("\n").map((line) => JSON.parse(line));
  }

  async dispose(): Promise<void> {
    if (this.process.exitCode === null) this.process.kill("SIGKILL");
    await this.exited;
    await rm(this.directory, { recursive: true, force: true });
  }
}
