/** Isolates the unstable quota read so an unsupported or slow read cannot block agent work. */
import type { Query, SDKControlGetUsageResponse } from "@anthropic-ai/claude-agent-sdk";

import { quotaWindows, type QuotaWindow } from "./quota-windows";

type UsageResult = { available: boolean; report?: SDKControlGetUsageResponse; windows?: QuotaWindow[]; reason?: string };

export class OptionalUsage {
  private pending?: Promise<UsageResult>;

  async read(query: Query, signal: AbortSignal): Promise<UsageResult> {
    const method = query.usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET;
    if (typeof method !== "function") return { available: false, reason: "unsupported" };
    if (this.pending) return { available: false, reason: "read_pending" };
    const operation = Promise.resolve().then(() => method.call(query, { skipBehaviors: true }))
      .then(report => ({ available: report.rate_limits_available === true, report, windows: quotaWindows(report) }))
      .catch(() => ({ available: false, reason: "unavailable" }));
    this.pending = operation;
    void operation.finally(() => { if (this.pending === operation) this.pending = undefined; });
    return new Promise(resolve => {
      const finish = (result: UsageResult) => { clearTimeout(timer); signal.removeEventListener("abort", abort); resolve(result); };
      const abort = () => finish({ available: false, reason: "closed" });
      const timer = setTimeout(() => finish({ available: false, reason: "timeout" }), 5000);
      signal.addEventListener("abort", abort, { once: true });
      if (signal.aborted) abort();
      void operation.then(finish);
    });
  }
}
