/** Checks native path rules without requiring a Windows host or a model call. */
import { posix, win32 } from "node:path";

import { expect, test } from "bun:test";

import { parseStart } from "../src/protocol";

test("native startup accepts drive and UNC paths with Windows path semantics", () => {
  for (const executable of ["C:\\Tools\\claude.exe", "\\\\server\\share\\claude.exe", "C:/Tools/claude.exe"]) {
    expect(parseStart({ executable, cwd: "C:\\Projects\\repo", settingSources: [] }, win32.isAbsolute).executable).toBe(executable);
  }
  expect(() => parseStart({ executable: "C:claude.exe", cwd: "C:\\Projects", settingSources: [] }, win32.isAbsolute)).toThrow("absolute");
  expect(() => parseStart({ executable: "/usr/bin/claude", cwd: "relative", settingSources: [] }, posix.isAbsolute)).toThrow("absolute");
});
