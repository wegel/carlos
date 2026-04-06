# Add a Claude Code backend to carlos without regressing the Codex path

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, `carlos` can run against either the existing `codex app-server` backend or a new Claude Code backend selected with `--backend claude` or `CARLOS_BACKEND=claude`. A user with a working `claude` CLI installation should be able to start a new Claude-backed session, resume a Claude session by explicit session id, or continue the most recent Claude session, and then use the same TUI to watch streamed assistant text, see tool activity, interrupt a turn, and read basic context-usage feedback.

This work matters because `carlos` is already a useful terminal shell around the internal `AppState` model and renderer, but today it is hard-wired to one JSON-RPC server implementation. The goal is not to fork the UI into two modes. The goal is to keep one TUI and one transcript model while translating Claude’s newline-delimited JSON stream into the Codex-shaped notifications that `carlos` already knows how to consume.

## Progress

- [x] (2026-04-06 13:08Z) Created this ExecPlan from `carlos-claude-backend-prompt.md`, grounded it in the current repository structure, and registered it in `PROGRAM_PLAN.md`.
- [x] (2026-04-06 13:16Z) Introduced `src/backend.rs`, implemented `BackendClient` for `AppServerClient`, and routed the shared startup/event/input paths through the trait-backed surface while intentionally leaving the Codex-only picker/archive flow concrete in `src/app/terminal_ui.rs`.
- [x] (2026-04-06 15:04Z) Prototyped the Claude launch contract against a real local `claude` CLI and confirmed the stdin stream-json envelope, the `system/init` session payload, the top-level tool-result event shape, and the practical behavior of `--continue` / `--resume`.
- [x] (2026-04-06 15:31Z) Implemented `src/claude_backend.rs` with a `ClaudeClient`, a stream-json subprocess launch path, synthetic start metadata, and an NDJSON-to-synthetic-JSON-RPC translation layer for streamed text, tool calls, tool outputs, turn lifecycle, interruption, and token usage.
- [x] (2026-04-06 15:39Z) Integrated CLI/backend selection, Claude-specific bootstrap rules, queued-next-turn behavior in place of unsupported mid-turn steer, a Claude model catalog, and user-facing guards for unsupported picker resume / archive / runtime-model editing flows.
- [ ] Finish closeout for the current implementation slice: collect the required engineering review, resolve any blocking findings, commit the Claude adapter slice, and then move this ExecPlan to `.agents/done/`.
- [x] (2026-04-06 16:28Z) Re-ran full automated validation after fixing the startup handshake mismatch: `cargo test` now passes with 183 tests, `cargo build --release` succeeds, and `~/.local/bin/carlos` has been refreshed from the release build.
- [x] (2026-04-06 16:37Z) Passed live Claude smoke checks with the built binary for `--backend claude`, `--backend claude continue`, and `--backend claude resume <SESSION_ID>`. The new-session path streamed assistant text and updated the exit resume hint to the real Claude session id after the first turn; continue and explicit resume both displayed the expected no-history system note and accepted a follow-up turn successfully.
- [x] (2026-04-06 17:18Z) Closed two late functional gaps before review signoff: Claude turn submission now emits a synthetic `userMessage` row so user prompts appear in the shared transcript, and the Claude runtime path no longer inherits stale Codex runtime defaults into the top-bar label. Validation now passes at `cargo test` 184, `cargo build --release`, installed-binary refresh, and a live release-binary smoke confirming prompt visibility plus a Claude-native runtime label.
- [ ] (2026-04-06 18:26Z) Final engineering-review signoff is still pending. Two separate reviewer invocations were attempted after the fixup commit: one custom `codex exec` reviewer session and one stricter retry that provided the exact diff in stdin. Both sessions kept exploring the repository but did not return a final verdict in the required output shape before this run had to stop, so the ExecPlan cannot be closed yet under the repo review rules.
- [x] (2026-04-06 18:39Z) The longer-running built-in reviewer finally returned a concrete finding instead of process chatter: the prompt-echo fix introduced a cross-sender ordering race between synthetic user prompts and reader-thread Claude output. Fixed by moving Claude prompt surfacing into synchronous app-side turn submission instead of injecting it from the backend event channel. Validation reran cleanly: `cargo test` 184, `cargo build --release`, and release-binary reinstall.
- [x] (2026-04-06 19:28Z) A fresh reviewer pass against the prompt-ordering fix found two follow-up regressions in the queued-turn path: Claude follow-up prompts could still jump ahead of deferred output from the prior turn, and auto-queued Ralph turns polluted editable input history on the Claude backend. Fixed by tagging queued turns with history intent, skipping auto-history for Ralph/internal turns, and refusing to auto-submit a queued turn until both deferred output and immediately pending UI/server events have been drained. Validation reran cleanly: `cargo test` 188, `cargo build --release`, and release-binary reinstall.

