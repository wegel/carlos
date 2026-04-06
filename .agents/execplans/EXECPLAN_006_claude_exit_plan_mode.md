# Handle Claude ExitPlanMode approvals in carlos

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, a Claude session running inside `carlos` should be able to leave plan mode when Claude emits the `ExitPlanMode` tool call. The user-visible behavior is that `carlos` pauses normal chat input, shows the proposed plan plus the allowed follow-up prompts, lets the user approve or decline, and then sends a real response back to Claude for that exact tool call instead of leaking a plain `yes` or `no` message into the transcript.

This matters because the current Claude backend already renders Claude tool calls and tool results, but it does not implement Claude-side approvals. In practice that means Claude can ask to leave plan mode, `carlos` can display the situation, but the session stays stuck because there is no backend reply path. The goal of this ExecPlan is to wire `ExitPlanMode` into the existing approval overlay and backend response path without creating a second Claude-only state machine or regressing the Codex flow.

## Progress

- [x] (2026-04-06 18:00Z) Created this ExecPlan, registered it in `PROGRAM_PLAN.md`, and grounded the scope in the shipped Claude adapter, the existing app approval overlay, and a captured failing Claude session log.
- [x] (2026-04-06 16:48Z) Probed the raw `claude -p --input-format stream-json --output-format stream-json` transport in `/tmp` and captured the real `ExitPlanMode` flow: `ToolSearch(select:ExitPlanMode)` -> `ExitPlanMode` tool call -> immediate fallback `tool_result` with `content: "Exit plan mode?"` and `is_error: true` when no host approval UI answers it.
- [x] (2026-04-06 16:49Z) Proved the practical recovery path for `carlos`’ current backend mode: resuming the same session with `--permission-mode bypassPermissions` changes the live Claude process to `permissionMode: "bypassPermissions"`, after which a follow-up user message like `Continue with the approved plan now.` causes Claude to execute the already-approved write plan successfully.
- [ ] Update the app and backend design to treat `ExitPlanMode` as a local recovery/continue flow for the bypass-permissions Claude backend, not as a synchronous tool-result approval that the user can race in real time.
- [ ] Translate Claude `ExitPlanMode` failures into a pending approval request that the app can render through the existing overlay, including startup recovery for imported Claude history when the last persisted turn failed with `Exit plan mode?`.
- [ ] Implement Claude backend approval responses so accepting or declining the overlay sends the correct follow-up user message for the active bypass-permissions Claude process.
- [ ] Add focused tests, run `cargo test`, rebuild the release binary, reinstall `~/.local/bin/carlos`, and capture reviewer feedback.

## Surprises & Discoveries

- Observation: the app already has a generic pending-approval pipeline with keyboard handling, overlay rendering, and backend reply hooks. The missing piece for Claude is not UI plumbing but the Claude backend’s lack of request translation and response support.
  Evidence: `src/app/approval_state.rs`, `src/app/notifications.rs`, `src/app/input_events.rs`, and `src/app/overlay_render.rs` already implement approval state, request parsing, `y`/`n` handling, and approval rendering. `src/claude_backend.rs` still returns `Claude backend approvals are not implemented` from both `respond()` and `respond_error()`.

- Observation: the current stuck session is caused by `carlos` writing an error-like `tool_result` back to Claude with literal content `Exit plan mode?`, after which the user’s `yes` is recorded as a normal plan-mode chat message.
  Evidence: the local Claude session log `~/.claude/projects/-var-home-wegel-work-perso-stormvault/82f024a0-f05c-4eff-b8e5-0f2272063cad.jsonl` shows repeated `tool_use` records with `name: "ExitPlanMode"` followed by user-side `tool_result` records containing `content: "Exit plan mode?"`, `is_error: true`, and later a separate user text record `text: "yes"` with `permissionMode: "plan"`.

- Observation: `ExitPlanMode` is structurally a tool call, not assistant prose. Claude supplies both a free-form plan body and a structured `allowedPrompts` list that the client should present before replying.
  Evidence: the same session log shows `tool_use` input fields `plan`, `planFilePath`, and `allowedPrompts`, including tool names such as `Bash` plus concrete prompts like `run cargo build, test, clippy, fmt`.

- Observation: we do not yet have a captured successful `ExitPlanMode` acceptance or decline envelope from Claude’s stream-json transport.
  Evidence: local session search under `~/.claude/projects/` found multiple failing `ExitPlanMode` retries but no successful `tool_result` shape to copy, so the exact response payload must be probed directly before implementation hardens around an assumed schema.

