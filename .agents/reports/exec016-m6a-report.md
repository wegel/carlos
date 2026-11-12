# Milestone 6, Part 1 report

Implemented Part 1 only. The four-part split was written to `/tmp/carlos-m6-split.md` before source edits. Part 1 moves Claude phase, native model metadata and inheritance rules, quota facts/read deadlines, and context reads into `src/session/claude/`. The app keeps the local settings snapshot and quota redraw timer in ViewState. Parts 2–4 remain unfinished by design and their behavior is unchanged.

Starting commit: `253e3ddf0b1b6755b76dd8c111e547f3a46675b1`; branch: `daemon-split`. The owner checkout was never accessed. No commit, push, stash, reset, install, dependency change, toolchain change, or edit to `.agents/`, `docs/`, or `src/claude_backend/**` occurred. Rust remains `1.100.0 (0ef1bc386 2026-11-09)`.

## Per-file changes

- `src/app/backend_setup_launch.rs`: Call the session method for the existing Starting phase and initial inheritance rules after history loads.
- `src/app/claude_context.rs`: Pass plain activity facts and the existing revision to the session reader; retain activity checks and revision ownership.
- `src/app/claude_control_replies.rs`: Send successful optional-quota reports to the owner with the unchanged missing-revision default of zero.
- `src/app/claude_edit.rs`: Read and set session phase at the same edit boundaries; leave edit text, UUIDs and recovery code untouched.
- `src/app/claude_models.rs`: Keep the edited-prompt/composer tail and editor snapshot in the app; delegate metadata and inheritance rules.
- `src/app/claude_notifications.rs`: Read session phase in existing event guards and set Closed at the old failure point.
- `src/app/claude_quota.rs`: Keep only countdown redraw timing and the ready/backend guard; delegate optional reads to session facts.
- `src/app/claude_reset.rs`: Use session phase accessors at the existing reset/init points; preserve all reset guards.
- `src/app/claude_sdk_events.rs`: Apply quota stream facts through the session method.
- `src/app/claude_session_history.rs`: Call session switch-choice rules before the old thread reset; reset moved metadata/read facts and the local quota timer at the old Claude reset point.
- `src/app/claude_state.rs`: Remove moved fields and the mixed ClaudeSelection; re-export the session phase enum for existing callers.
- `src/app/claude_task_view.rs`: Borrow quota windows through read-only accessors; retain labels, expiry filters and iteration order.
- `src/app/input.rs`: Call the quota view bridge at the old loop position.
- `src/app/input_events_keys.rs`: Read session phase in the existing closed-helper key branch.
- `src/app/input_events_model_settings.rs`: Clear the settings snapshot in ViewState at the old close point.
- `src/app/input_queue_rules.rs`: Read session phase in the existing submission gate.
- `src/app/input_timers.rs`: Merge the quota countdown and backend-read deadlines through the view bridge.
- `src/app/mod.rs`: Register the two new Claude parity test modules.
- `src/app/render_status.rs`: Convert the same plain weekly facts to the existing styled indicator; read context freshness from the session.
- `src/app/turn_submit.rs`: Read session phase in the same submit/queue/closed-helper branches.
- `src/app/view_state.rs`: Own the Claude quota redraw deadline and the captured settings-editor pair.
- `src/session/claude/context.rs`: Own outstanding context reads, freshness, model cache and deadlines; preserve token/revision/default/send-failure rules.
- `src/session/claude/mod.rs`: Declare the metadata/read concerns and expose the plain phase and facts types.
- `src/session/claude/model_choices.rs`: Own startup/switch inheritance, prompt filtering and saved-choice rules.
- `src/session/claude/models.rs`: Own native capabilities, observed models, effort checks, aliases and model catalogs.
- `src/session/claude/quota.rs`: Own native quota windows/revisions/read deadlines and return plain weekly allowance facts.
- `src/session/claude/quota_updates.rs`: Expose explicit session methods for quota reports and optional reads.
- `src/session/claude/state.rs`: Define private Claude metadata/read fields, defaults, borrowed accessors and phase/reset methods.
- `src/session/claude/test_support.rs`: Replace former direct test writes with three test-only fixture methods.
- `src/session/mod.rs`: Declare the Claude session module.
- `src/session/state.rs`: Own and initialize the private Claude facts group.
- `src/tests/approval_lifetime_support.rs`: Set session phase in the existing shared Claude approval fixture.
- `src/tests/claude_backend_tests/review_receiver_tests.rs`: Read Closed from the session in the existing receiver checks.
- `src/tests/claude_context_tests.rs`: Use session accessors and fixture methods for the existing context cases.
- `src/tests/claude_edit_tests.rs`: Use phase accessors/setters in the existing edit/recovery cases.
- `src/tests/claude_metadata_parity_tests.rs`: Add nine tests enumerating model/effort/default/editor/switch/launch cases with literal expected values.
- `src/tests/claude_native_state_tests.rs`: Read session phase in the existing native-state assertions.
- `src/tests/claude_quota_tests.rs`: Use the session owner and borrowed windows; retain every assertion, including the exact 60% display string.
- `src/tests/claude_read_parity_tests.rs`: Add five tests enumerating context correlation/default/availability and quota timer/error/guard cases.
- `src/tests/claude_sdk_tests.rs`: Read/set session phase in the existing reset/clear cases.
- `src/tests/claude_session_tests.rs`: Read moved metadata/phase/inheritance and seed inheritance through the owner.
- `src/tests/claude_visibility_tests.rs`: Read session phase in the existing visibility assertion.
- `src/tests/input_history_split_fork_tests.rs`: Set session phase in the existing Claude fork fixture.
- `src/tests/input_status_tests.rs`: Seed freshness and quota through session methods in the existing Claude status cases.

