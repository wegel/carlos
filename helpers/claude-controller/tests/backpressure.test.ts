/** Uses a Python host so Bun cannot secretly drain the blocked OS pipe for the test. */
import { resolve } from "node:path";

import { expect, test } from "bun:test";

async function blockedHost(mode: string): Promise<Record<string, any>> {
  const executable = process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE;
  const command = executable ? [executable] : [process.execPath, resolve(import.meta.dir, "../src/main.ts")];
  const host = Bun.spawn(["python3", resolve(import.meta.dir, "blocked-host.py"), mode, ...command], {
    stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([
    new Response(host.stdout).text(), new Response(host.stderr).text(), host.exited,
  ]);
  if (code !== 0) throw new Error(`Blocked host failed: ${stderr}`);
  return JSON.parse(stdout);
}

test("blocked output leaves callback replies and native controls live; draining preserves frames", async () => {
  const result = await blockedHost("resume");
  expect(result.code).toBe(0);
  expect(result.controlsBeforeDrain).toBe(true);
  expect(result.sequences).toEqual(Array.from({ length: 16 }, (_, index) => index));
  expect(Object.values(result.replyCounts)).toEqual(Array(23).fill(1));
});

test("shutdown and EOF exit within a bound when the host never resumes reading", async () => {
  for (const method of ["shutdown", "eof"]) {
    const result = await blockedHost(method);
    expect(result.code).toBe(1);
    expect(result.stderr).toContain("did not drain stdout");
    expect(result.elapsed).toBeLessThan(3.5);
  }
});

test("unread native event flood closes the SDK instead of filling its internal queue", async () => {
  const result = await blockedHost("flood");
  expect(result.code).toBe(1);
  expect(result.stderr).toContain("native_backlog_overflow");
  expect(result.elapsed).toBeLessThan(4);
});

test("synchronous callback flood closes the helper when its bounded output queue fills", async () => {
  const result = await blockedHost("callback-flood");
  expect(result.code).toBe(1);
  expect(result.stderr).toContain("output_overflow");
  expect(result.elapsed).toBeLessThan(4);
});