- Observation: the raw print/stream-json transport does expose `ExitPlanMode`, but it does not wait for a human-scale approval round trip. In the direct probe, Claude emitted `ToolSearch(select:ExitPlanMode)`, then `ExitPlanMode`, then almost immediately wrote its own fallback user-side `tool_result` with `content: "Exit plan mode?"` and `is_error: true`.
  Evidence: the `/tmp` probe session `778b4606-ed45-4787-bcf6-c5d34efd93ba` logged `ToolSearch` at `16:47:20Z`, `ExitPlanMode` at `16:47:22Z`, and the fallback error tool result at `16:47:22Z` in the persisted session JSONL under `~/.claude/projects/-tmp/778b4606-ed45-4787-bcf6-c5d34efd93ba.jsonl`.

- Observation: sending a delayed success `tool_result` with plain content `approved` for the same `ExitPlanMode` tool call does not, by itself, lift the session out of plan-mode restrictions.
  Evidence: after injecting `{"type":"tool_result","content":"approved","is_error":false,...}` for `toolu_01HRuUf81gvH3CxnSeoZD4Qk`, the same session remained `permissionMode: "plan"` and follow-up writes failed with `Claude requested permissions to write ... but you haven't granted it yet.`

- Observation: resuming the same Claude session under `--permission-mode bypassPermissions` does override the live process permission mode even when the persisted session history contains a failed `ExitPlanMode`. In that resumed process, a normal user message asking Claude to continue the approved plan successfully executes the write.
  Evidence: the direct resume probe on session `778b4606-ed45-4787-bcf6-c5d34efd93ba` reported `permissionMode: "bypassPermissions"` in the `system/init` event, then wrote `/tmp/carlos-exit-plan-probe-2` successfully after the user message `Continue with the approved plan now.`

## Decision Log

- Decision: reuse the existing app approval overlay and pending-approval state instead of building a separate Claude-specific “plan mode UI”.
  Rationale: the app already has one path that blocks normal input, renders approval context, and dispatches approve or decline actions to the active backend. Reusing that path reduces code duplication and lowers the chance of Codex and Claude approval behavior drifting apart.
  Date/Author: 2026-04-06 / codex

- Decision: scope the first implementation to Claude `ExitPlanMode` only, even if other Claude-native approval-style tools may exist.
  Rationale: `ExitPlanMode` is the concrete user-visible blocker with captured evidence, and it exercises the full approval round-trip. Narrowing the first slice keeps the protocol probe and tests focused while leaving room to generalize later if more Claude-only approval tools appear.
  Date/Author: 2026-04-06 / codex

- Decision: require a transport probe for the accepted and declined `tool_result` envelope before writing the backend response code.
  Rationale: the captured failure log only proves what not to send. The success path must be grounded in an observed Claude subprocess interaction so `carlos` does not encode a speculative reply format into the backend.
  Date/Author: 2026-04-06 / codex

- Decision: keep the Claude transcript row for the `ExitPlanMode` tool call while also emitting a synthetic approval request into the app.
  Rationale: the transcript should remain faithful to what Claude asked for, but the app also needs a structured pending approval so user input is bound to the original tool call instead of becoming free-form chat text.
  Date/Author: 2026-04-06 / codex

- Decision: adapt the implementation to the transport `carlos` actually uses today: a bypass-permissions Claude process with best-effort recovery from persisted or freshly observed `ExitPlanMode` fallback turns.
  Rationale: the raw print/stream-json transport does not leave enough time for a human to answer `ExitPlanMode` before Claude emits its own fallback error tool result. The directly reachable recovery path in `carlos` is to resume the session under `--permission-mode bypassPermissions`, surface the pending approval to the user, and on acceptance send a normal follow-up user message that tells Claude to continue with the approved plan.
  Date/Author: 2026-04-06 / codex

- Decision: treat `ExitPlanMode` acceptance in `carlos` as a local recovery action that sends a follow-up user message, not as a direct attempt to synthesize the entire hidden internal approval state transition.
  Rationale: the only observed reliable path with the current backend launch mode is `resume under bypassPermissions` plus a follow-up prompt. A guessed low-level approval payload would be brittle and was not sufficient in the direct probe.
  Date/Author: 2026-04-06 / codex

## Outcomes & Retrospective

Outcome (2026-04-06 / codex): the protocol reconnaissance is complete enough to replace the original assumption. The implementation has not started yet, but the actual transport behavior, viable recovery path, and updated target behavior are now explicit enough for a stateless implementer to begin work safely.

