/** Handles one answer in the isolated validation process, then exits. */
import { validateQuestionAnswer } from "./form-answer";

/** Check one answer from stdin in this disposable process. */
export async function runAnswerWorker(): Promise<void> {
  try {
    const { kind, payload, answer } = JSON.parse(await Bun.stdin.text());
    validateQuestionAnswer(kind, payload, answer);
    process.stdout.write(JSON.stringify({ ok: true }));
  } catch (error) {
    process.stdout.write(JSON.stringify({ ok: false, message: error instanceof Error ? error.message : String(error) }));
  }
}
