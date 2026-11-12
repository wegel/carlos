# Milestone 5d and preliminary tasks

Implemented P1, P2, P3, and only part 5d on `daemon-split`, starting at `395be38b5911df15c408e2ef8f7ff101c6e7f5ef`. P4 changed no repository file. The work leaves 26 changed/new files, no new dependency, and no source change under `.agents/` or `docs/`. No commit, push, stash, reset, toolchain change, or install was performed. The owner's `/var/home/wegel/work/perso/carlos` checkout was left untouched.

## Patches and exact replay

Apply `/tmp/carlos-m5d/p1.patch`, `p2.patch`, `p3.patch`, then `m5d.patch` to the starting commit. The patches contain 3, 1, 1, and 21 files respectively. A fresh archive of the starting commit accepted all four patches in that order. All 1,212 tracked/new files then matched this working tree byte for byte, with no extra file. Ignored build artifacts are outside that Git tree check. `patch-proof.json` records every changed file hash and every patch hash; `save-patches.py` repeats the proof without changing this checkout's index.

## Files

- `src/app/state_summaries.rs` (p1): Pass the borrowed turn/part scope instead of formatting a disk-cache entry before the session checks the item.
- `src/session/summary_updates.rs` (p1): Build the unchanged disk key only when binding a new request; count builds in tests with a thread-local Cell.
- `src/session/summary_binding_tests.rs` (p1): Keep the existing binding assertions and count fresh, repeated, cached-label, and disabled-label cases.
- `src/tests/runtime_tests/startup_preferences.rs` (p2): Run the startup test in a child with CARLOS_REASONING_SUMMARY cleared and temporary home/config/state/cache directories.
- `src/tests/protocol_stop_tests.rs` (p3): Name the existing timeoutMs: 40 test that still exercises the deadline thread.
- `src/app/claude_quota.rs` (m5d): Read the moved plain quota type and convert its result to the existing weekly colors.
- `src/app/codex_lifecycle.rs` (m5d): Clear quota facts and the view timer through the app method when Codex closes.
- `src/app/codex_reconnect_install.rs` (m5d): Clear both quota owners at the existing reconnect point.
- `src/app/input_timers.rs` (m5d): Keep the connection guard and combine session deadlines with the view countdown timer.
- `src/app/notifications.rs` (m5d): Send quota replies/updates to session methods and clear both owners on account changes.
- `src/app/rate_limits.rs` (m5d): Keep only the app methods that tick/combine/reset the view countdown timer and session read timers.
- `src/app/render_status.rs` (m5d): Convert borrowed session quota facts to the same weekly label before the existing layout code.
- `src/app/state.rs` (m5d): Remove AppState.rate_limits; construct its facts in SessionState and its timer in ViewState.
- `src/app/view_state.rs` (m5d): Own quota_next_draw, initially None.
- `src/app/weekly_usage.rs` (m5d): Keep the exact percent/day formats and map OnTrack/Close/Behind to the existing three colors.
- `src/session/mod.rs` (m5d): Register usage, quota facts, session methods, and plain usage tests.
- `src/session/state.rs` (m5d): Own private quota facts and expose them only through session methods.
- `src/tests/codex_reconnect_tests.rs` (m5d): Keep the quota connection-gate test using the new app tick method.
- `src/tests/input_status_tests.rs` (m5d): Keep existing screen assertions and supply quota snapshots through the session method.
- `src/tests/status_sweep_tests.rs` (m5d): Pin complete status lines at widths 28, 36, and 54 while a long model name shortens.
- `src/tests/weekly_usage_tests.rs` (m5d): Preserve every existing test and its percent-string checks; drive pending reads through the real methods instead of writing private fields.
- `src/session/rate_limits.rs` (m5d): Own windows, notifications received during a read, pending reads, and next_read; preserve the old read/update rules.
- `src/session/usage.rs` (m5d): Own RateWindow, quota arithmetic, Pace, plain WeeklyUsage values, and unix_now without terminal imports.
- `src/session/usage_tests.rs` (m5d): Enumerate plain pace boundaries, expired resets, equality with now, and subtraction overflow.
- `src/session/usage_updates.rs` (m5d): Expose borrowed quota facts plus explicit session methods for tick/reply/update/clear.
- `src/tests/weekly_usage_parity_tests.rs` (m5d): Pin text/colors, combined wakeups, read expiry/retry, and successful snapshots that stop reporting quota.

## P1, P2, and P3 evidence