Retrospective:
- The key insight is that this is not fundamentally a “plan mode” feature. It is a missing Claude approval round-trip layered on top of an approval UI the app already has.
- The first protocol assumption in this plan was wrong for the transport `carlos` actually uses. Raw `print/stream-json` does surface `ExitPlanMode`, but it also auto-falls back quickly enough that a human-driven approval cannot race it.
- The largest technical risk is no longer discovering a hidden “perfect” approval payload. It is turning the observed recovery path into a clean user experience in `carlos` without lying about what the backend can do synchronously.

## Context and Orientation

The Claude backend boundary lives in `src/claude_backend.rs`. It starts a `claude -p --input-format stream-json --output-format stream-json` subprocess, translates Claude NDJSON records into synthetic Codex-shaped notifications, and exposes the shared `BackendClient` trait from `src/backend.rs`. Today the live Claude translator emits transcript items for assistant text, tool calls, and tool results, but `ClaudeClient::respond()` and `ClaudeClient::respond_error()` both bail with `Claude backend approvals are not implemented`.

The app-side approval machinery already exists and is backend-agnostic. `src/app/notifications.rs` recognizes server requests with an `id` and converts specific methods into `PendingApprovalRequest` values. `src/app/approval_state.rs` defines the approval kinds and the result payloads to send back when the user chooses accept, decline, or cancel. `src/app/input_events.rs` blocks normal typing while an approval is pending and routes `y`, `n`, `s`, and `c` into `client.respond(...)`. `src/app/overlay_render.rs` draws the modal overlay that shows the approval title, method, details, and footer hints.

For this ExecPlan, “Claude `ExitPlanMode` approval” now means the following exact sequence for the current `carlos` backend design:

1. Claude emits a `tool_use` block with `name: "ExitPlanMode"` and a unique Claude `tool_use` identifier.
2. In the raw print/stream-json transport, Claude immediately falls back to a user-side error `tool_result` with `content: "Exit plan mode?"` if no host approval UI answers it.
3. `carlos` treats that pair as a pending approval request and shows the plan text to the user, preventing normal chat input while the approval is pending.
4. When the user accepts, `carlos` uses its live bypass-permissions Claude process to send a normal follow-up user message instructing Claude to continue the approved plan. When the user declines, `carlos` sends a normal follow-up user message asking Claude to revise the plan.
5. Because the active Claude process in `carlos` runs with `--permission-mode bypassPermissions`, the accepted follow-up can execute the planned write without getting trapped in plan mode or write-permission prompts.

The captured failing examples live outside this repository in Claude’s local session store. The external user session `~/.claude/projects/-var-home-wegel-work-perso-stormvault/82f024a0-f05c-4eff-b8e5-0f2272063cad.jsonl` proves the original stuck-session bug. The direct `/tmp` probes under `~/.claude/projects/-tmp/` prove the raw print/stream-json transport behavior that `carlos` actually sees.

## Plan of Work

Keep the protocol probe notes in this ExecPlan as durable context, then implement the feature against the behavior actually observed in the raw transport.

Extend `src/claude_backend.rs` so that when a `tool_use` block with `name: "ExitPlanMode"` completes, the translator records the plan payload and emits a synthetic request notification that the app can render. The synthetic request should carry the Claude `tool_use` identifier, the full `plan` text, and `planFilePath` when present. The translator should also recognize the immediate fallback user-side `tool_result` with `content: "Exit plan mode?"` and keep enough state to mark the pending plan approval as unresolved rather than just appending an opaque error transcript row.

Extend `src/app/approval_state.rs` and `src/app/notifications.rs` so the app recognizes the synthetic Claude request and turns it into a `PendingApprovalRequest`. This request should use a dedicated approval kind such as `ClaudeExitPlanMode`. The detail lines should show the plan text clearly. This approval should offer accept and decline. It should not offer “accept for session” or “cancel turn”.

Then implement the backend response path in `src/claude_backend.rs` for the current bypass-permissions backend mode. `ClaudeClient::respond()` should detect the Claude exit-plan approval request and convert the user choice into a normal follow-up user message:

- accept: send a user message that tells Claude the plan is approved and it should continue with the planned implementation now.
- decline: send a user message that tells Claude the plan is not approved and it should revise the plan before requesting approval again.

This is intentionally not a low-level `tool_result` reply attempt. The direct probes showed that the current print/stream-json transport already emitted its fallback error by the time a human could respond, while the resumed bypass-permissions process successfully continued the plan after an ordinary follow-up user message.