## Parity ledger (old method: preserved rules; current owner)

The starting reference is 253e3ddf0b1b6755b76dd8c111e547f3a46675b1. Each entry names the old guards, early returns, defaults, ignored errors and order. A missing error/dedup branch means the old method had none; the move adds none. This ledger uses lists to fit narrow screens.

- `ClaudeUiState::default` / `ClaudeSelection::default`: Ready, absent capabilities/observation, both inheritance flags true, empty quota/context readers; unchanged session defaults in `claude/state.rs`. The app retains enabled=false, epoch=0, generation=1 and all later-part defaults. `ViewState` starts the editor pair and countdown at None.
- `claude_models::apply_capabilities`: Closed returns before any write or edit dequeue. Pending model precedes current. Explicit means previously explicit OR pending model OR first metadata with requested != observed. Support means exact native value/resolvedModel OR exact observed value. Unsupported requested model clears only pending model, then current/pending effort and sets effort inheritance; model inheritance becomes !supported OR !explicit. Record selected choice, validate effort, store the same capability clone, rebuild models, mark Ready only when !awaiting_reset_init, set "Claude ready", then process edited text. Session `apply_claude_capabilities` owns all metadata/settings writes and returns false for Closed. App bridge keeps the original edited-text tail: take once; retire queue; mark edit queued; clear rewind; clear composer only if equal; enqueue text with history. No new capability check or reset guard.
- `observe_model`: invalidate context only when a previous observation exists and differs; invalidate before replacing the observation. Record a model/refresh catalog only if both current and pending model are absent; native alias when metadata exists, exact string otherwise. App bridge keeps the changed-observation guard and invalidation; session `observe_claude_model` keeps the native observation, choice guard, alias and catalog writes. Repeated observation does not invalidate; an explicit choice is never replaced.
- `validate_effort`: no known current model returns; pending effort precedes current; absent/auto returns. Invalid only when supportsEffort==false OR a supplied array lacks the exact effort string. Missing arrays do not invent a restriction. Invalid clears current and pending effort and restores inheritance. Same session helper in `claude/models.rs`.
- `prompt_choices`: independently omit model/effort only when that field inherits; preserve supplied options and bytes otherwise. Session `claude_prompt_choices`; app bridge delegates.
- `begin_settings_edit`: capture model/effort editor strings only when no pair exists, in model-then-effort order; reopening does not overwrite the pair. Same app method, now `view.claude_settings_edit`. The existing settings-close branch still clears the pair at the same point in `input_events_model_settings.rs`.
- `prepare_saved_choices`: no editor pair returns without writes. Only a present default differing from its captured string counts as changed. Inherited unchanged model becomes None and clears pending model only; otherwise mark model explicit. Inherited unchanged effort OR absent/auto default becomes None and clears both current/pending effort; otherwise mark effort explicit. Model rule precedes effort rule; summary untouched. Session `prepare_saved_claude_choices` owns these rules; app supplies a borrowed captured pair and the same defaults object. No editor state is stored in session facts.
- `refresh_models`: absent metadata returns. Missing/nonarray models yields no rows. Skip a row without string value; selected resolved id replaces its alias only on exact match. Display label falls back to value; default_effort=None, is_default=false. Insert selected model at index 0 only if missing, with [auto]. Set effort capability iff any retained model has a non-auto level, summary=false, then set models through the old stable sort. Same session helper `refresh_claude_models`.
- `native_efforts`: retain string entries in native order; supportsEffort=false, missing/empty/no-string list yields [auto]. Same session free function.
- `find_model`: absent/nonarray models returns None; first exact value OR resolvedModel match wins. Same session free function.
- `native_alias`: first native matching row's string value, else exact observed string; no trim. Same session free function.
- `configure_claude_app` (split startup choices): existing history load must succeed before enable/phase/inheritance/status writes. Session `start_claude_settings` sets Starting, inherits model iff acknowledged model absent, inherits effort iff absent OR exactly auto. App keeps load/enable/status ordering. It does not reject raw choices or add a Closed guard.
- `switch_session` (split settings/reset fields only): caller still accepts only a newer generation with id; retire queue first; take unsent drafts and edit; discard acknowledged inherited model/effort only when the corresponding pending field is absent; reset thread; reset Claude facts; clear Claude approvals and matching confirmation; enable; restore generation/Starting and saved edit/drafts; clear catalog; set old status. Session `switch_claude_runtime_choices` owns both pending-choice rules at the same pre-thread-reset point. `reset_claude_metadata` resets the moved group at the old ClaudeUiState reset point while retaining both inheritance flags. App keeps the editor pair in place (old code saved/restored it) and clears `view.claude_quota_next_draw` at that reset point. All prompt/history/approval/recovery steps remain app-side.
- `claude_context::refresh`: mark freshness false, retain counts, pending slot, deadline and model. Session `refresh_claude_context`.
- `invalidate`: increment activity.context_revision, clear counts, clear next_read, then mark stale; pending read and cached model remain. App keeps revision for Part 2 and calls session `invalidate_claude_context` for the last three writes, in the same order.
- `tick`: availability false returns false before touching model/read state. On current-model change: save whether counts existed for redraw; replace cached model; increment revision; invalidate counts/deadline/freshness. Fresh idle/no pending returns accumulated redraw. An outstanding read never overlaps another read; after >=5s mark stale, return redraw OR stale transition, retain pending. A future next_read returns accumulated redraw. Otherwise increment sequence; install token/sent pending slot; set next_read=now+1s; request context with current revision/token. Ignore error details, but on send failure clear pending, back off 5s, include prior freshness in redraw, then mark stale. Session `tick_claude_context` retains this exact order; app supplies plain availability/working and borrows the existing activity revision by mutable reference. No second model comparison or new polling loop.
- `wait`: unavailable -> None; fresh idle/no pending -> None. Pending uses sent+5s only if still future, otherwise None (no endless wake). No pending uses next_read or now. Return saturating duration. Session `claude_context_wait`; bridge supplies the same plain predicates.
- `receive`: a supplied numeric contextToken requires exact pending token, else return before settling. Missing/non-numeric token skips that guard. Set pending=None before revision test. Supplied numeric mismatched revision clears next_read and returns without restoring counts/freshness. Missing/non-numeric revision skips that guard. Set deadline from Instant::now()+1s on ok==true, otherwise +5s; rawMaxTokens numeric value precedes maxTokens even when raw is zero; require numeric used and positive max and ok==true before replacing counts/marking fresh/returning. All other replies mark stale while retaining known counts. Session `receive_claude_context`.
- `available` / `working`: still app predicates until process facts move: enabled AND Ready AND transient != exact "Compacting"; active turn OR native state != Idle. Tick/wait bridges read them, with no state writes. No backend-kind restriction was added to context reads.
- `ClaudeQuota::tick`: client kind Claude AND ready, else false before timers. Advance next_draw by 60s iff absent/due; advance next_read by 300s iff absent/due before sending usage.optional with current revision; ignore send failure entirely; return only redraw due. App `claude_quota::tick` owns guard/countdown; session quota tick owns backend deadline/request. The app loop still calls it after the same prior optional work.
- `ClaudeQuota::wait`: earliest draw/read deadline, each saturated at now; no availability guard added. App `claude_quota::wait` merges the view countdown with session quota wait; choosing minimum after saturation gives the same duration as choosing the earliest timestamp first.
- `event`: increment revision even if input is unusable; apply top-level fraction only for a valid key; then iterate unifiedWindows object in its existing map order, accepting valid keys only. Same session quota method, reached through `claude_quota_event`.
- `valid_key`: exact five_hour, seven_day, seven_day_opus, seven_day_sonnet, seven_day_overage_included, overage; nothing else. Same session free function.
- `fraction`: need numeric utilization and i64 reset; accept only inclusive 0..1; label replaces underscores with spaces; multiply by 100; insert at current stream revision. Same session quota helper.
- `optional`: unavailable report (report.rate_limits_available==false) retains windows only with newer revisions, then returns. Otherwise optional windows array; skip missing string key; skip bucket whose current revision > requested revision; require numeric usedPercent and i64 resetsAt; missing/nonstring label defaults to key. Unlike stream reports, optional reports accept arbitrary bucket names. Same session method through `claude_quota_optional`.
- `insert`: reject key/label >4096 bytes, nonfinite/outside inclusive 0..100 used, reset <= unix_now; reject a new key when already 128 buckets, while allowing updates. Store exact label/used/reset/revision. Same session helper; no pruning or tighter bound added.
- `indicator`: only seven_day; round used to u64; build the same RateWindow with WEEK_MINUTES/reset; evaluate at unix_now. Session returns plain WeeklyUsage; `render_status` applies the same existing WeeklyIndicator conversion to choose text/color. Task view reads borrowed window accessors and keeps its expired-window filter and wording.
- `claude_control_replies::{receive,failed}` and `claude_sdk_events::receive`: only the quota write path changed. Failed optional usage remains ignored; successful optional usage still defaults missing revision to 0; failed context still settles via the same receive bridge; start/capabilities still return ApplyClaudeSettings even if the metadata handler ignored Closed. All dispatch order and other guards stay app-side.
- Mechanical phase callers (`claude_notifications::{route,can_apply,fail_session}`, `claude_reset::{receive,initialize}`, `claude_edit::{begin,failed}`, `turn_submit`, `input_events_keys`, `input_queue_rules`) retain their code branches and ordering; only reads/setters change. In particular: fail_session still recovers before Closed; failed edit never reopens Closed; duplicate reset key still returns; init requires awaiting reset and a supplied session_id; closed/generation/epoch exceptions remain verbatim. The backend normalizer, translator and result/idle tracker are byte-for-byte unchanged.

