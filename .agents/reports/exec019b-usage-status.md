# EP019b: preserve context, quota and activity in the socket footer

Commit `47ad6ca` lets the socket terminal use the same status-label placement, token formatting, pace colors, model shortening, and activity sweep as the embedded terminal. Its ordinary footer shows accepted model choices, context counts and stale markers, weekly allowance, native activity, task uncertainty, private queue mode, dictation state, hidden pending requests, and the Ralph badge. Connection and private-command notices retain their existing precedence. The question overlay still paints after the composer, and the settings popup keeps its existing place above the main view.

The shared painter lives in `src/app/status_line.rs`; `render_status.rs` remains the embedded adapter and `remote/status.rs` supplies the socket adapter. Both call the same placement and drawing functions. The socket adapter uses the existing `SettingsFacts.label`, including the native current-turn versus future-choice distinction, instead of inventing a second settings label.

## Facts and native reads

An optional `SessionInfo.status` adds only facts that the footer needs: Claude context freshness, native main activity and thinking estimate, pending request count, task uncertainty, and Ralph toggle/waiting/continuation flags. Existing context, quota, model, turn, and task facts remain authoritative. Queued human and automatic prompt text never enters the new status payload. The footer uses the same canonical unsubmitted cards as the question panel for its F4 reminder, including beside another status notice.

The core exports each transient native label as at most 2,048 UTF-8 bytes, including a visible ellipsis when it clips the label. It retains the original byte count and a complete flag; the original native text stays in the core. These strings describe what the device can display and never identify a native task, request, or run. The optional Rust payload uses a box so the new fields do not inflate every session-change enum value. The client counts the retained label bytes against its existing memory allowance.

Claude supplies its actual shared seven-day bucket to the existing quota field. Codex supplies its own reported seven-day bucket. Grok supplies none. Model-specific buckets and detailed quota rows remain separate in the task overview. The daemon never treats session token totals as account quota. A captured clock preserves the exact reset timestamp when the core publishes a Claude quota.

The daemon now drives Codex's existing optional rate-limit reader. It keeps the existing request ids, five-minute refresh interval, ten-second timeout, sparse-update rules, and account/reconnect clearing. The native sender already queues the small request without waiting for its pipe or reply. The embedded bridge retains its existing reader; this change adds no second poller to the same embedded session.

Each terminal redraws countdowns using the absolute reset time and its own clock. Its animation clock also remains local. Opening another terminal does not create another native quota read, and the footer does not subscribe to task-panel polling.

## Actual terminal checks

Five standalone workflows in `tests/ui/test_daemon_status.py` drive the real terminal, daemon, and deterministic native peers:

- A withheld Codex quota read leaves two private Unicode drafts editable. One native reply updates both footers; widths 8 through 100 preserve the old right-edge quota layout and context warning color.
- Sparse Codex updates retain the accepted reset/duration, ignore another model's bucket, and hide an expired quota without inventing a replacement value.
- Claude's native quota reaches the main footer without task interest. Opening F2 still shows its distinct five-hour and seven-day rows and preserves the private composer.
- A native context read fails while Claude reports a thinking estimate. The footer retains the stale count, accepts private typing, then clears the stale marker after the existing retry deadline and matching reply.
- Explicit user text triggers native compaction facts. Two terminals show one failure notice and one successful attempt, then the refreshed native context count. Neither device sends an extra compact prompt or changes another device's draft.

The final clean candidate passes all five workflows in 9.591 seconds. Three existing question-panel workflows also pass: hiding a nonblocking request without sending its answer as chat, preserving the exact answer through reconnect, and retaining it through a failed subscription before explicit retry. The pre-status terminal fails at the precise missing weekly-label check while keeping its draft editable. This matched run uses the same frozen candidate daemon as the positive run.

Four existing weekly terminal workflows and three existing embedded Claude status workflows also pass after the pure-painter extraction. The first relocated embedded-Claude run failed before native startup because it lacked the packaged-helper path; the retry supplies the recorded frozen controller through a command-scoped environment variable. That setup failure remains recorded.

Thirty-two focused terminal status tests pass, including the original narrow-layout and sweep checks plus three new per-device clock/label cases. Four new core tests cover bounded UTF-8 export, publication without task interest, backend-specific quota, and private Ralph prompt exclusion. Default and no-default workspace tests and strict workspace Clippy pass on the final clean source. The normal source commit hook also passes the integrated workspace tests.

## Closed-reader correction and source identity

A final timer audit found that the new owner wait hook could retain an overdue quota deadline after native closure or while Codex rebuilt. Two focused tests fail before the guard with `Some(0ns)` where no wakeup is allowed. The corrected hook uses the existing native-closed and backend-readiness checks, so it cannot keep the owner awake on a stopped reader. It adds no retry policy or scheduler.

The developer first captured 1,952 source/fixture files during a coordinated short freeze. That intermediate copy included unfinished neighboring work and helped check the five status workflows and the closed-reader guard. `/tmp/carlos-ep019b/usage-status-source-capture.json` and `usage-status-guard-pair/identity.json` retain its exact hashes and results separately.

After the question prerequisite landed, the developer rebuilt the final candidate on commit `13e0ff1`, tree `7b7fd209`, plus the exact 21-path status patch. All 21 measured status files equal the blobs committed in `47ad6ca`; the intervening parent commit `6f941c8` changes documentation only. The compiled Claude controller comes from the same clean source and pinned Bun 1.3.10, using the existing locked dependencies without an install.

The final frozen terminal SHA256 is `a94615e387365c031de6d98b829a190595eaa0d9b6dbab44830aa16242990aba`; the daemon SHA256 is `96c35d6f0c19011889c00d02431851d44495d1fe00fbb16106a475b7137325be`. Exact logs, fixture hashes, matched negative, source layers, and limits are collected in `/tmp/carlos-ep019b/usage-status-handoff.json`. The clean build and terminal logs use the `usage-status-clean-` prefix in that directory.

## Remaining EP work

The old terminal has no separate compact key or native-control command. Existing explicit text and native compaction notices keep their established route. The lead developer separately owns deferred Ctrl+R; the headless SetRalph command remains explicit on/off. The startup worker separately owns native F5 reconnect and its guarded caller. Both remain required EP019b work.

The status commit follows the question-visibility prerequisite and preserves its hidden-card reminder. It adds only the readonly Ralph getter and the status initialization/tick hooks; the lead developer's Ralph changes and the startup worker's run-loop move remain separate. The lead developer owns runner registration, whole-EP checks, embedded-path removal, and final reviews. No mid-EP reviewer, install, merge, push, or default-route change ran for this slice.