Also extend startup handling in `src/app/mod.rs` and the Claude local-history import path so that if imported Claude history ends with an unresolved failed `ExitPlanMode` turn, `carlos` surfaces the approval overlay immediately after loading history. This is the directly user-reachable scenario for resumed external Claude sessions.

Keep the Codex backend untouched except where shared approval plumbing requires additive support for the new approval kind. The app must continue to block normal typing while any approval is pending, so the existing bug where `yes` becomes a user message cannot recur once a Claude exit-plan approval is active in `carlos`.

## Milestones

### Milestone 1: Prove the Claude `ExitPlanMode` reply envelope

At the end of this milestone, the repository maintainers should know the raw `ExitPlanMode` stream that `carlos` actually receives. The work is a bounded subprocess probe and documentation update, not production code. Acceptance is a short captured transcript in this ExecPlan showing `ToolSearch(select:ExitPlanMode)`, the `ExitPlanMode` tool call, the immediate fallback `Exit plan mode?` error tool result, and the proven resume-with-bypass recovery path.

### Milestone 2: Surface `ExitPlanMode` as a pending approval

At the end of this milestone, a live or imported Claude `ExitPlanMode` failure should create a pending approval overlay in `carlos` instead of falling through to transcript-only behavior. Acceptance is a focused test proving that the translator or startup recovery path emits the synthetic approval request and an app-level test proving that normal text entry is blocked while that request is pending.

### Milestone 3: Reply to Claude and leave plan mode

At the end of this milestone, pressing the approval keys in `carlos` should cause the Claude backend to send the correct follow-up user message for the current bypass-permissions process, allowing the approved plan to continue. Acceptance is a backend-level test for the serialized follow-up user message plus a manual smoke test where a resumed Claude session with a failed `ExitPlanMode` turn is approved through `carlos` and then proceeds with the planned write.

### Milestone 4: Validate, review, and close out

At the end of this milestone, the new tests should pass, the release binary should be rebuilt and installed, and the engineering reviewer should have assessed the change. Acceptance is `cargo test`, `cargo build --release`, a refreshed `~/.local/bin/carlos`, and a recorded reviewer verdict in this ExecPlan.

## Concrete Steps

Run the following commands from the repository root at `/var/home/wegel/work/perso/carlos` unless noted otherwise.

1. Inspect the current Claude backend and approval plumbing before editing:

       sed -n '1,260p' src/claude_backend.rs
       sed -n '1,260p' src/app/approval_state.rs
       sed -n '1,320p' src/app/notifications.rs
       sed -n '140,320p' src/app/input_events.rs

2. Probe the Claude CLI outside the app to capture the `ExitPlanMode` transport behavior. The exact prompt may evolve, but the output must include the raw NDJSON lines for `ToolSearch`, `ExitPlanMode`, the fallback `Exit plan mode?` tool result, and the resume-under-bypass recovery path:

       claude -p --input-format stream-json --output-format stream-json --verbose

   Feed a controlled prompt sequence until Claude emits `ExitPlanMode`, record the fallback error behavior, then verify that resuming the same session with `--permission-mode bypassPermissions` plus a follow-up user message continues the approved plan. Record those transcripts in this ExecPlan before coding the response path.

3. Implement the translator, approval mapping, and backend response path in:

       src/claude_backend.rs
       src/app/approval_state.rs
       src/app/notifications.rs

4. Add or extend tests in:

       src/tests/claude_backend_tests.rs
       src/app/... tests that exercise approval pending state if existing coverage is insufficient

5. Validate the change:

       cargo test
       cargo build --release
       install -Dm755 target/release/carlos ~/.local/bin/carlos

6. Run a manual smoke test against a Claude session that reaches plan mode, then collect engineering review using the repo’s `codex exec`-based reviewer workflow.

Expected validation result after implementation:

       running 2xx tests
       test result: ok. 2xx passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   The exact test count may increase, but the suite must remain fully green.

## Validation and Acceptance

Acceptance is behavior, not just compilation.

For the automated path, run `cargo test` and expect the full suite to pass with new coverage for Claude exit-plan translation, startup recovery, and follow-up prompt serialization. The new tests should prove three concrete behaviors: Claude `ExitPlanMode` failures become a pending approval request, normal input is blocked while that approval is pending, and approving or declining serializes the expected follow-up user message.

For the manual path, start `carlos` against the Claude backend and resume a Claude session whose last persisted turn failed with `Exit plan mode?`. The app should render a modal approval overlay instead of requiring the user to type raw chat text. Press the accept key and observe that `carlos` sends the follow-up approval prompt into the bypass-permissions Claude process, after which Claude continues with the planned implementation. At no point should typing `yes` appear in the transcript as a user message while the approval is pending.

