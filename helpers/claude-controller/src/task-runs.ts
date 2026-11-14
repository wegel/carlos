/** Fences native per-task stops against the newest run observed on the CLI pipe. */
import { ProtocolError } from "./protocol";

type Run = { id: string; terminal: boolean; uncertain: boolean; bytes: number };
const MAX_TASKS = 4096;
const MAX_BYTES = 1024 * 1024;
const MAX_ID_BYTES = 4096;

export class TaskRuns {
  private runs = new Map<string, Run>();
  private bytes = 0;
  private unavailable = false;

  observe(value: unknown): void {
    if (!record(value) || value.type !== "system") return;
    if (value.subtype === "background_tasks_changed") {
      if (Array.isArray(value.tasks)) for (const task of value.tasks) {
        if (record(task)) this.accept(task.task_id, task.run_id, task.status);
      }
    } else if (typeof value.subtype === "string" && ["task_started", "task_progress", "task_updated", "task_notification"].includes(value.subtype)) {
      const patch = record(value.patch) ? value.patch : value;
      this.accept(value.task_id, value.run_id, patch.status);
    }
  }

  require(task: string, expected: unknown): void {
    // Older CLIs have no run ids; their existing explicit controls remain available.
    if (expected === undefined || expected === null) return;
    if (typeof expected !== "string" || !expected || Buffer.byteLength(expected) > MAX_ID_BYTES) {
      throw new ProtocolError("invalid_request", "A task run must be a bounded nonempty string");
    }
    if (this.unavailable) {
      throw new ProtocolError("task_run_unavailable", "Native task identities exceeded the guarded-stop budget; open a new query before using guarded task stops");
    }
    const run = this.runs.get(task);
    if (!run || run.id !== expected || run.terminal || run.uncertain) {
      throw new ProtocolError("stale_task_run", "The selected native task run is absent, changed, ended or unknown; stop was not sent");
    }
  }

  private accept(task: unknown, id: unknown, status: unknown): void {
    if (this.unavailable || typeof task !== "string" || !task) return;
    if (Buffer.byteLength(task) > MAX_ID_BYTES || typeof id === "string" && Buffer.byteLength(id) > MAX_ID_BYTES) {
      this.disable();
      return;
    }
    const previous = this.runs.get(task);
    if (typeof id !== "string" || !id) {
      if (previous) previous.uncertain = true;
      return;
    }
    if (previous && id < previous.id) return;
    const same = previous?.id === id;
    const bytes = Buffer.byteLength(task) + Buffer.byteLength(id) + 64;
    if (!previous && this.runs.size >= MAX_TASKS || bytes > MAX_BYTES - this.bytes + (previous?.bytes ?? 0)) {
      this.disable();
      return;
    }
    this.bytes += bytes - (previous?.bytes ?? 0);
    this.runs.set(task, { id, bytes, uncertain: same && previous.uncertain,
      terminal: same && previous.terminal || typeof status === "string" && ["completed", "failed", "stopped", "killed"].includes(status) });
  }

  private disable(): void {
    // Forgetting a newer run would let a late older frame authorize a stale stop.
    this.unavailable = true;
    this.runs.clear();
    this.bytes = 0;
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
