/** Runs server-controlled regular expressions in a disposable helper process. */
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { ProtocolError, type ObjectValue } from "./protocol";

declare const CARLOS_STANDALONE: boolean;
const standalone = typeof CARLOS_STANDALONE !== "undefined" && CARLOS_STANDALONE;
const VALIDATION_LIMIT_MS = 1000;
let active = 0;

/** Kill a slow answer checker while leaving the original user callback pending. */
export function validateAnswerIsolated(kind: string, payload: ObjectValue, answer: ObjectValue, signal: AbortSignal,
  budgetMs = VALIDATION_LIMIT_MS): Promise<void> {
  if (active >= 4) return Promise.reject(new ProtocolError("validation_busy", "Other answers are being checked; try again"));
  if (signal.aborted) return Promise.reject(new ProtocolError("unknown_callback", "This request was cancelled"));
  const args = standalone ? [] : [fileURLToPath(new URL("./main.ts", import.meta.url))];
  const child = spawn(process.execPath, [...args, "--validate-native-answer"], { stdio: ["pipe", "pipe", "ignore"] });
  active++;
  return new Promise((resolve, reject) => {
    let output = "";
    let finished = false;
    const finish = (error?: ProtocolError) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      // A pathological pattern cannot retain a worker after its budget or cancellation.
      child.kill("SIGKILL");
      if (error) reject(error); else resolve();
    };
    const abort = () => finish(new ProtocolError("unknown_callback", "This request was cancelled"));
    const timer = setTimeout(() => finish(new ProtocolError("validation_limit", "This form's validator exceeded its work limit; edit the answer, decline, or cancel")), budgetMs);
    signal.addEventListener("abort", abort, { once: true });
    child.on("error", error => finish(new ProtocolError("validation_failed", error.message)));
    child.stdout.on("data", data => {
      output += data;
      if (output.length > 65536) finish(new ProtocolError("validation_failed", "Validator reply exceeded its limit"));
    });
    child.on("close", () => {
      active--;
      if (finished) return;
      try {
        const result = JSON.parse(output);
        finish(result.ok ? undefined : new ProtocolError("invalid_answer", String(result.message)));
      } catch { finish(new ProtocolError("validation_failed", "Validator exited without an answer")); }
    });
    child.stdin.on("error", () => finish(new ProtocolError("validation_failed", "Validator input closed")));
    child.stdin.end(JSON.stringify({ kind, payload, answer }));
  });
}
