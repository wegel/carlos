/** Writes to the host pipe itself, bypassing Bun's additional stdout buffer. */
import { write } from "node:fs";
import { Writable } from "node:stream";

export class StdoutStream extends Writable {
  private retry?: ReturnType<typeof setTimeout>;
  private cancelWrite?: () => void;

  constructor(private fd: number) { super({ highWaterMark: 64 * 1024 }); }

  override _write(chunk: Buffer, _encoding: BufferEncoding, done: (error?: Error | null) => void): void {
    let offset = 0;
    let settled = false;
    const finish = (error?: Error | null) => {
      if (settled) return;
      settled = true;
      this.cancelWrite = undefined;
      done(error);
    };
    this.cancelWrite = () => finish(new Error("Host output pipe closed"));
    const attempt = () => {
      this.retry = undefined;
      if (settled) return;
      write(this.fd, chunk, offset, chunk.length - offset, null, (error, bytes) => {
        if (settled) return;
        if (error?.code === "EAGAIN" || error?.code === "EWOULDBLOCK") {
          this.retry = setTimeout(attempt, 5);
          return;
        }
        if (error) { finish(error); return; }
        offset += bytes;
        if (offset === chunk.length) finish();
        else this.retry = setTimeout(attempt, 0);
      });
    };
    attempt();
  }

  override _destroy(error: Error | null, done: (error?: Error | null) => void): void {
    clearTimeout(this.retry);
    this.cancelWrite?.();
    done(error);
  }
}