- P1: `history_builds_cache_entries_only_for_new_bindings` uses N=24 and checks labels disabled, enabled with existing bindings, and enabled with cached labels already applied. Each case builds exactly N keys for its initial N new bindings, then zero keys for two passes over those N item events and missing-command completions. The thread-local counter measures the actual cache-entry formatter and adds no production work. A separate instrumented archive of `94ba37f` ran the corresponding app test and produced exactly the same counts: N initially and zero on repetition. Thus the difference from that commit is zero builds. The old code also bound requests while labels were disabled, so suppressing those initial N builds would have changed behavior. See `p1-original-counts.log` and the repository counting test. Existing disk-key spelling tests still pin both scoped JSON keys and unscoped keys.
- P2: the parent launches the same test by its exact name. Only the child receives the cleared reasoning-summary variable and temporary HOME, XDG_CONFIG_HOME, XDG_STATE_HOME, and XDG_CACHE_HOME. The child constructs the app and calls the real Codex setup helper. No test mutates its process environment. The existing startup assertions remain intact.
- P3: the one-line comment names `protocol_transport_tests::a_short_queue_read_deadline_discards_late_reply_without_stopping_native_work`, including its `timeoutMs: 40` case. No stop-test assertion changed.

## Method parity checklist

The request calls this a parity table for part 5b; these entries cover the methods moved/split by 5d. Part 5b stays untouched. Lists follow the repository's narrow-screen rule.

- `RateWindow::merge`, app weekly module to session usage: require a nonnegative integer usedPercent; clamp it at 100; read optional integer duration/reset; then fill missing facts from a compatible previous window. Missing/invalid usedPercent still returns None.
- `RateWindow::with_previous`, same move: reuse prior fields only when the new duration is absent or equal. A changed duration still cannot inherit an old reset.
- `RateWindow::indicator`, same move: reject a non-week duration, absent reset, subtraction overflow, and reset at/before now, in that order. Compute allowance left and cap the time fraction at one week. Zero allowance is Behind; the inclusive 100% pace boundary is OnTrack, then the inclusive 75% boundary is Close, otherwise Behind. It now returns numbers and Pace instead of styled strings.
- `unix_now`, same move: retain SystemTime/UNIX_EPOCH, zero before the epoch, and the i64 maximum clamp. The app's existing Claude clock callers use a read-only re-export while their state awaits milestone 6.
- `WeeklyIndicator`, app weekly module: format the same `{remaining}%` and one-decimal `d` strings. Map OnTrack to COLOR_GUTTER_USER, Close to COLOR_TOOL_EDIT, and Behind to COLOR_DIFF_REMOVE. No context-label code changed.
- `RateLimits::tick`, split between app rate_limits and session rate_limits: retain the non-Codex early return before any timer changes. The app advances a due/absent visual deadline by 60 seconds, even without a visible quota. The session then expires a pending read at deadline <= now, sets next_read to now+300 before trying a new request, and records pending/during_read only for Ok(Some(id)). A failed/unsupported request still waits for the normal refresh. A 10-second read expiry still does not trigger an immediate retry.
- `RateLimits::wait`, split across those owners: the session takes the minimum of next_read and the pending read's deadline. The app takes the minimum of that wait and the view's visual deadline. All durations still saturate at zero. Taking the minimum before or after this monotone subtraction gives the same wait.
- `RateLimits::response`, app to session: require an outstanding read and its exact string id; ignore others. Clear pending before interpreting the reply. Only a present result replaces windows, including a present null/empty result. Prefer rateLimitsByLimitId.codex, then rateLimits. Overlay the saved during-read updates after the snapshot, preserving their existing merge rule. An error with no result keeps the old windows.
- `RateLimits::update`, app to session: reject a different string limitId; retain acceptance of absent/non-string ids. Visit primary then secondary. Ignore missing/null/invalid windows. Merge the visible facts and, only while a read is pending, merge during_read separately.
- `RateLimits::clear`, split: the session resets all quota/read facts to Default; the app also resets the view deadline to None. Account changes, closure, and reconnect still call it at their old points.
- `RateLimits::indicator` / `indicator_at`, app to session: use the same clock and choose the first unexpired weekly window in primary/secondary order. The status renderer and Claude quota adapter convert the plain result to colors without changing layout.
- `input_timers::tick_rate_limits` / `idle_wait`: keep the existing connection/rebuild guard and all other deadlines in their old order. The new session methods permit explicit writes, and the app can only borrow quota facts.

## Enumerated quota and screen checks