## Surprises & Discoveries

- Observation: `carlos` does not consume arbitrary JSON deltas. `src/app/notification_items.rs` only reacts to exact method names such as `item/started`, `item/completed`, and `item/agentMessage/delta`, and for delta events it expects `params.delta` to be a plain string.
  Evidence: `handle_item_notification()` only reads `params.get("delta").and_then(Value::as_str)` for the agent, tool-call, and tool-result delta methods.

- Observation: the backend dependency is wider than `src/protocol.rs`. The concrete `AppServerClient` type is currently wired directly into startup, event-loop orchestration, turn submission, interruption, and the resume picker archive action.
  Evidence: concrete `AppServerClient` references exist in `src/app/mod.rs`, `src/app/input.rs`, `src/app/input_events.rs`, and `src/app/terminal_ui.rs`.

- Observation: resume history is currently populated only from the start/resume response body. If a backend does not return `result.thread.turns`, `load_history_from_start_or_resume()` silently leaves the transcript empty.
  Evidence: `src/app/notification_items.rs` only appends history when the response contains `result.thread.turns`.

- Observation: the existing context-usage parser already accepts both the modern `thread/tokenUsage/updated` shape and the older token-count shape, so the Claude backend can reuse the existing context badge by emitting one of those shapes instead of teaching the renderer about a new protocol.
  Evidence: `src/app/context_usage.rs` exposes both `context_usage_from_thread_token_usage_params()` and `context_usage_from_token_count_params()`.

- Observation: the current picker delete flow is Codex-specific because it always issues `thread/archive`. Claude MVP should not pretend to support archive semantics until a supported Claude-side action exists.
  Evidence: `src/app/terminal_ui.rs::pick_thread()` hard-calls `client.call("thread/archive", ...)` when the user presses `d`.

- Observation: the cleanest Milestone 1 seam was narrower than the whole bootstrap path. The event loop, turn submission, and model-catalog fetch converted cleanly to `&dyn BackendClient`, but the no-id resume picker is still intentionally concrete because it owns Codex-only archive semantics.
  Evidence: the trait-backed changes were limited to `src/backend.rs`, `src/protocol.rs`, `src/app/mod.rs`, `src/app/input.rs`, and `src/app/input_events.rs`, while `src/app/terminal_ui.rs` still takes `&AppServerClient`.

- Observation: Claude stream-json input for a user turn is not a bare prompt string. The accepted envelope is a top-level `type: "user"` object with `message.role = "user"` and a content array of text blocks.
  Evidence: a real `claude -p --input-format stream-json --output-format stream-json` session accepted input shaped as `{"type":"user","message":{"role":"user","content":[{"type":"text","text":"..."}]}}`, and `src/claude_backend.rs::send_stream_user_message()` now emits that exact form.

- Observation: `claude -p --resume <SESSION_ID>` in stream-json mode does not replay prior transcript history and does not even idle successfully without a fresh prompt.
  Evidence: the CLI returned `Error: No deferred tool marker found in the resumed session... Provide a prompt to continue the conversation.` during the transport spike, and the Claude startup path now injects a system note instead of attempting undocumented session-history scraping.

- Observation: Claude tool execution is a multi-message lifecycle: assistant tool-use blocks arrive first, then tool results arrive as a top-level user event, and finally a follow-up assistant message may continue the turn.
  Evidence: the transport spike captured `stream_event.content_block_start/content_block_delta/content_block_stop` with `tool_use`, followed by a top-level `{"type":"user", ... "tool_use_result": ...}` record, then a second assistant message and terminal `result`.

