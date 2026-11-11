/** Runs the local JSON-lines controller without letting a blocked host grow its output. */
import { createInterface } from "node:readline";

import { runAnswerWorker } from "./answer-worker";

import { Controller } from "./controller";
import { CLOSE_DRAIN_MS, OutputError, OutputWriter } from "./output-writer";
import { StdoutStream } from "./stdout-stream";

if (process.argv.includes("--validate-native-answer")) {
  await runAnswerWorker();
} else {
  runController();
}

function runController(): void {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  const pending = new Set<Promise<void>>();
  let controller: Controller;
  const writer = new OutputWriter(new StdoutStream(process.stdout.fd), (error) => {
    process.stderr.write(`Claude controller output failed: ${error.message}\n`);
    process.exitCode = 1;
    controller.fail(error instanceof OutputError ? error : new OutputError("stdout_error", error.message));
    // An OS write can remain blocked on some pipe implementations after JS cleanup.
    // The child was stopped above; this final deadline applies only to fatal output errors.
    setTimeout(() => process.exit(1), 3000).unref();
  });
  controller = new Controller(writer, undefined, (reason) => {
    if (!["shutdown", "stdin_eof", "sdk_eof", "SIGTERM", "SIGINT"].includes(reason)) {
      process.exitCode = 1;
      process.stderr.write(`Claude controller closed: ${reason}\n`);
    }
    lines.close();
    process.stdin.pause();
    queueMicrotask(() => { void finishOutput(writer, pending); });
  });

  lines.on("line", (line) => {
    const request = controller.receive(line);
    pending.add(request);
    void request.finally(() => pending.delete(request));
  });
  lines.on("close", () => controller.close("stdin_eof"));
  process.stdin.on("error", () => { controller.close("stdin_error"); process.exitCode = 1; });
  for (const signal of ["SIGTERM", "SIGINT"] as const) {
    process.on(signal, () => { controller.close(signal); process.exitCode = signal === "SIGINT" ? 130 : 143; });
  }

}

async function finishOutput(writer: OutputWriter, pending: Set<Promise<void>>): Promise<void> {
  // A deadline applies only after shutdown/EOF/failure, never while a user thinks.
  const timer = setTimeout(() => writer.abort(new OutputError("output_drain_timeout",
    `Host did not drain stdout within ${CLOSE_DRAIN_MS} ms after close`)), CLOSE_DRAIN_MS);
  await Promise.allSettled([...pending]);
  await writer.finish();
  clearTimeout(timer);
}
