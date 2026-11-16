/** Keeps a native window whole when a result or assistant block needs an earlier record. */
import type { ObjectValue } from "./protocol";

export class HistoryWindows {
  private dependencies: number[] = [];
  private tools = new Map<string, number>();
  private assistants = new Map<string, number>();

  get length(): number { return this.dependencies.length; }

  record(record: ObjectValue): void {
    const position = this.length;
    let earliest = position;
    const message = record.message as ObjectValue | undefined;
    const content = Array.isArray(message?.content) ? message.content as ObjectValue[] : [];
    if (record.type === "assistant") {
      const id = typeof message?.id === "string" ? message.id : record.uuid;
      if (typeof id === "string" && id.trim()) {
        earliest = this.assistants.get(id.trim()) ?? position;
        this.assistants.set(id.trim(), earliest);
      }
      for (const part of content) {
        if (part.type === "tool_use" && typeof part.id === "string") this.tools.set(part.id, position);
      }
    }
    if (record.type === "user") {
      for (const part of content) {
        if (part.type === "tool_result" && typeof part.tool_use_id === "string") {
          earliest = Math.min(earliest, this.tools.get(part.tool_use_id) ?? position);
        }
      }
    }
    this.dependencies.push(earliest);
  }

  seal(): void { this.tools.clear(); this.assistants.clear(); }

  start(end: number, limit: number): number {
    let start = Math.max(0, end - limit);
    for (let position = end - 1; position >= start; position--) {
      start = Math.min(start, this.dependencies[position]!);
    }
    return start;
  }
}
