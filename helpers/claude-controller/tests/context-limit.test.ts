/** Exercise saved startup sizes through the real SDK without model calls. */
import { afterEach, expect, test } from "bun:test";

import { Peer } from "./peer";

const peers: Peer[] = [];
afterEach(async () => { for (const peer of peers.splice(0)) await peer.dispose(); });

test("saved startup sizes validate before launch and use only the native flag layer", async () => {
  const peer = await Peer.create(process.env.CARLOS_CONTROLLER_TEST_EXECUTABLE);
  peers.push(peer);
  for (const contextLimit of [null, "500k", 99999, 1000001, 100000.5]) {
    expect((await peer.start({ contextLimit })).ok).toBe(false);
  }
  expect((await peer.start({ contextLimit: 500000 })).ok).toBe(true);
  const frames = await peer.cliFrames();
  const launch = frames.find((frame) => frame.argv)!;
  expect(launch.argv[launch.argv.indexOf("--settings") + 1]).toBe('{"autoCompactWindow":500000}');
  expect(frames.filter((frame) => frame.request?.subtype === "apply_flag_settings")).toHaveLength(0);
  expect(frames.filter((frame) => frame.argv)).toHaveLength(1);
  expect(frames.filter((frame) => frame.type === "user")).toHaveLength(0);
});
