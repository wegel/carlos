/** Converts optional quota percentages and ISO reset times without mixing model buckets. */
import type { SDKControlGetUsageResponse } from "@anthropic-ai/claude-agent-sdk";

export type QuotaWindow = { key: string; label: string; usedPercent: number; resetsAt: number };

export function quotaWindows(report: SDKControlGetUsageResponse): QuotaWindow[] {
  if (!report.rate_limits_available || !report.rate_limits) return [];
  const windows: QuotaWindow[] = [];
  const add = (key: string, label: string, value: { utilization: number | null; resets_at: string | null } | null | undefined) => {
    if (typeof value?.utilization !== "number" || !Number.isFinite(value.utilization)
      || value.utilization < 0 || value.utilization > 100 || !value.resets_at) return;
    const reset = Date.parse(value.resets_at) / 1000;
    if (Number.isFinite(reset)) windows.push({ key, label, usedPercent: value.utilization, resetsAt: Math.floor(reset) });
  };
  for (const key of ["five_hour", "seven_day", "seven_day_oauth_apps", "seven_day_opus", "seven_day_sonnet"] as const)
    add(key, key.replaceAll("_", " "), report.rate_limits[key]);
  for (const row of report.rate_limits.model_scoped ?? []) add(`model:${row.display_name}`, row.display_name, row);
  return windows.slice(0, 128);
}
