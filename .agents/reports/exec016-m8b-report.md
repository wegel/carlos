# Milestone 8.2: native goals and local goal editors

The session now owns native goal knowledge/value/revision, read and write correlation, deadlines, direct-input capability, accepted changes, backend errors/notices and Ralph checks. The view keeps visibility, objective/budget editors, active field, scroll, local errors and explicit activation/clear confirmations. Every goal write still requires the original view-side consent before the session sends it. All native fields are private to the session tree and exposed only through borrowed/value reads and named commands.

## Handler audit

Tick/wait retain enabled/closed/pending-read checks, first-read behavior and the 30-second visible refresh. Visibility is passed in for that call and never stored in session facts. A read still uses the shared saturated token counter and the same epoch, timeout and root params. A failed read marks goal knowledge unknown and runs the same Ralph check.

The mutation gate retains closed/knowledge/direct-input/pending checks in that order, followed by the Ralph and pending-toggle check only when activation is possible. The app keeps draft conversion and confirmation; the session captures revision and whether the accepted request may activate a goal, sends the exact method/params and never retries an uncertain write. Failed activation makes knowledge unknown, advances its revision and requires a new read as before.

Replies still route only the goal operation, root and epoch; then match token, method and exact params. A read can apply only at its original revision. Mutation replies preserve unsupported/invalid error handling, the special activation-uncertain rules, the cleared-boolean behavior, and newer-native-event precedence. Only a successful reply for an objective-bearing request removes that editor. Backend read errors do not erase unrelated local draft errors; a new backend mutation error/clear reports the same visible last-writer effect to the view.

Goal parsing keeps the 128 KiB bound, root/objective/status checks, nonnegative counters, timestamp checks and positive-or-null budget rule. Native goal notifications retain their root filter and malformed-object rejection. Ralph still stops only at the same old call sites, retains queued humans and appends the same notice once. Reconnect invalidation merely marks the goal unknown and requests another read; it adds no new Ralph call. Close and root-switch commands preserve the old pending-work/editor rules.

The terminal prose builder was split into short functions because the changed body exceeded the repository function-size limit. Its line order, values and text are unchanged. The shared plain connection scope now has a neutral name, CodexScope, for queue and goal controllers; the remaining connection owner moves in milestone 9.

## Evidence and direct review

All 994 Rust tests pass, including the existing goal consent, pending-write freeze, editor recovery, native-queue precedence, idle timers and Ralph tests. A new session-only test asserts exact native get params, rejects a late read after a newer native update, checks the native goal/Ralph guard and preserves the direct-input refusal. All 174 terminal workflows pass in 79.253 seconds. Format, all-file style, whitespace, file size and function size checks pass. Release build/check and comparison/speed results are recorded in the ExecPlan; artifacts live in `/tmp/carlos-m8b/`.

The primary agent reviewed this part directly against `7135d25`, following the engineering reviewer checklist. The review caught an extra Ralph reconciliation in the first reconnect adapter and removed it. No remaining BLOCKER or MAJOR. Verdict: PASS (self-review, no subagent).

The inherited 3,000-stream comparison has no native queue/goal controls; those state transitions are exercised by the enumerated Rust tests and terminal peers above. No queue/goal mutation coverage is claimed for that corpus. The corpus still checks transcript/history/callback effects for all three backends against the parent and the pre-plan reference, and fails on any panic.

Additional launch/reconnect, mouse/render and thread-switch callers only use the moved fact/read or view paths. No native protocol method, dependency, persistent format or installed binary changed.