Seven Rust tests were added: the P1 counting test, three quota parity tests, two plain usage tests, and the status-layout test. No existing test was removed, weakened, or ignored. Existing tests still check sparse merges, foreign buckets, newer notifications during reads, account clears, unsupported windows, refresh limits, and the disconnected/reconnecting gate.

The same three quota parity tests and the same status-layout test ran successfully against the starting commit in an isolated archive. The old endpoint uses tiny test-only adapters to keep the old app calls; it retains its original arithmetic and combined timer. See `previous-quota-parity.log`, `current-quota-parity.log`, and `previous-layout.log`.

Text/color cases at an explicit Unix clock include:

- 100% left / 7 days: green, `100%`, `7.0d`.
- 50% left / 3.5 days: green at equality; 49% at the same reset: amber.
- 75% left / 7 days: amber at equality; 74%: red.
- 0% left / one second: red, `0%`, `0.0d`; 1% at that reset: green.
- 100% left / 14 days: green; 99%: amber; the text remains `14.0d` while pace caps its time fraction at one week.
- A reported 101% used: clamp to 0% left, red, `1.0d` in the one-day case.
- Plain pace tests also check 50% left at 302399, 302400, and 302401 seconds, plus reset one second in the past, reset exactly now, and i64 subtraction overflow.
- Existing cases retain 40% left at 2.1 days (green), 3.5 days (amber), and 6 days (red).

Both endpoints produced these wakeups for each of five quota states: no reported quota; 60% left resetting in one day; exhausted quota resetting in one day; reset one second in the past; reset exactly now. Times below are seconds after the initial tick, using the same sequential ticks at both endpoints. A skipped tick deliberately checks a late countdown redraw.

- Before the first tick: no wait. At t=0: redraw, read #1, next wake in 10 seconds.
- t=9: wait 1, no redraw, next wake in 1.
- t=10: wait 0, expire read #1, no redraw, next wake in 50.
- t=59: wait 1, no redraw, next wake in 1.
- t=60: wait 0, redraw, next wake in 60; no new read.
- t=299: wait 0 after the deliberately skipped countdown, redraw, next wake in 1.
- t=300: wait 0, read #2, no redraw, next wake in 10.
- t=310: wait 0, expire read #2, no redraw, next wake in 49.
- t=359: wait 0, redraw, next wake in 60.
- t=600: wait 0, redraw and read #3, next wake in 10.

Three successful no-quota snapshots (empty result, empty rateLimits, and a map containing only another bucket) clear a previously reported quota. A prior sparse null update and an error reply leave it intact. Account clearing also removes the next visual wake. The old and new endpoints pass all these cases.

The complete status line is pinned at widths 28, 36, and 54 with a 26-character model name, 90k/100k context, and W50%/3.0d. The model shortens to `a…` and `abcdefghi…` at the two narrow widths; the context and weekly text keep their exact cells. The app takes a wall-clock reading only to place a reset three days away; the one-decimal countdown has a wide rounding margin, and this test has no timing deadline.

## Live-event comparison

Seed 516053 selected 3,000 streams from `target/m4-fix/streams.jsonl`: 981 Codex, 1,025 Claude, and 994 Grok. The saved sample matches the earlier worker's sample. Both the richer milestone harness and the reduced pre-plan harness compared the starting commit with this code: each matched all 3,000 streams and 54,012 checkpoints exactly, including session facts, backend calls, dirty/redraw logs, and rendered output. Neither endpoint panicked. The reduced harness also reproduced exactly 278 differences against the saved a59221c endpoint.

`comparison.json`, `sample-manifest.json`, `sample.jsonl`, the shard input/output files, and the saved build/run scripts preserve the evidence. Temporary archives contain the harness hooks; this checkout does not. The harness source and executable were rebuilt after the final production edit.

The sample contains no account/quota messages and does not tick quota timers. Its facts also omit quota fields. It therefore cannot prove quota arithmetic, timer behavior, successful quota omission, or startup-file isolation; the enumerated old/new tests above and P1/P2 tests cover those paths. The synchronous replay also does not prove all thread schedules. No work crossed a new thread boundary in 5d.

## P4: all 278 pre-plan differences

No newly discovered unintended screen change or backend-call change remains in these classes. The sole changed backend-call stream is the already documented, intended Claude failure rule. The previous report's counts were close: after ignoring only the obsolete status-string oracle, 121 streams differ. The extra 157 streams come solely from that oracle, and another status-first stream later reaches class F.

