# Milestone 7.3: Claude callback commands and plain replies

The primary agent implemented and reviewed this part directly. No subagent or external reviewer ran.

## What changed

`src/session/claude/callbacks.rs` accepts requests, creates their cards and owns the original prompt/detail builders. `callback_questions.rs` holds the unsupported-kind rules and plain field descriptions. `callback_reply.rs` sends converted answers and retires cards only after success. Browser consent and completion now append their notices in session methods and return the exact transcript update. `src/session/approval_reply.rs` retains the existing Codex-first retirement rule and reports which editor the view must discard.

`ViewState.claude_answers` holds every unsent answer. The obsolete `ClaudeUiState` wrapper and its module are removed. Callers and tests now import `ClaudePhase` directly from the session. The app keeps draft conversion, field defaults, selection, error scroll, default-no confirmation and keyboard hints.

## Handler audit

- `receive_callback` still ignores nonstring ids, builds descriptions before checking kind and warns for unsupported kinds before checking duplicates. Supported duplicates compare exact native ids across the pending and Claude queue. Browser registration precedes card adoption/queueing. Only an accepted callback creates an editor and registers the original field hint.
- The prompt and detail builders moved verbatim. Permission title/displayName and elicitation title/message fallbacks, server/source details, ExitPlanMode text, JSON tool input and URL-first order remain the same.
- Question descriptions preserve the missing/blank-plan check; AskUserQuestion only for permission; non-url elicitation only; formIssue before schema parsing. The view still applies MCP defaults in order. Native answer conversion did not change.
- `cancel_callback` removes only a string-id editor; the view clears confirmation only for a matching live card. The session withdraws that browser id, retires matching queued cards and promotes only when the active id matches. Unknown and nonstring ids keep the original behavior.
- `clear_pending_approval` still gives a live Codex callback priority; otherwise it removes only the pending string-id editor and promotes the Claude queue. The report carries ids, never editable widgets or cards.
- `respond_to_pending_approval` keeps Codex dispatch, answer conversion, unavailable-choice fallback and default-to-no checks in their original order. The session then sends the plain answer, records browser consent, retires/promotes and sets the original status. Failed writes preserve every card/draft and set the same escaped status and draft error. No native answer is sent before confirmation.
- Browser registration, cancellation, once-only completion, early-completion notices and the distinction between consent and completion stay unchanged. `/clear` retains unresolved process requests. The recorded old-session queued-card defect was not changed.

## Checks and review

The unchanged 470-sequence consent fixture passes. Two new headless tests prove exact card promotion/deduplication and unsupported/default precedence using SessionState without AppState or any widget. All 992 Rust tests pass; all 174 terminal workflows pass in 78.209 seconds. Format, all-file style, function/file size and whitespace checks pass. The changed functions stay below 60 lines and files at or below 400.

An initial allocation test found that the reply method cloned the native payload twice. The session now borrows the active card and splits its browser/card borrows; the existing per-backend/per-choice allocation test passes without changing its expected counts. The native payload is copied only once for the view's answer builder, as before.

The primary agent used the engineering reviewer prompt to inspect guards, failed writes, fallback values, status/hint precedence and borrow costs against parent `729efb3`. Verdict: PASS, no BLOCKER or MAJOR. This is a self-review. Terminal tests and the live-event comparison provide independent executable checks, not an independent human/model review.

The comparison artifacts are under `/tmp/carlos-m7c/`; the ExecPlan records the completed result and speed check. The same fixed 3,000-stream corpus covers all three backends, callbacks, questions, forms, cancellation and validation replies. No production backend, disk format, redraw algorithm or test expectation was weakened.

The additional changed callers outside milestone 7's short list only replace the removed ClaudeUiState/ClaudePhase paths: Claude history/quota, turn submit/queue guards and existing phase fixtures. These paths do not change their decisions.
