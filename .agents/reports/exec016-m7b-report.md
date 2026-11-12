# Milestone 7.2: Codex callbacks without editors

The primary agent recovered the interrupted worker output from the named stash at `91570ea`. Every source hash equals the saved audit under `/tmp/carlos-m7b/`. The stash remains as a recovery copy. No agent was started during handover.

## What moved

`src/session/codex/` now owns typed callback keys, payloads, blocking flags, revision/candidate/card tokens, byte budgets, retired-id windows, approval parsing, question descriptions, backend replies and interrupts. `ViewState.codex_callbacks` owns drafts, focus and opened requests. The view reads the blocking flag from the session instead of copying it. Parser tests now sit beside the code they test.

## Handler audit

- `receive` keeps duplicate/recent-id checks before method, id, object, byte/count and owner checks. The session builds plain descriptions, inserts the record, queues its card and reports a schema preflight. The app creates its editor, applies the same blocking/focus rule and returns the preflight after activation. Payloads and limits are unchanged.
- `build_request`, supported methods and owner checks keep their old bodies. Approval parser bodies and answer builders match the previous source byte for byte. Defaults still populate widgets only in the view.
- `resolve` checks the owning thread before retirement. `retire` removes queued cards, retires the exact typed id, promotes only if active, then lets the view remove that editor and recompute blocking/opened focus. Reset still retires each live id; process reset also clears the retired window.
- `edited` increments with saturation and clears candidate/card/checking. `checked` keeps revision zero separate, does not clear a busy notice on preflight, ignores another revision, clears failed candidates and takes candidate before card on success. No new equality guarantee was added.
- `respond` still builds the exact native answer in the view. Cancel on a question interrupts its own turn. Accepted forms mark the card submitted before checking whether a check already runs; failed writes keep the draft. A deferred reply must resolve its original submitted card token.
- `interrupt` ignores missing/already-interrupting callbacks, requires string thread/turn, invalidates the candidate only after a successful request and waits for native resolution. Failed control replies touch only matching thread/turn editors.
- Route order, approval consent rules, the single card write point, native protocols, disk formats and transcript redraw algorithms remain unchanged. Additional callers only change field paths and borrowed reads.

## Evidence

The saved comparison covers 3,000 streams and 51,390 checkpoints, seed `0x167b02`, including 300 question and 300 form streams, 300 validation replies and 645 cancellations. All streams match the previous commit exactly; neither endpoint panics. Against pre-plan `a59221c`, 2,897 match exactly; the remaining 103 are the recorded H (92), T (7) and F (4) classes. One backend-call difference belongs to the recorded F close/fail rule. The saved ledger explains each case. The 470-sequence consent fixture passes unchanged.

The worker ran the new tests at both endpoints and passed build, release build, format, style, two full Rust suites and 174 terminal workflows. The resumed primary agent verified source hashes, reviewed the changed handlers against their parent, and passed 990 Rust tests in 3.76 seconds, format, style and diff whitespace checks. Resumed terminal and speed results are recorded in the ExecPlan. The sole compiler warning remains the existing atomic-method rename in protocol_writer.rs.

## Direct review

The primary agent applied `.agents/reviewers/engineering_reviewer.md` directly, as the owner prohibited subagents. No BLOCKER or MAJOR finding. The request-key wrapper preserves number/string spelling and both original byte bounds; the read accessors expose no mutable facts or editor types. Verdict: PASS. This is a self-review, not an independent review.