I replayed all 278 with full event-by-event old/new snapshots. `p4-ledger.json` names each stream, its first event index and payload, all fields changed at that first event, cause, prior report class, final changed facts, and whether final rendering/calls differ. `p4-counts-final.json`, `p4-first-fields.json`, `p4-normalized.json`, and `p4-{old,new}.jsonl` preserve counts, examples, and traces. The saved 94ba37f and part-5c reduced outputs agree, so the classification applies to the requested 94ba37f comparison too.

The first differing field uses the first event where a component changes, then the first changed fact in alphabetical order, or dirty if no fact changes:

- `facts.status`: 158 streams. All first differences come from the stale-hint oracle described below. One, stream 11800, also reaches F later.
- `dirty`: 103 streams. 102 are H; one is the empty-history T case below.
- `facts.active`: 12 streams. Each also changes boundary, status, and the active turn's status under F.
- `facts.rows`: five streams. Four are S; one is the visible foreign-history T case.

These disjoint first-cause classes sum to 278:

- **Stale view hint in the oracle, 158 first-cause streams; 157 exclusively.** Commit `a2a5d31` moved F4/F5 guidance out of the session string and removed the test-only status_revision/last-writer reader. The later harness replaced that reader with `view.status_hint.unwrap_or(session.status())`. This keeps an old hint forever, even after a later turn/backend fact. The production renderer reads neither string; it builds status labels from concrete task/model/context/queue/connection facts. Users see the same screen in the 157 exclusive cases, and the backend receives the same calls. The owner intended the split/removal; the cross-version oracle needs an update. For example, 68 first-cause streams compare old `turn interrupted` with stale `Codex has a pending request; F4 answers it`, 23 compare `turn completed` with that hint, and 41 compare a new Claude approval with old `selection cleared`. The rest involve later turn starts/sends, answers, interrupts, errors, and an old unavailable-choice hint. H stream 12726 also develops a status-only discrepancy later. Stream 11800 first differs in status at event 17 and reaches F at event 23, so it must not be counted as exclusively an oracle artifact.
- **H, 102 first-cause streams.** Commit `d98a64d` builds history in a draft store and no longer logs per-row invalidations from the old temporary AppState. Both endpoints request the same effective dirty range and adopt/reset the real cache. Users see the same rows and screens, and calls match. This is the intended history-store change and a difference in what the test hook records. Of these streams, 101 were H in the original full report; stream 6954 was formerly B but its remaining difference is only H after the B fix. For all 102, each changed dirty batch has the same minimum index on both sides. See `p4-h-ranges.json`; its separate 11205 entry is T, not H.
- **F, 12 first-cause streams; 13 including the later F in 11800.** Commit `ac02f0a` fails the locally active Claude turn on terminal failure even when the mock never registered its UUID in the accepted queue. Old facts can retain an InProgress turn; new facts mark Failed and clear active/boundary. Users can see the turn finish as failed and avoid a spurious later stop marker. In stream 1432, a later interrupt sends `turn/interrupt` on the old side and no call on the new side because the turn is already failed. This is intended and explicitly documented in the plan/full-corpus report. The mock's unregistered active turn is not evidence that the real SDK omits its accepted-prompt fact.
- **S, four first-cause streams (352, 13492, 13615, 15160).** Commit `d98a64d` keeps the same Claude assistant entry when native assistant/block identity repeats after completion. Users avoid a duplicate answer or empty shell, and calls match. Three streams still differ in final rendering; 15160 changes only an empty stored row at the end. The plan explicitly requires this identity rule and records repeated assistant work as reachable on resume.
- **T, two first-cause streams, both from `0b034ed`.** Both apps have already forked to `fork-thread`, but a later direct harness action supplies history from the original thread. Stream 11556 first changes rows/turns/rendering at event 14: old code adopts 16 foreign rows; new code rejects the history and keeps the fork's empty transcript. Stream 11205 first changes only dirty at event 9: both transcripts are empty, so rejecting the foreign empty history merely removes one redundant dirty(0). Both preserve calls. The owner deliberately fixed the mismatched-history bug this way; the full report's outcome already records that rejection as a new, intended difference. Ordinary callers adopt/check the thread before loading history; these direct harness actions bypass that path.

There are no sampled P cases. Class B contributes no remaining identity/row/call defect: four formerly B streams now have only stale-hint differences, and the fifth formerly B stream has H. The raw overlapping totals remain exactly the supplied ones: components 278, dirty 109, checkpoints 176, final facts 162, final rendered 5, calls 1. Final facts split into 144 status-only streams and 18 streams with other fact changes (13 F, four S, one visible T). The five final rendered differences are 352, 1432, 11556, 13492, and 13615; the only call difference is 1432.