Because this changes installed runtime behavior for `carlos`, a successful implementation must also include `cargo build --release` and a refreshed `~/.local/bin/carlos`.

## Idempotence and Recovery

The code changes in this plan are additive and can be applied incrementally. The protocol probe can be rerun as many times as needed, and its outputs should be recorded in this ExecPlan so a later contributor does not need to rediscover the reply envelope from scratch.

If a future Claude version changes the raw `ExitPlanMode` flow again, update this ExecPlan before changing code. Do not guess a hidden synchronous approval schema. If a partial implementation leaves the backend compiling but the manual smoke test cannot continue the approved plan in a bypass-permissions Claude process, remove or guard the new approval translation until the response path is correct; a visible modal that cannot actually unblock the user is worse than the current transcript-only behavior.

## Artifacts and Notes

Known failing behavior before implementation:

    assistant tool_use:
      name: "ExitPlanMode"
      id: "toolu_01LspVjGPCstd1CtjGLwJ7bH"

    current bad user tool_result written by carlos:
      {
        "type": "tool_result",
        "content": "Exit plan mode?",
        "is_error": true,
        "tool_use_id": "toolu_01LspVjGPCstd1CtjGLwJ7bH"
      }

    then a separate user chat message appears:
      {
        "role": "user",
        "content": [{ "type": "text", "text": "yes" }],
        "permissionMode": "plan"
      }

That sequence proves the original bug: the answer path was not bound to a user-visible recovery flow.

Observed raw transport behavior from the direct `/tmp` probes:

    system/init:
      permissionMode: "plan"
      tools include "ToolSearch" and "ExitPlanMode"

    assistant:
      tool_use ToolSearch { query: "select:ExitPlanMode", max_results: 1 }

    user:
      tool_result { content: [{ "type": "tool_reference", "tool_name": "ExitPlanMode" }] }

    assistant:
      tool_use ExitPlanMode {
        plan: "# Plan: Create file ...",
        planFilePath: "/home/wegel/.claude/plans/greedy-inventing-river.md"
      }

    immediate fallback from Claude when no host approval responds:
      user tool_result {
        content: "Exit plan mode?",
        is_error: true,
        tool_use_id: "<ExitPlanMode tool_use_id>"
      }

Observed recovery path:

    1. Resume the same session with:
         claude -p --resume <SESSION_ID> --permission-mode bypassPermissions ...
    2. Confirm system/init now reports:
         permissionMode: "bypassPermissions"
    3. Send a normal user message:
         "Continue with the approved plan now."
    4. Claude executes the planned Write tool successfully.

## Interfaces and Dependencies

The existing backend boundary in `src/backend.rs` must remain:

    pub(crate) trait BackendClient {
        fn kind(&self) -> BackendKind;
        fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
        fn respond(&self, request_id: &Value, result: Value) -> Result<()>;
        fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()>;
        fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>>;
        fn stop(&mut self);
    }

At the end of this ExecPlan, the following interfaces and behaviors must exist:

- In `src/app/approval_state.rs`, add a dedicated approval kind for Claude plan exit, with response mapping that supports accept and decline and rejects unsupported session-wide or cancel semantics.
- In `src/app/notifications.rs`, add parsing for a synthetic Claude approval request method whose params include the original Claude `tool_use` identifier, `plan`, and optional `planFilePath`.
- In `src/claude_backend.rs`, retain the existing transcript item translation for Claude tool calls and add a synthetic approval request emission for `ExitPlanMode` plus its fallback error result.
- In `src/claude_backend.rs`, implement `ClaudeClient::respond()` so it serializes and writes the follow-up user prompt that continues or rejects the plan in the active bypass-permissions Claude process.
- In `src/app/mod.rs` or the Claude startup recovery path, surface a pending approval immediately when imported Claude history ends with an unresolved failed `ExitPlanMode`.
- In `src/tests/claude_backend_tests.rs`, add regression coverage for live translation, startup recovery detection, and response serialization.

Plan change note (2026-04-06 / codex): Revised after direct raw transport probes in `/tmp`. The original plan assumed `carlos` needed to synthesize an immediate low-level `tool_result` approval. The probes showed the raw print/stream-json transport falls back too quickly for that to be the practical UX, while resume under `--permission-mode bypassPermissions` plus a follow-up user message reliably continues the approved plan. The implementation is now scoped to that observed recovery path.
