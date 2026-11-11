/** Checks byte caps and ordered writes against a stream that can stop draining. */
import { Writable } from "node:stream";

import { expect, test } from "bun:test";

import { NativeOutput } from "../src/native-output";
import { OutputWriter } from "../src/output-writer";
import type { Output } from "../src/protocol";
import { RecentIds } from "../src/recent-ids";

class HeldStream extends Writable {
  readonly frames: string[] = [];
  private release?: () => void;

  constructor() { super({ highWaterMark: 1 }); }

  override _write(chunk: Buffer, _encoding: BufferEncoding, done: (error?: Error | null) => void): void {
    this.frames.push(chunk.toString("utf8"));
    this.release = () => done();
  }

  drainFrame(): void { const release = this.release; this.release = undefined; release?.(); }
}

const frame = (id: string): Output => ({ v: 1, type: "reply", id, ok: true, result: "some output" });

test("writer sends one frame at a time and preserves all accepted frames when the host resumes", async () => {
  const stream = new HeldStream();
  const failures: Error[] = [];
  const writer = new OutputWriter(stream, (error) => failures.push(error));
  const first = writer.write(frame("first"));
  const second = writer.write(frame("second"));
  expect(stream.frames).toHaveLength(1);
  expect(writer.queuedBytes).toBeGreaterThan(Buffer.byteLength(stream.frames[0]!));
  stream.drainFrame();
  await first;
  expect(stream.frames).toHaveLength(2);
  stream.drainFrame();
  await second;
  expect(writer.queuedBytes).toBe(0);
  expect(stream.frames.map((line) => JSON.parse(line).id)).toEqual(["first", "second"]);
  expect(failures).toHaveLength(0);
  stream.destroy();
});

test("synchronous producer overflow fails once, rejects pending writes, and frees the byte budget", async () => {
  const stream = new HeldStream();
  const failures: Error[] = [];
  const writer = new OutputWriter(stream, (error) => failures.push(error), 100);
  const first = writer.write(frame("first")).catch((error) => error);
  writer.enqueue(frame("second"));
  expect((await first).code).toBe("output_overflow");
  expect(writer.queuedBytes).toBe(0);
  expect(stream.destroyed).toBe(true);
  expect(failures).toHaveLength(1);
  await writer.write(frame("third")).catch(() => {});
  expect(failures).toHaveLength(1);
});

test("close drain deadline frees a permanently blocked stream", async () => {
  const stream = new HeldStream();
  const failures: Error[] = [];
  const writer = new OutputWriter(stream, (error) => failures.push(error));
  const pending = writer.write(frame("first")).catch((error) => error);
  await writer.finish(20);
  expect((await pending).code).toBe("output_drain_timeout");
  expect(failures).toHaveLength(1);
  expect(stream.destroyed).toBe(true);
});

test("native gate counts queued events but not control frames; delivery frees each event budget", () => {
  const failures: Error[] = [];
  const native = new NativeOutput((error) => failures.push(error), 1000, 1000, 2);
  native.on("data", () => {});
  native.on("error", () => {});
  native.write('{"type":"system","subtype":"a"}\n');
  native.write('{"type":"control_response","response":{}}\n');
  native.write('{"type":"system","subtype":"session_state_changed","sdk_host_only":true}\n');
  expect(native.pendingEvents).toBe(1);
  native.delivered();
  expect(native.pendingBytes).toBe(0);
  native.write('{"type":"system","subtype":"b"}\n');
  native.write('{"type":"system","subtype":"c"}\n');
  native.write('{"type":"system","subtype":"d"}\n');
  expect(failures).toHaveLength(1);
  expect((failures[0] as { code?: string }).code).toBe("native_backlog_overflow");
  native.destroy();
});

test("native gate stops oversized partial lines before JSON parsing", () => {
  const failures: Error[] = [];
  const native = new NativeOutput((error) => failures.push(error), 20);
  native.on("error", () => {});
  native.write('{"type":"system",');
  native.write('"text":"too large');
  expect((failures[0] as { code?: string }).code).toBe("native_frame_overflow");
  native.destroy();
});

test("recent ID memory evicts completed requests but never evicts active requests", () => {
  const ids = new RecentIds(4);
  ids.begin("active");
  for (let index = 0; index < 5000; index++) { ids.begin(`id-${index}`); ids.complete(`id-${index}`); }
  expect(() => ids.begin("active")).toThrow("active");
  expect(() => ids.begin("id-4999")).toThrow("recently");
  expect(() => ids.begin("id-0")).not.toThrow();
  ids.complete("active");
  expect(() => ids.begin("active")).toThrow("recently");
});