The proposed runtime-settings explanation is refuted for this oracle: its `facts()` contains no current model/effort/summary fields, and both constructors use empty defaults. Commit `767db98` therefore cannot produce the extra 162 fact counts here. The traces contain no changed JSON field type or added canonical field either. The effective status value changed because the adapter combines owners incorrectly; H records a different set of temporary redraw calls. Neither is evidence of changed quota or runtime requests.

A future harness should compare the session status fact separately from the optional local hint, and use the rendered status line to compare what users see. If it needs the old single-string last-writer history, the harness itself should track that chronology. It should not restore unused production status_revision state. P4 makes no such code change.

## Required checks

The checkout keeps Rust 1.100.0 (0ef1bc386, 2026-11-09), with default features. Every required command passed; all seven exit codes are in `checks.json`.

- `cargo build`: passed.
- `cargo build --release`: passed.
- `cargo fmt --check`: passed.
- `bash scripts/check-rust-style.sh --all`: passed; all changed/new files stay below 400 lines and functions below 60 lines.
- `cargo test`, run 1: 897 passed, 3 ignored, 3.74 seconds in the test executable.
- `cargo test`, run 2: 897 passed, 3 ignored, 3.74 seconds in the test executable.
- `bash scripts/test.sh`: passed all 174 terminal workflows, plus its Rust/controller checks. The final terminal run took 77.457 seconds for the terminal tests. The earlier complete run also passed all 174. No timing test failed and no failure needed a retry.

Some cold command durations include test compilation; the actual Rust suites stayed below eight seconds. The production builds/tests print only the allowed existing protocol_writer.rs deprecation warning. Temporary harness builds also warn about unused diagnostic helpers; those helpers do not exist in the submitted tree. No Codex wire field/method changed, so the unrelated contract/schema proof was not repeated. No binary was installed.

## Speed checks

Seven old/new runs alternated on Rust 1.100.0 with default release features, after builds, harness runs, and test processes finished. The old release executable uses the starting commit's production code in the isolated previous tree; its temporary test adapters add only unused diagnostics. Both perf cases keep identical transcript counts. Times below are old/new median milliseconds; scroll/typing/working/append are p50.

- synthetic: replay 47.75/48.41, apply 15.09/15.30, layout 14.55/14.64, draw 3.46/3.44, scroll 0.71/0.70, typing 0.60/0.59, working 0.60/0.59, append 0.61/0.60.
- history: replay 1.11/1.13, apply 0.01/0.01, layout 0.01/0.01, draw 0.49/0.48, scroll 0.43/0.43, typing 0.37/0.37, working 0.37/0.36, append 0.39/0.39.

The checks add no per-frame, per-row, or per-delta transcript scan. The quota path still examines two windows. The app formats the same two short strings once per indicator read, and P1 removes unnecessary cache-key builds. Perf logs and all individual measurements live beside `perf-medians.json`.

## Paths outside the allowed list and remaining limits

- `src/app/notifications.rs`: six changed lines route existing quota messages/account clearing to the new owners.
- `src/app/codex_lifecycle.rs`: one call changes so closure clears both owners.
- `src/app/codex_reconnect_install.rs`: one call changes for the same reason at reconnect.
- `src/app/claude_quota.rs`: the existing shared weekly arithmetic caller must import the moved plain type and convert to the same colors. Its other quota logic remains untouched.
- `src/tests/protocol_stop_tests.rs`: P3 explicitly requires the one-line comment; this is outside the milestone's usage-test family.
- `src/tests/codex_reconnect_tests.rs`: the quota-specific test needs the new tick entry point. Listed here conservatively because the file is named for reconnect, although this case is a usage test.

The other files are named by the allowed list or are affected usage/settings/summary tests. No part 5a/5b method or unrelated part-5c helper changed.

For the plan: preserve the current retry rule (set the 300-second refresh before requesting, even when the request fails), the 60-second visual wake without reported quota, and the existing connection guard in the app. Moving that guard into a future daemon requires the backend connection facts to move first; this part adds no stronger disconnected-read guarantee. Claude's own quota timers/state remain for milestone 6; only its shared arithmetic call changes here. The pre-plan harness must stop treating sticky local hints as session status before its raw difference count can guide later work.

Nothing requested remains unfinished. The limits above describe what the harness does not exercise and which old behaviors this part intentionally preserves.