New accessors and owner commands do no translation, transcript traversal or redraw work. Test-only fixture setters replace direct writes that the old tests made; they add no production API behavior.

## Tests added and comparison with the starting code

Fourteen new tests pass against both endpoints: nine model/settings tests and five read/timer tests, each with 0 failures and 0 ignored. `enumerated-tests.json` records both test binaries' distinct hashes and results. The previous endpoint uses only test adapters for the old field paths and the old quota tick/wait signatures; its production rules remain at the starting commit. Expected values are explicit examples rather than calculated copies of the implementation.

- Model metadata: no choice, supported alias, resolved id, unsupported model, unsupported effort, native observation, custom observed model, auto effort, and pending model/effort precedence.
- Catalog: skip malformed rows; preserve resolved id and display label; keep only native string effort levels; fall back to auto for missing/empty/unsupported lists.
- Phase/reset: Closed ignores metadata; Starting becomes Ready only when no reset init is outstanding; Ready remains Ready while waiting for reset init.
- Observation: a changed native model invalidates context once; current/pending explicit models survive; repeated observation does not invalidate again.
- Editor saves: absent snapshot, inherited unchanged values, independently changed values, explicit choices, auto effort, and unchanged summary.
- Switch: inherited versus explicit acknowledged values, with and without pending overrides; keep the local editor pair, reset metadata and enter Starting.
- Launch/prompt: missing model and missing/auto effort inherit; model and effort filtering remain independent for all four flag pairs.
- Context: exact/wrong/missing/malformed token and revision; obsolete reply releases its slot; invalid reply retains known counts; raw max wins even when zero; unavailable/Starting/Closed/Compacting readers neither send nor wake.
- Quota: exact 60s draw and 300s read deadlines, including send failure; unchanged revision in requests; no report/error row on optional write failure; Codex/Grok/Starting/Closed guards.