- Observation: Claude does not emit `system/init` immediately on process startup in stream-json mode. The init event arrives only after the first streamed user turn is submitted.
  Evidence: the initial TUI smoke failed with `timed out waiting for Claude session init`, while a direct pipe test and the repaired TUI run showed `system/init` as the first stdout line only after sending a stream-json user message. The implementation now starts with a placeholder thread id and swaps in the real Claude session id through a synthetic `thread/initialized` notification.

- Observation: without an explicit synthetic user-message event, Claude-backed turns do not show the human prompt in the shared transcript model.
  Evidence: the first live smoke after backend bring-up showed only assistant output for Claude turns because the translator never emits `userMessage` items and the shared UI only appends user prompts from that synthetic surface. The backend now emits a synthetic `item/started` `userMessage` line after successful turn submission.

- Observation: inheriting persisted runtime defaults into the Claude path leaks Codex-specific labels into the Claude status bar even though Claude model/effort/summary editing is disabled.
  Evidence: the first release smoke showed `gpt-5.4/high/concise` in the Claude top bar. The Claude startup path now seeds no shared runtime defaults, and the later `thread/initialized` notification sets only the actual Claude model name.

- Observation: the shared queued-turn path serves two distinct producers with different history semantics: explicit user follow-ups queued during an active Claude turn should still bind back to input-history rewind entries, while Ralph bootstrap and continuation turns should never appear as editable prompt history.
  Evidence: the second engineering-review pass showed that recording history unconditionally on successful Claude `turn/start` polluted prompt navigation with Ralph’s base prompt and automatic continuations.

## Decision Log

- Decision: keep the TUI protocol surface stable by introducing a small backend trait with Codex-shaped request/response methods, then make `ClaudeClient` synthesize JSON-RPC-shaped responses and notifications on top of its NDJSON transport.
  Rationale: the rest of the application already works in terms of JSON strings flowing through `UiEvent::ServerLine(String)` and `handle_server_message_line()`. Preserving that surface contains the feature to the transport boundary instead of rewriting the renderer, transcript model, or notification parser.
  Date/Author: 2026-04-06 / codex

- Decision: use a trait object (`Box<dyn BackendClient>`) rather than a second large branch tree throughout the app.
  Rationale: the runtime is dominated by process I/O and rendering, not dynamic dispatch. A trait object keeps the shared paths simple and makes the Claude work additive instead of duplicating `run()`, the event loop, and turn submission logic.
  Date/Author: 2026-04-06 / codex

- Decision: scope the MVP to new Claude sessions, `continue`, and `resume <SESSION_ID>`. Do not support `carlos resume` without an id for Claude in this ExecPlan.
  Rationale: the current no-id resume flow depends on `thread/list`, the picker UI, and archive support. Those are all Codex-shaped today. Shipping the backend first is more valuable than blocking on a second session-management implementation.
  Date/Author: 2026-04-06 / codex

- Decision: leave `src/app/terminal_ui.rs` concrete in Milestone 1 and convert only the shared runtime surfaces to `BackendClient`.
  Rationale: the picker flow is not a shared runtime surface in the MVP because Claude will not support no-id resume or archive yet. Keeping that file concrete avoids a fake abstraction that would immediately need backend-specific branching anyway.
  Date/Author: 2026-04-06 / codex

- Decision: skip Claude interactive approvals in the MVP by launching Claude in a no-prompt permission mode, and make `respond()` / `respond_error()` on the Claude client fail loudly if they are ever called unexpectedly.
  Rationale: `carlos` approval UI is built around JSON-RPC server requests with ids. Recreating Claude’s approval protocol is a separate feature and should not be mixed into the initial transport and translation work.
  Date/Author: 2026-04-06 / codex

- Decision: when the user submits input during an active Claude turn, queue that text as the next turn instead of attempting a fake `turn/steer`.
  Rationale: the repository already has safe queued-turn machinery in `RalphRuntimeState` that the main event loop drains after the active turn completes. Reusing that path is simpler and more honest than inventing unsupported mid-turn semantics.
  Date/Author: 2026-04-06 / codex

- Decision: if Claude resume does not provide replayable prior transcript history, record that explicitly in the UI with a system message rather than scraping Claude’s private session storage.
  Rationale: scraping implementation-specific session files would create a brittle coupling to an external tool. A clear limitation message is a safer MVP behavior.
  Date/Author: 2026-04-06 / codex

