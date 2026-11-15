# Wait for a cancelled native child to disappear

The joined daemon run used the frozen `form-compat-pair` and passed 142 of 143 cases. The held Grok initialize case failed because `kill(pid, 0)`, which checks whether a process ID still exists, still found its child immediately after the daemon exited. Its native initialize marker and pending operation were correct. The captured PID later disappeared.

Grok uses its own ACP process and cancellation callback. Commit `1d7bcd4` changed the Codex process group path. Grok cancellation sends a fatal signal without waiting for a native reply. The OS can still expose that PID briefly after it receives the signal.

A controlled probe reproduced the exact original unittest failure with the frozen product binary. The probe gave the captured child CPU time only when its selected CPU was idle, then kept that CPU busy until the test first observed the PID after shutdown. The child had `SIGKILL` pending (`ShdPnd=0x100`), no process memory, and `PPid=1`. In the separate timing trace, its PID disappeared 1.3ms after the CPU gate opened. The same original test passes under the same schedule after the fixture adds the bounded death fence.

The two-line fixture change asserts that the captured child is alive before shutdown, then uses the existing eight-second `wait_for` predicate to require its PID to disappear. The original final `assertFalse(alive(child))` remains. The adjacent three-backend shutdown test already uses this death/reap fence. The frozen product pair remains valid because this change affects only the test.

Evidence:

- Original full failure: `/tmp/carlos-ep019b/joined-runtime-all-daemon.log`, case artifacts under `joined-runtime-all-daemon/carlos-daemon-cancel-launch-l7h5uhp2`.
- First unchanged focused run: `/tmp/carlos-ep019b/cancel-launch-before01.log` passed. The failure depends on the signal/death schedule.
- Controlled timing probe: `/tmp/carlos-ep019b/cancel_launch_schedule_probe.py`, `cancel-launch-scheduler-trace.json` and `.log`.
- Exact original unittest, controlled failure: `/tmp/carlos-ep019b/cancel_launch_case_probe.py before`, `cancel-launch-controlled-before.log`, `cancel-launch-before-trace.json`.
- Same unittest and schedule after the fence: `cancel_launch_case_probe.py after`, `cancel-launch-controlled-after.log`, `cancel-launch-after-trace.json`.
- Ordinary focused two shutdown cases: `/tmp/carlos-ep019b/cancel-launch-focused-after.log` passed.
- Full 143 after the fixture change: `/tmp/carlos-ep019b/cancel-launch-full143-after.log` and `cancel-launch-full143-after/` artifacts. All 143 passed in 76.044s.
- Frozen binary/helper identity: `/tmp/carlos-ep019b/form-compat-pair/identity.json`. These checks do not claim a final EP HEAD or release gate.

The probes restore their own CPU affinity and stop/reap their private CPU gate. No install, main-branch edit, push, merge or reviewer ran.
