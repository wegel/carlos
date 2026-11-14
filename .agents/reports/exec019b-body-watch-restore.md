# Wait for accepted session facts before restoring body watches

The terminal could restore original-source or normalized-output watches after
the daemon rejected its reconnect subscription. The old predicate checked that
the cached session was Ready and no request was pending; it did not check that
this connection had successfully subscribed. Reuse `session_is_ready` for both
automatic watch restore and the terminal's visible-read caller. Accepted bytes
remain available for offline reading. Explicit successful retry restores the
accepted cursor once; no chat or private mutation is replayed.

The new public client test exercises both formats through held, rejected and
superseded subscription replies. It fails before the fix because an unwanted
watch appears after rejection. The actual terminal/daemon relay test fails on
the old release binary with two source subscriptions instead of one, then
passes with the fix while retaining visible text and the private composer.
Logs under `/tmp/carlos-ep019b`: `body-watch-restore-negative.log`,
`body-watch-restore-ui-negative.log`, `body-watch-restore-client-tests.log` and
`body-watch-restore-ui-fixed.log`. The fixed focused UI proof took0.539s.

The first isolated full release run exposed an unrelated test assumption in
the200-source workflow: seeing the last tool's text does not mean PageDown has
reached its trailing rows and entered follow-bottom. On phone resize the
terminal correctly kept its manual scroll anchor, leaving the final text below
the smaller screen. The test now presses End before checking follow-bottom
across resize. It still requires all200 original sources,32 maximum watches,
exact history reads and the earlier prepend anchor. Both focused tests pass
in2.002s at `target/ui-artifacts/body-restore-history-fixed`; the original
failed artifact is `run-pm5mhS` under the isolated release target.

The repaired isolated default-feature `bash scripts/test.sh --release` passes:
1293Rust tests/7ignored,53 daemon workflows,220 terminal workflows,67 helper
checks,15 architecture checks, strict Clippy/fmt and package smoke. Log:
`/tmp/carlos-ep019b/body-watch-restore-release-repeat.log`; artifacts:
`/tmp/carlos-ep019b/markdown-runtime-target/ui-artifacts/run-tlADkI`.
This accepted-code snapshot excludes concurrent task/long-line work. The
native Codex schema did not change. Neither main nor the installed binary changed.

The first bounded reviewer invocation timed out while waiting for the full
gate and produced no verdict. The stricter noninteractive retry returned PASS
without findings after reading the focused code and completed negative/fixed
proofs. Raw output follows. This closes a reconnect prerequisite, not EP019b.

1. Reviewed only the unstaged changes in the four specified files, plus the completed negative proofs and fixed terminal log.

2. Findings: No BLOCKER, MAJOR, or MINOR issues found. Both automatic watch restoration and terminal visible reads now require an accepted current session subscription. Held, rejected, and superseded replies cannot reopen watches. The client test checks both body formats and their accepted cursors; the terminal test checks retained text, private draft, and zero sends.

3. Corrective guidance: None required. The fixed terminal proof passed in 0.539s. The supplied client suite reports 76 passing tests. I did not inspect or wait for the separate release gate.

4. Verdict: **PASS** for the scoped changes.