- Decision: translate Claude `Bash` activity as a `toolCall` row plus a separate `toolResult` row instead of a synthetic `commandExecution` completion item in the first pass.
  Rationale: the current transcript machinery already renders generic tool-call/tool-result pairs reliably, while the success-path `commandExecution` formatting suppresses the streamed output details that are most useful for Claude’s tool-result events. This keeps the MVP honest and avoids losing stdout/stderr context.
  Date/Author: 2026-04-06 / codex

- Decision: suppress successful Claude `Read` tool results from the transcript unless they carry an error.
  Rationale: Claude returns full file contents in many `Read` tool results, which would spam the transcript and drown the assistant turn. The file path is still visible from the preceding tool-call item, so hiding the success payload is the safer MVP default.
  Date/Author: 2026-04-06 / codex

- Decision: attach explicit history-intent metadata to queued turns and delay queued-turn submission until prior output has been drained from both deferred buffers and the UI event channel.
  Rationale: a plain queued string was no longer enough once Claude started appending prompts locally. The queue must distinguish user-authored follow-ups from internal Ralph turns, and the loop must not submit the next Claude turn ahead of already-produced output from the previous one.
  Date/Author: 2026-04-06 / codex

## Outcomes & Retrospective

Pending. On completion, `carlos` should have one shared TUI that can speak to either backend, with Codex behavior preserved and Claude support added behind an explicit backend selection. The expected tradeoff for the first version is that Claude session browsing, archiving, and interactive approvals remain out of scope, but the common interactive experience for new work, explicit resume, streaming text, tools, interruption, and context usage becomes available.

The milestone order below intentionally favors transport containment over breadth. The most important outcome is not “there is a Claude subprocess.” The important outcome is that the new backend fits the current app architecture cleanly enough that future Claude-specific follow-up work can stay localized.

Milestone 1 partial outcome: the shared runtime no longer depends directly on `AppServerClient`. The new `BackendClient` trait now covers shared request/response and event-stream behavior, `AppServerClient` implements it without changing the Codex transport, and the main event loop plus input-handling paths compile against the trait. The Codex-only picker/archive path remains concrete by design, and the full suite still passes after the refactor.

Milestone 2 and Milestone 3 partial outcome: the repository now has a concrete Claude adapter slice. `carlos` can select `--backend claude` or `CARLOS_BACKEND=claude`, start a `claude` subprocess in stream-json mode, reuse the Claude session id as the thread id, translate streamed assistant text and tool activity into the existing synthetic notification surface, queue follow-up user input instead of pretending to steer an active Claude turn, and reject unsupported no-id resume behavior clearly. Focused Claude tests now cover CLI parsing, queued-next-turn behavior, streamed text translation, and Bash tool-result translation, and the full suite currently passes at 182 tests before the final runtime smoke/build/install pass.

Plan change note (2026-04-06 / codex): the implementation narrowed one user-visible detail relative to the original prompt examples. Bash tool activity currently renders through the generic `toolCall` + `toolResult` transcript path rather than the compact `commandExecution` row, because that preserves output fidelity with the existing renderer. If later UX work wants the compact shell row, that should be a follow-up on top of the now-working transport boundary instead of being forced into the initial backend landing.

Validation note (2026-04-06 / codex): the first end-to-end TUI smoke exposed a real startup bug in the initial adapter design. Waiting for Claude session init during `ClaudeClient::start()` deadlocked startup because `system/init` only appears after the first user message. The shipped implementation now handles that lazily, and the repaired release binary was verified interactively for new-session, continue, and explicit-resume flows.

Review note (2026-04-06 / codex): the first engineering-review pass surfaced two real user-facing correctness issues in the just-landed adapter slice: missing Claude user prompts in the transcript and misleading inherited runtime labels from the Codex defaults path. Both were fixed in a follow-up slice before final review signoff.

Open review-status note (2026-04-06 / codex): after the fixup commit, the required fresh reviewer invocation was retried with a stricter prompt that supplied the exact diff in stdin and explicitly forbade extra repo exploration. The reviewer tooling still failed to return a final shaped verdict during this run, so reviewer signoff remains the only blocker to moving this ExecPlan to `.agents/done/`.

