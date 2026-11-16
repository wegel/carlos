# EP021b end-only Fable review

Session: b6e45425-07a2-486f-9aeb-cd04cc495b99
Model: claude-fable-5-2; effort medium; read-only; exit 0.
Cost: USD 2.7990306.

## 1. Summary

I reviewed the runtime changes in `4407f6e..cf50440` from `target/ep021b-review.patch`, the final report, and the surrounding source for the paths below. I read the code only and ran nothing. I skimmed the test sections by name, and did not read the Grok replay translator (`translate_text.rs`, `types.rs`), `event_rows.rs`, the plan documents or the source ledger.

- **Paths covered:** engine history worker, Codex and Claude window readers and their session adoption, prompt recall, the helper's window code, Grok permission callbacks and writer, tool facts, and the footer clock.
- **Overall:** the direction fits the end goal. Cursors, snapshots and callback ids stay inside the owner, there is no new wire family, and client/protocol production code is unchanged, so the phone gets these features for free.
- **Blockers:** none found.
- **Main concern:** the "append older rows" path is now a second, hand-maintained way of building session state. That is where future defects will come from, more than from the line count.

## 2. Findings

### BLOCKER
None.

### MAJOR

**M1. Older-window adoption duplicates the load reducers and keeps only a hand-picked subset of state.**
- **Codex:** `session/history_prepend.rs:34` (`restore_older_record`) is a near-copy of `session/history_receive.rs:33` (`restore_history_record`). The copy omits `panel_events::report` and prompt change tracking. This EP already had to add the same `describe_tool_facts` branch to both, plus `item_history.rs`.
- **Claude:** `session/claude/history_window.rs:74-124` replays facts into a throwaway `SessionState` and then moves across only rows, prompt ids and turns, records, and unknown prompts. Anything else the reducers produce there is dropped silently: turn status, completed-item ids, render overrides, task and sub-agent state.
- **Why it matters:** a session loaded through the legacy full reader and one loaded as newest window plus older windows can differ in everything except rows, and every future reducer field needs a manual merge line. The checks I saw compare rows (2000 exact); I did not find one that compares whole session state.
- **Not a defect today:** I checked steer-row kind and per-turn rewind positions, and both come out correct on the older path.

**M2. A second concurrent older read fails instead of waiting.**
- **Where:** `engine/history.rs:66` returns `full()` when `history_busy` is set or two workers are busy. The error is -32010, "session command queue is full or closed; nothing dispatched" (`engine/mailbox.rs:88`).
- **Trigger:** this is the normal two-device case, for example the phone auto-paging while the terminal user presses Up for prompt recall. One of them gets a misleading error.
- **Roadmap cost:** every client (terminal, Swift, web) would need its own retry policy. I did not check whether the terminal retries today.

### MINOR

**m1. An older Codex window that adds no rows can produce an empty page with `has_older` true.**
- `engine/history.rs:121` re-reads only for `Prompts`; a `History` request reads exactly one window.
- `protocol_history_window.rs:146` skips only turns with empty `items`, not turns whose items render no rows or are all removed as duplicates in `native_history.rs`.
- `Published::history` (`session/event_reads.rs:24`) then returns no entries with `has_older` true, which the client rejects as `StaleHistory` (`carlos-client/src/history.rs:73`).
- The next request probably recovers, but the phone would show a spurious error.

**m2. A failed Grok interrupt leaves permission cards dead.**
- `grok_backend/interrupt.rs:20` marks the callbacks as cancelling before `session/cancel` is written at lines 27-31. If that write fails, the `?` returns before `cancel_callbacks` at line 32.
- The same happens in `InputWriter::cancel_callbacks` if `enqueue_frames` fails after `cancel_session` (`process_callbacks.rs:141-162`) has set the flags.
- Result: no retired event is sent, the card stays visible, a later answer is refused as retired, and Grok gets no reply until another interrupt or the turn ends.
- This fails closed and is recoverable, but the report itself records an unexplained 500 ms writer timeout, so the path is reachable.

**m3. The Grok reader thread can block on the writer.**
- `process_client.rs` `receive_agent_request` calls `write_callback`, which waits on the writer, for the invalid-session, retired and unsupported-method replies.
- `cancel_callbacks` carries the comment that the reader "never waits for the native pipe", so the two paths disagree.
- If Grok is itself blocked writing output, the reader stalls and the request fails; I did not trace how the reader loop handles that error.

**m4. Display policy is landing where the phone cannot share it.**
- `tool_facts.rs:33-41` builds the English string "N lines" in Core. It reuses an existing field, but Swift cannot localise or reformat it.
- `src/app/remote/status.rs` `activity_label` filters the "requesting" label in the terminal only, so the phone would have to copy that rule or show the flash again.
- `tool_facts.rs:78-87` infers tool kind from a list of tool names (including the odd `"semeja"`). Core is the right place for it, but it needs one table test so it does not drift.

**m5. Some defensive code could be trimmed (optional).**
- `claude_backend/history_window.rs:134-222` re-checks every window field from the helper that ships in the same package, and `session/claude/history_window.rs` `initial_cursor` checks them a third time.
- `native_history.rs:18-26,155` keeps cursor and turn-id sets with a 64 MiB metadata budget against the Codex server.
- `process_callbacks.rs` adds a blake3 fingerprint to detect identical redelivery of a request.
- None of it is wrong, and each piece maps to a named hazard. Roughly 150 to 200 lines could go if you trust your own helper at that boundary. The Grok token, epoch and dispatched/cancelling machinery is heavy but justified by the cancel-versus-answer race it tests.

**m6. Loaded older rows are never evicted.**
The transcript and the Codex turn-id set grow as devices scroll back. It is bounded by the existing caps and fine for now; note it as a carried limit alongside the Claude SDK scan.

## 3. Corrective guidance

- **M1, before EP022 consumes paging:** add one differential test per backend that loads a fixture through the full reader and through newest-plus-all-older windows, then compares all exported state (snapshot, tasks, prompt selections, tool facts, turn status), not just rows. Then make `restore_older_record` and `restore_history_record` share one function with a mode flag. For Claude, either list in a comment and test which scratch state is deliberately discarded, or merge it.
- **M2:** when the in-flight read has the same cursor, hold the second request in a small bounded waiter list and answer both from the one accepted window. At minimum, return a distinct retryable "history read in progress" error and retry once in `carlos-client`, so no front end has to.
- **m1:** in `finish_history_read`, re-read for `History` too when the accepted window added no rows and a cursor remains. The existing `ReadScope` deadline and byte allowance already bound this.
- **m2:** if the cancel write or enqueue fails, clear the cancelling flags again, or enqueue the cancelled replies and send the retired events regardless.
- **m3:** use the non-waiting enqueue for those reader-side rejections, as `cancel_callbacks` does.
- **m4:** move the "requesting" suppression into Core's activity label. Plan a numeric line count when the wire next changes, without adding a field now.

## 4. Verdict

**PASS WITH ISSUES.** No blockers. M1 and M2 should be fixed or pinned by tests before EP022 builds real screens on paging; the minors can be scheduled.