Existing tests retain their cases and expected values. The quota test still checks the exact "60%" display string through the same styled conversion, as well as raw percentages and sparse revision behavior. No test was removed or weakened. No user workflow broke, so no terminal fixture was added.

## Fixed native-Claude sample

The sample contains **3,000 Claude streams**, drawn without replacement from the 8,100 Claude streams in `target/m4-fix/streams.jsonl`, seed **1466369 (`0x166001`). Corpus SHA-256: `0ff34c257a5acb7d0e5f787f4057fa03a8c757577d5091f5d58bcc4890dcfeb8`. `sample-manifest.json` lists every selected id; `sample.jsonl` and six 500-stream inputs preserve the exact sample. Each endpoint handles **38,041 event checkpoints**, renders at 160×48 and 60×20, records backend calls and redraw calls, and captures panics. No stream panicked, including either reference.

- Previous commit: **3,000/3,000 exact**, including every checkpoint, final session facts, moved metadata, accepted choices, composer/history facts, backend calls, redraw log and rendered lines. The stronger oracle also reads capabilities, observed model, inherited prompt filtering, quota windows and context freshness. Modern status reads use only `session.status()`.
- Pre-plan `a59221c`: **2,866 exact**, **134 within recorded classes**: H=101, F=28, S=5, P=0, T=0. No additional class or panic. Three backend-call differences belong to the recorded F cases. `comparison.json` lists every differing stream and its recorded class.
- Independent stored-report check: all 3,000 modern results match the saved `9587748` report endpoint for every final fact except the separately observed status, every backend call, final screen, redraw log, and every checkpoint's rows/render/screen/dirty hashes. `prior-report-components.json` records **0 differences**. This checks that merely finding the same stream id in the old manifest did not hide a new screen or backend difference.

