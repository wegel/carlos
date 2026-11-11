/** Writes bounded JSON frames in order and waits when the host stops reading. */
import type { Writable } from "node:stream";

import type { Output } from "./protocol";

export const OUTPUT_LIMIT_BYTES = 4 * 1024 * 1024;
export const CLOSE_DRAIN_MS = 1000;
export type OutputSink = { write: (frame: Output) => Promise<void>; enqueue: (frame: Output) => void };
type Frame = { data: string; bytes: number; resolve: () => void; reject: (error: Error) => void };

export class OutputError extends Error {
  constructor(readonly code: string, message: string) { super(message); }
}

export class OutputWriter implements OutputSink {
  private queue: Frame[] = [];
  private bytes = 0;
  private failure?: Error;
  private writing = false;
  private idle = new Set<() => void>();

  constructor(private stream: Writable, private onFailure: (error: Error) => void,
    private limit = OUTPUT_LIMIT_BYTES) {
    stream.on("error", (error) => this.fail(error));
    stream.on("close", () => {
      if (this.queue.length) this.fail(new OutputError("stdout_closed", "Host closed stdout with queued frames"));
    });
  }

  get queuedBytes(): number { return this.bytes; }

  abort(error: Error): void { this.fail(error); }

  write(frame: Output): Promise<void> {
    if (this.failure) return Promise.reject(this.failure);
    const data = `${JSON.stringify(frame)}\n`;
    const bytes = Buffer.byteLength(data);
    if (bytes + this.bytes > this.limit) {
      this.fail(new OutputError("output_overflow", `Queued stdout exceeds ${this.limit} bytes`));
      return Promise.reject(this.failure);
    }
    return new Promise((resolve, reject) => {
      this.bytes += bytes;
      this.queue.push({ data, bytes, resolve, reject });
      this.pump();
    });
  }

  enqueue(frame: Output): void { void this.write(frame).catch(() => {}); }

  async finish(graceMs = CLOSE_DRAIN_MS): Promise<void> {
    if (this.failure || !this.queue.length) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    await new Promise<void>((resolve) => {
      this.idle.add(resolve);
      timer = setTimeout(() => this.fail(new OutputError("output_drain_timeout",
        `Host did not drain stdout within ${graceMs} ms after close`)), graceMs);
    });
    clearTimeout(timer);
  }

  private pump(): void {
    if (this.writing || this.failure) return;
    const frame = this.queue[0];
    if (!frame) { this.resolveIdle(); return; }
    this.writing = true;
    let flushed = false;
    let drained = false;
    const complete = () => {
      if (!flushed || !drained || this.failure) return;
      this.stream.off("drain", drain);
      this.queue.shift();
      this.bytes -= frame.bytes;
      this.writing = false;
      frame.resolve();
      this.pump();
    };
    const drain = () => { drained = true; complete(); };
    this.stream.once("drain", drain);
    try {
      const ready = this.stream.write(frame.data, (error) => {
        if (error) { this.fail(error); return; }
        flushed = true;
        complete();
      });
      if (ready) { drained = true; complete(); }
    } catch (error) {
      this.stream.off("drain", drain);
      this.fail(error instanceof Error ? error : new Error(String(error)));
    }
  }

  private fail(error: Error): void {
    if (this.failure) return;
    this.failure = error;
    for (const frame of this.queue.splice(0)) frame.reject(error);
    this.bytes = 0;
    this.resolveIdle();
    this.stream.destroy();
    this.onFailure(error);
  }

  private resolveIdle(): void {
    for (const resolve of this.idle) resolve();
    this.idle.clear();
  }
}