Follow-up review note (2026-04-06 / codex): once allowed to run longer, the built-in `codex review --commit 1b1df95` session did return a concrete bug report. It flagged a prompt-ordering race caused by sending synthetic Claude user-message events from a different `mpsc` producer than the reader-thread-translated Claude output. That finding was resolved by removing backend-side prompt injection and appending the Claude user prompt synchronously in `submit_turn_text()` after a successful Claude `turn/start` call.

Second follow-up review note (2026-04-06 / codex): the next built-in `codex review --commit 824d816` pass found that the first fix still allowed queued Claude prompts to overtake deferred output from the previous turn, and that the new unconditional Claude history-recording path leaked Ralph auto-prompts into editable history. The implementation now tags queued turns with `record_input_history`, keeps Ralph/internal turns out of input history, and only auto-submits a queued turn once no deferred or prefetched events remain to be rendered from the previous turn.

## Context and Orientation

`carlos` is a Rust terminal user interface under `src/app/` that currently talks to `codex app-server` through `src/protocol.rs::AppServerClient`. In this repository, “backend” means the subprocess plus transport adapter that provides session lifecycle, streamed events, turn submission, interruption, and optional approvals. The current backend speaks JSON-RPC over newline-delimited JSON. Claude Code speaks a different newline-delimited JSON protocol, usually called NDJSON here: one complete JSON object per line on stdout, with streamed event objects such as `message_start`, `content_block_delta`, and `message_stop`.

The current data path is important. `src/app/mod.rs::run()` starts the backend and seeds `AppState`. `src/app/input.rs::run_conversation_tui()` reads `UiEvent::ServerLine(String)` messages and passes them to `src/app/notifications.rs::handle_server_message_line()`. That notification dispatcher mostly delegates item lifecycle work to `src/app/notification_items.rs`, which updates `AppState.messages` and the item-id mappings that drive the transcript. The renderer does not know anything about backend protocols; it only knows `AppState`.

For this ExecPlan, a “synthetic JSON-RPC notification” means a JSON line produced locally by the Claude adapter, not by Claude itself, shaped like the notifications the existing parser already expects. Example: when Claude emits a streamed text delta, the adapter should forward a line shaped like `{"method":"item/agentMessage/delta","params":{"itemId":"...","delta":"..."}}`. This lets `src/app/notification_items.rs` keep doing the transcript work unchanged or nearly unchanged.

The major repository touchpoints are:

- `src/main.rs`: module declarations only; it must add the new backend modules.
- `src/protocol.rs`: current Codex client, request parameter builders, and response parsers.
- `src/app/mod.rs`: CLI parsing, backend startup, session bootstrap, model loading, and the top-level `run()` orchestration.
- `src/app/input.rs`: main event loop and server-line dispatch.
- `src/app/input_events.rs`: turn submission, interruption, and approval replies.
- `src/app/notifications.rs` and `src/app/notification_items.rs`: the notification methods and fields the Claude adapter must synthesize.
- `src/app/tools.rs`: tool naming, command extraction, diff detection, and generic tool formatting that Claude tool vocabulary should reuse.
- `src/tests/*.rs`: the current test layout. This ExecPlan should add a new backend-focused test module instead of hiding Claude tests inside unrelated files.

The main external assumption is simple and explicit: the machine running Claude validation must already have a working `claude` CLI binary installed and authenticated. This ExecPlan does not cover installing or authenticating Claude itself.

## Plan of Work

Start by isolating the transport boundary. Create `src/backend.rs` with a small `BackendClient` trait and a backend-kind enum. Move only the common transport surface into that trait: request/response lines, approval replies, event receiver ownership, process stop, and a cheap way to identify whether the concrete backend is Codex or Claude. `src/protocol.rs::AppServerClient` should implement the trait without changing its current behavior.

Once the Codex client sits behind the trait, change the shared TUI paths to accept `&dyn BackendClient` or `Box<dyn BackendClient>` instead of `&AppServerClient`. The key files are `src/app/mod.rs`, `src/app/input.rs`, and `src/app/input_events.rs`. `src/app/terminal_ui.rs` should remain Codex-specific for the resume picker in this ExecPlan because the Claude backend will not use picker mode yet. Do not widen that file unless the implementation truly needs it.