The status reader needed one careful repair. The saved pre-plan binary reads `app.status`, which mixed backend facts with key/editor hints before the split. Comparing that mixed field to `session.status()` gave 230 raw differing streams, including 96 extra status-only streams. Keeping the old app behavior unchanged, a temporary pre-plan observer now records only backend status writes and exposes `session.status()` to the oracle. It still performs every original write to `app.status`, so the old renderer and all old decisions see exactly the original value. The observer does not read any modern value or a view hint.

`preplan-status-mapping.json` lists the 46 old hint call sites matched to their current counterparts. A conditional observer rule distinguishes the old shared approval-error helper's backend-send failure from its local form error, exactly as the current code does. `preplan-core-status.rs` and the patched temporary pre-plan tree preserve the observer. All 3,000 streams were replayed through it. Compared with the original saved pre-plan binary, all final nonstatus facts, backend calls, screens, redraw logs and every checkpoint's row/render/screen/dirty hashes match exactly; the observer changes only the compared status in 98 streams, two overlapping known classes. `preplan-observer-check.json` records this proof. The original raw-status results and the failed intermediate observer checks remain under `/tmp/carlos-m6a/`; no production status rule was changed to make the comparison pass.

The native harness does not run the periodic quota/context timer loop, open settings widgets, or cover every launch/switch combination. The fourteen enumerated tests cover those paths on both the previous and current code. The sample also does not enumerate reader/UI thread schedules or append an extra EOF to truncated input. This part changes no backend normalizer or thread boundary; the full milestone 3 schedule proof and unrelated Codex/Grok corpora were not rerun.

## Checks

- `cargo build`: passed, final build 4.38s; default features.
- `cargo build --release`: passed, final build 48.84s; default features.
- `cargo fmt --check`: passed again after the final test edit.
- `bash scripts/check-rust-style.sh --all`: passed again after the final test edit.
- `cargo test` (cargo-test-final-1): test result: ok. 911 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 5.54s; command wall 5.771s.
- `cargo test` (cargo-test-final-2): test result: ok. 911 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 3.76s; command wall 3.922s.
- Two earlier full Rust runs also passed: 911 passed, 3 ignored, suite times 3.74s and 3.75s; command walls 3.894s and 3.906s. The final two runs follow a test-only edit that retained the old exact quota-display assertion. Test runtime stays below 8s.
- `bash scripts/test.sh`: passed once, command wall 111.257s. It passed 911 Rust tests/3 ignored, both 59-test controller runs, package/recorder checks, and **174 terminal workflows** in 78.804s. This suite exercised the same production code as the final tree; the later edit changes only that quota test assertion.
- `git diff --check`: passed.
- File/function check: maximum changed file 400 lines; maximum function 60 lines; no over-limit function or glob import.
- Session boundary scan: no terminal crate, `crate::app`, `Deref` or `RefCell` reference in the new Claude session modules; session fields are private or `pub(in crate::session)` and app writes use owner methods.
- No intermittent Rust/terminal failure occurred and no timing-test retry was needed. Builds did not overlap the final full Rust or terminal runs. Only the existing deprecated `fetch_update` warning in `protocol_writer.rs` remains; no new compiler warning.

