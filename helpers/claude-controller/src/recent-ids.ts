/** Detects concurrent duplicate requests and remembers a bounded completed-ID window. */
import { ProtocolError } from "./protocol";

export const RECENT_ID_LIMIT = 1024;

export class RecentIds {
  private active = new Set<string>();
  private recent = new Set<string>();

  constructor(private limit = RECENT_ID_LIMIT) {}

  begin(id: string): void {
    if (this.active.has(id) || this.recent.has(id)) {
      throw new ProtocolError("duplicate_id", "Request ID is active or was used recently");
    }
    this.active.add(id);
  }

  complete(id: string): void {
    if (!this.active.delete(id)) return;
    this.recent.add(id);
    if (this.recent.size > this.limit) this.recent.delete(this.recent.values().next().value!);
  }
}