Then add `src/claude_backend.rs`. This module owns the Claude subprocess, its stdin/stdout threads, request bookkeeping for synthetic `call()` responses, and the translator from Claude NDJSON to Codex-shaped notifications. The launch path must support three modes: a fresh session, `--resume <session_id>`, and `--continue`. The adapter must record the Claude session id from the initial system event and reuse that id as the `thread_id` stored in `AppState`.

The translator is the center of the feature. It should maintain enough per-turn state to:

- generate a stable synthetic turn id when a Claude message starts,
- assign stable synthetic item ids for text blocks and tool-use blocks,
- accumulate text deltas into plain-string `item/agentMessage/delta` notifications,
- accumulate `input_json_delta` fragments until a tool-use block has complete JSON input,
- map Claude `Bash` tool use to `commandExecution` items so `src/app/tools.rs` can show the compact command row,
- map all other tool use to `toolCall` items and all tool results to `toolResult` items,
- emit a synthetic `thread/tokenUsage/updated` notification whenever Claude provides enough usage data,
- emit a synthetic `turn/completed` notification with `status: "completed"` or `status: "interrupted"` when the turn ends.

Because the Claude protocol details are not fully known from this repository alone, take one explicit prototyping slice before finalizing the transport helpers. Verify three things against a real `claude` binary: the stdin envelope that starts a user turn, the exact `system/init` payload that carries the session id, and whether `--resume <id>` replays any prior transcript automatically. Record what was observed in `Surprises & Discoveries` and adjust the concrete helper functions accordingly. If resume does not replay history, do not chase undocumented session storage; append a system note after startup when resuming or continuing a Claude session with no seeded transcript.

After the adapter exists, integrate the CLI. Extend `CliOptions` in `src/app/mod.rs` with a backend enum parsed from `--backend <codex|claude>` and `CARLOS_BACKEND`, defaulting to Codex. Startup must branch by backend. The Codex path keeps the current `initialize`, `thread/start`, `thread/resume`, `thread/list`, and `model/list` behavior. The Claude path must:

- reject `resume` without an explicit session id with a clear error,
- support plain start, explicit resume, and continue,
- seed `AppState` with the Claude session id from the synthetic start/resume response,
- populate the runtime model list only if the launch layer can pass a chosen model through reliably; otherwise, leave Claude model settings disabled for this ExecPlan and record the limitation clearly.

Turn submission also needs one explicit behavioral fork. In `src/app/input_events.rs::submit_turn_text()`, keep the current Codex `turn/start` and `turn/steer` behavior. For Claude, a new turn still goes through `call("turn/start", ...)`, but an attempted steer during an active turn should enqueue the text for the next turn and set a status such as `queued for next Claude turn`. Add or expose a small `AppState` helper for queueing ordinary turn input so this does not reach into `RalphRuntimeState` directly.

Interruption should remain user-visible and simple. When the user interrupts a Claude turn, the adapter should signal the Claude subprocess in the least surprising supported way, then emit the same synthetic `turn/completed` notification shape the rest of the app already uses for interrupted turns. The app should continue appending the existing “Turn interrupted” system marker through the normal notification path.

Finally, tighten formatting and test coverage. `src/app/tools.rs` may need small Claude-oriented additions so a `Bash` item shows `input.command`, `Read` shows a file path, and `Edit` / `Write` outputs route through the existing diff extraction when the Claude tool result contains diff-like text. Add focused tests for the translation helpers, CLI parsing, queued-next-turn behavior, and any new tool-formatting cases. Keep the Codex tests green and add Claude tests in a new `src/tests/claude_backend_tests.rs`.

## Milestones

### Milestone 1: Introduce the backend boundary without changing behavior

At the end of this milestone, the shared TUI and input code no longer depend on the concrete `AppServerClient` type, but the only implemented backend behavior is still Codex. Acceptance is that `cargo test` remains green and a normal Codex session still starts, streams, and exits exactly as before.

### Milestone 2: Prove the Claude transport contract

At the end of this milestone, the repository contains a small, tested Claude transport layer that can start the subprocess, read NDJSON lines, identify the session id, and translate at least one streamed text turn into synthetic notifications. Acceptance is a manual smoke test with a real `claude` binary plus unit tests for the translation helpers, and a recorded answer to the two open transport questions: stdin envelope and resume-history behavior.