The source does not add a transcript-length loop, per-row work, per-delta work, or a large-facts clone. Model strings and the capability snapshot keep the old copy points. Existing redraw calls and cache invalidation remain unchanged and the previous-commit oracle matches their complete log. No transcript/render algorithm changed, so unrelated performance runs were not repeated.

## Small edits outside the allowed list

Milestone 6 explicitly permits the smallest necessary caller exception. The following seven files are reported conservatively even where a shared test contains Claude cases:

- `src/app/backend_setup_launch.rs`: replace the old phase/inheritance writes with one owner call; otherwise it would write removed fields.
- `src/app/input.rs`: replace the old quota tick call with the view/session bridge; the timer's old position in the loop stays fixed.
- `src/app/input_events_model_settings.rs`: clear the moved local editor pair at the existing close point.
- `src/app/input_queue_rules.rs`: read phase from its new owner in the existing gate.
- `src/tests/approval_lifetime_support.rs`: update one Claude phase fixture write.
- `src/tests/input_history_split_fork_tests.rs`: update one Claude phase fixture write.
- `src/tests/input_status_tests.rs`: use owner fixture/update methods for two Claude status cases.

All other edited files are within Milestone 6's allowed paths. None of these exceptions changes a decision, key, request, format or file path.

## Discoveries, decisions and remaining work

- The app no longer decides native inherit or pending-choice switch rules through combinations of generic runtime setters. `model_choices.rs` owns those rules, completing the named earlier review item for Part 1.
- The quota reader owns backend deadlines; the view owns the minute redraw deadline. The old quota wait still wakes for a stored deadline even if the helper is no longer ready; no stricter readiness rule was added.
- Activity and reset guards stay with the later parts. For now the context bridge supplies plain availability/working facts and a reference to the existing context revision. The session increments that revision at the same old model-change point, after caching the new model and before invalidating/sending. Part 2 can move the revision and activity predicates together without undoing this reader.
- The old metadata handler still sets "Claude ready" while a reset init blocks the Ready phase; the control-reply handler still returns ApplyClaudeSettings when Closed metadata was ignored. The old tests and sample agree; these quirks were not tightened.
- A context reply without numeric token/revision still bypasses those checks; rawMaxTokens=0 still prevents fallback to maxTokens; optional quota reports still accept names the stream allowlist rejects. A future daemon may want stronger provenance or stricter read rules, but this part adds none.
- The final pre-plan proof must compare backend status facts rather than the old mixed UI/status field. The independent old-render proof above makes that observer change reviewable. Reusing a view hint, or blindly ignoring all status mismatches, would conceal different errors.
- No behavior issue remains unresolved for Part 1 within the stated tests/sample. Parts 2–4, the remaining mixed ClaudeUiState fields, generation/epoch/reset ownership, accepted prompt/edit ownership, task/browser/process ownership and history records remain for their written parts. This report does not claim that a finite mutation sample proves every possible stream.

## Patch and reproduction

`/tmp/carlos-m6a/part1.patch` contains all **44 changed files**, including **10 new files**, with no staged changes. It is **114634 bytes**, SHA-256 `cf03cdf08334d5a98ff1c11f1555483a32cfcb740ceadf4138b342109016f8a6`. Apply it to the starting commit with `git apply /tmp/carlos-m6a/part1.patch`.

The patch passed `git apply --check`, was applied to a fresh archive of the starting commit under `/tmp/carlos-m6a/patch-proof/`, and matched all **1223 repository/new-file bytes** against the final working tree. `patch-proof.json` records the result. The scripts, endpoint binaries, original raw-status outputs, corrected observer, sample manifest, comparison outputs, check logs and parity ledger remain under `/tmp/carlos-m6a/`. No temporary oracle hook was added to the working tree.
