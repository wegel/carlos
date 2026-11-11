/** Caps native frames before the SDK can retain an unbounded unread event queue. */
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { Transform, type TransformCallback } from "node:stream";

import type { SpawnedProcess, SpawnOptions } from "@anthropic-ai/claude-agent-sdk";

import { OutputError } from "./output-writer";

export const NATIVE_FRAME_LIMIT = 4 * 1024 * 1024;
export const NATIVE_BACKLOG_LIMIT = 8 * 1024 * 1024;
export const NATIVE_EVENT_LIMIT = 4096;

export class NativeOutput extends Transform {
  private parts: Buffer[] = [];
  private partialBytes = 0;
  private unread: number[] = [];
  private unreadBytes = 0;
  private child?: ChildProcessWithoutNullStreams;

  constructor(private onFailure: (error: OutputError) => void,
    private frameLimit = NATIVE_FRAME_LIMIT, private backlogLimit = NATIVE_BACKLOG_LIMIT,
    private eventLimit = NATIVE_EVENT_LIMIT) { super(); }

  spawn(options: SpawnOptions, stderr: (text: string) => void): SpawnedProcess {
    const child = spawn(options.command, options.args, { cwd: options.cwd, env: options.env,
      signal: options.signal, stdio: ["pipe", "pipe", "pipe"] });
    this.child = child;
    child.stderr.setEncoding("utf8").on("data", stderr);
    child.stdout.on("error", (error) => this.destroy(error));
    child.stdout.pipe(this);
    return { stdin: child.stdin, stdout: this,
      get killed() { return child.killed; }, get exitCode() { return child.exitCode; },
      get signalCode() { return child.signalCode; },
      kill: child.kill.bind(child), on: child.on.bind(child), once: child.once.bind(child), off: child.off.bind(child) };
  }

  delivered(): void {
    const bytes = this.unread.shift();
    if (bytes !== undefined) this.unreadBytes -= bytes;
  }

  stop(): void {
    this.child?.stdout.destroy();
    this.child?.kill("SIGTERM");
  }

  get pendingBytes(): number { return this.unreadBytes; }
  get pendingEvents(): number { return this.unread.length; }

  override _transform(chunk: Buffer, _encoding: BufferEncoding, done: TransformCallback): void {
    try {
      let start = 0;
      for (let end = chunk.indexOf(10); end >= 0; end = chunk.indexOf(10, start)) {
        this.append(chunk.subarray(start, end + 1));
        this.forward();
        start = end + 1;
      }
      if (start < chunk.length) this.append(chunk.subarray(start));
      done();
    } catch (error) {
      const failure = error as OutputError;
      this.onFailure(failure);
      done(failure);
    }
  }

  override _flush(done: TransformCallback): void {
    try { if (this.partialBytes) this.forward(); done(); }
    catch (error) { this.onFailure(error as OutputError); done(error as Error); }
  }

  private append(part: Buffer): void {
    this.partialBytes += part.length;
    if (this.partialBytes > this.frameLimit) {
      throw new OutputError("native_frame_overflow", `Native CLI frame exceeds ${this.frameLimit} bytes`);
    }
    this.parts.push(part);
  }

  private forward(): void {
    const line = Buffer.concat(this.parts, this.partialBytes);
    if (isQueuedEvent(line)) {
      if (this.unreadBytes + line.length > this.backlogLimit || this.unread.length >= this.eventLimit) {
        throw new OutputError("native_backlog_overflow", "Native CLI exceeded the unread SDK event budget");
      }
      this.unread.push(line.length);
      this.unreadBytes += line.length;
    }
    this.parts = [];
    this.partialBytes = 0;
    this.push(line);
  }
}

function isQueuedEvent(line: Buffer): boolean {
  try {
    const message = JSON.parse(line.toString("utf8"));
    if (["control_request", "control_response", "control_cancel_request", "keep_alive", "transcript_mirror"].includes(message.type)) return false;
    return !(message.type === "system" && message.subtype === "session_state_changed" && message.sdk_host_only === true);
  } catch { return false; } // Let the SDK report malformed native JSON.
}