### Milestone 3: Ship Claude MVP integration

At the end of this milestone, `carlos --backend claude`, `carlos --backend claude continue`, and `carlos --backend claude resume <SESSION_ID>` all work within the agreed MVP scope. Acceptance is that assistant text streams into the transcript, Claude tool use renders through the existing tool formatting path, `Ctrl+C` interrupts a turn, and the context badge updates from Claude usage data when available.

### Milestone 4: Harden, validate, and review

At the end of this milestone, Codex and Claude paths both pass the test suite, the release binary has been rebuilt and reinstalled, and the engineering review has either passed or been resolved. Acceptance is that the final change set is buildable, validated, and documented, with this ExecPlan updated to match what actually shipped.

## Concrete Steps

All commands below should be run from the repository root: `/var/home/wegel/work/perso/carlos`.

1. Identify the current concrete backend touchpoints before editing:

       rg -n "AppServerClient|thread/start|thread/resume|thread/list|thread/archive|turn/start|turn/steer|turn/interrupt" src/app src/protocol.rs

2. Add the backend boundary and Claude module skeleton, then run:

       cargo test

3. During the Claude transport spike, use focused tests while the adapter is still incomplete:

       cargo test claude_backend

   If the repository does not yet have a Claude-specific test filter, add the new tests first and use the module name you create, for example:

       cargo test claude_backend_tests

4. After Claude startup and translation compile, run the full suite:

       cargo test

5. After the runtime behavior is in place, build and refresh the installed binary:

       cargo build --release
       cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

6. Manual Codex smoke check after the abstraction lands:

       cargo run --

   Expect the usual Codex-backed session startup and the final resume hint on exit.

7. Manual Claude smoke check for a new session, assuming the `claude` binary is installed and authenticated:

       cargo run -- --backend claude

   Enter a short prompt such as “say hello and run pwd”. Expect streamed assistant text, at least one tool row if Claude uses tools, and a final resume hint that includes the Claude session id.

8. Manual Claude smoke check for explicit resume:

       cargo run -- --backend claude resume <SESSION_ID>

   If Claude does not replay prior history, expect either an empty transcript plus a clear system note or whatever replay behavior was actually observed during Milestone 2. Record the observed behavior in this ExecPlan.

9. Manual Claude smoke check for continue:

       cargo run -- --backend claude continue

   Expect Claude to reopen the most recent session if the CLI supports `--continue`; otherwise, stop, record the exact failure, and revise this plan before forcing a different behavior.

10. Keep this ExecPlan current after every meaningful slice by updating `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`.

## Validation and Acceptance

Validation is mandatory after each milestone.

The minimum acceptance bar for this ExecPlan is:

- `cargo test` passes after each meaningful implementation slice and at the end.
- `cargo build --release` passes once the runtime-facing Claude path is complete.
- `~/.local/bin/carlos` is refreshed from the new release build before closeout.
- The existing Codex backend still supports start, resume, continue, interruption, approvals, session picker, and archive exactly as before.
- `carlos --backend claude` starts a Claude-backed session and shows streamed assistant text in the main transcript.
- `carlos --backend claude resume <SESSION_ID>` and `carlos --backend claude continue` work within the behavior confirmed during the transport spike.
- Claude `Bash` tool activity renders as `commandExecution`-style transcript rows, and non-shell Claude tools render through the generic tool-call/tool-result path.
- Interrupting a Claude turn produces the same visible interrupted-turn behavior as the Codex path.
- The context-usage badge updates from Claude usage data when Claude provides enough token information.
- Any unsupported Claude flows in this MVP are rejected clearly and intentionally, not by accidental fall-through.
- The final engineering review is not `FAIL`.

Final acceptance is not “the code compiles with another flag.” Final acceptance is that a human can actually run `carlos` against Claude, use the existing TUI productively for the MVP flows, and still trust that the Codex backend has not regressed.

## Idempotence and Recovery

This work should proceed in small, buildable commits. The Codex path must remain usable throughout. If the backend abstraction lands before the Claude client is complete, the branch should still be in a state where `cargo test` passes and the default Codex backend works.

If the Claude transport spike disproves one of the assumed CLI contracts, do not patch around it with undocumented filesystem scraping or a second hidden protocol. Stop, record the observed behavior in `Surprises & Discoveries`, update `Decision Log`, and revise the narrowest safe design. The safe fallback for unsupported session-history replay is a clear system note after resume, not a brittle importer.

Binary installation must keep using the temporary-file-and-rename pattern:

    cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

## Artifacts and Notes

The source prompt for this ExecPlan is `carlos-claude-backend-prompt.md`. This plan intentionally narrows that prompt in three places for the first implementation pass:

- Claude no-id resume picker and archive are deferred.
- Claude interactive approvals are deferred.
- Claude model-selection UI is only in scope if the launch flag can be verified cleanly during implementation; otherwise it stays disabled rather than misleading.

The Claude adapter should emit synthetic lines shaped like these examples:

    {"method":"turn/started","params":{"turn":{"id":"claude-turn-1"}}}
    {"method":"item/started","params":{"item":{"id":"claude-text-1","type":"agentMessage"}}}
    {"method":"item/agentMessage/delta","params":{"itemId":"claude-text-1","delta":"hello"}}
    {"method":"item/completed","params":{"item":{"id":"claude-bash-1","type":"commandExecution","command":"pwd","exitCode":0,"aggregatedOutput":"/repo\n"}}}
    {"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"modelContextWindow":200000,"total":{"totalTokens":12345},"last":{"totalTokens":12345}}}}
    {"method":"turn/completed","params":{"turn":{"id":"claude-turn-1","status":"completed"}}}

If Claude tool results arrive separately from tool-use blocks, prefer reusing the same synthetic item id so a started placeholder can later become the completed output row through the existing mapped-message path.

## Interfaces and Dependencies

Create `src/backend.rs` with these stable names unless implementation evidence forces a revision:

    use std::sync::mpsc;
    use std::time::Duration;
    use anyhow::Result;
    use serde_json::Value;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum BackendKind {
        Codex,
        Claude,
    }

    pub(crate) trait BackendClient {
        fn kind(&self) -> BackendKind;
        fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
        fn respond(&self, request_id: &Value, result: Value) -> Result<()>;
        fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()>;
        fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>>;
        fn stop(&mut self);
    }

Create `src/claude_backend.rs` with these implementation-facing names:

    pub(crate) enum ClaudeLaunchMode {
        New,
        Resume(String),
        Continue,
    }

    pub(crate) struct ClaudeClient { ... }

    impl ClaudeClient {
        pub(crate) fn start(
            cwd: &std::path::Path,
            launch_mode: ClaudeLaunchMode,
            model: Option<&str>,
        ) -> anyhow::Result<Self>;
    }

The Claude module should keep the translation helpers pure where possible so they are easy to test. At minimum, isolate helpers with shapes like:

    fn translate_claude_line(state: &mut ClaudeTranslationState, line: &str) -> anyhow::Result<Vec<String>>;
    fn synthetic_start_response(session_id: &str) -> String;
    fn synthetic_model_list_response() -> String;

Modify these files as part of the implementation:

- `src/main.rs`: declare `mod backend; mod claude_backend;`
- `src/protocol.rs`: implement `BackendClient` for `AppServerClient`
- `src/app/mod.rs`: parse backend selection, branch startup, and keep Codex-only picker behavior
- `src/app/input.rs`: accept the trait-backed client in the event loop
- `src/app/input_events.rs`: switch shared calls to the backend trait and queue-next-turn behavior for Claude
- `src/app/state.rs` or `src/app/ralph_runtime_state.rs`: expose a safe helper to enqueue ordinary next-turn input
- `src/app/tools.rs`: add any small Claude vocabulary support needed for `Bash`, `Read`, `Write`, and `Edit`
- `src/tests.rs`: register `src/tests/claude_backend_tests.rs`
- `src/tests/claude_backend_tests.rs`: add translation, CLI, and queued-input tests

Plan change note: 2026-04-06. Created this ExecPlan by converting `carlos-claude-backend-prompt.md` into the repository’s living-plan format and narrowing the MVP to the flows that fit the current app architecture cleanly.
Plan change note: 2026-04-06. Updated after Milestone 1 to record the actual abstraction seam that landed: shared runtime paths now use `BackendClient`, while the Codex-only picker/archive flow remains concrete on purpose.
