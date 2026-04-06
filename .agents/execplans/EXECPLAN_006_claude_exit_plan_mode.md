# Handle Claude ExitPlanMode approvals in carlos

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, a Claude session running inside `carlos` should be able to leave plan mode when Claude emits the `ExitPlanMode` tool call. The user-visible behavior is that `carlos` pauses normal chat input, shows the proposed plan plus the allowed follow-up prompts, lets the user approve or decline, and then sends a real response back to Claude for that exact tool call instead of leaking a plain `yes` or `no` message into the transcript.

This matters because the current Claude backend already renders Claude tool calls and tool results, but it does not implement Claude-side approvals. In practice that means Claude can ask to leave plan mode, `carlos` can display the situation, but the session stays stuck because there is no backend reply path. The goal of this ExecPlan is to wire `ExitPlanMode` into the existing approval overlay and backend response path without creating a second Claude-only state machine or regressing the Codex flow.

## Progress

- [x] (2026-04-06 18:00Z) Created this ExecPlan, registered it in `PROGRAM_PLAN.md`, and grounded the scope in the shipped Claude adapter, the existing app approval overlay, and a captured failing Claude session log.
- [ ] Prove the accepted and declined Claude `tool_result` envelope for `ExitPlanMode` with a focused subprocess probe instead of guessing from the failing log.
- [ ] Translate Claude `ExitPlanMode` tool calls into a pending approval request that the app can render through the existing overlay.
- [ ] Implement Claude backend approval responses so accepting or declining the overlay sends a real `tool_result` bound to the original Claude `tool_use_id`.
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

## Outcomes & Retrospective

Outcome (2026-04-06 / codex): planning only. The implementation has not started yet, but the failure mode, target files, and first protocol unknown are now explicit enough for a stateless implementer to begin work safely.

Retrospective:
- The key insight is that this is not fundamentally a “plan mode” feature. It is a missing Claude approval round-trip layered on top of an approval UI the app already has.
- The largest technical risk is not rendering or keyboard handling; it is replying to Claude with the exact accepted or declined `tool_result` envelope it expects.

## Context and Orientation

The Claude backend boundary lives in `src/claude_backend.rs`. It starts a `claude -p --input-format stream-json --output-format stream-json` subprocess, translates Claude NDJSON records into synthetic Codex-shaped notifications, and exposes the shared `BackendClient` trait from `src/backend.rs`. Today the live Claude translator emits transcript items for assistant text, tool calls, and tool results, but `ClaudeClient::respond()` and `ClaudeClient::respond_error()` both bail with `Claude backend approvals are not implemented`.

The app-side approval machinery already exists and is backend-agnostic. `src/app/notifications.rs` recognizes server requests with an `id` and converts specific methods into `PendingApprovalRequest` values. `src/app/approval_state.rs` defines the approval kinds and the result payloads to send back when the user chooses accept, decline, or cancel. `src/app/input_events.rs` blocks normal typing while an approval is pending and routes `y`, `n`, `s`, and `c` into `client.respond(...)`. `src/app/overlay_render.rs` draws the modal overlay that shows the approval title, method, details, and footer hints.

For this ExecPlan, “Claude `ExitPlanMode` approval” means the following exact sequence:

1. Claude emits a `tool_use` block with `name: "ExitPlanMode"` and a unique Claude `tool_use` identifier.
2. `carlos` records that tool call in the transcript, then creates a pending approval request keyed to that exact `tool_use` identifier.
3. The app overlay shows the proposed plan text and the `allowedPrompts` list, preventing normal chat input while the approval is pending.
4. When the user accepts or declines, `carlos` sends a Claude `tool_result` back to the subprocess for that same `tool_use` identifier.
5. Claude either exits plan mode and continues, or remains in plan mode according to the response. In either case, the reply must not appear as a stray user chat message.

The captured failing example lives outside this repository in Claude’s local session store at `~/.claude/projects/-var-home-wegel-work-perso-stormvault/82f024a0-f05c-4eff-b8e5-0f2272063cad.jsonl`. That file proves the current bug but not the correct reply schema, so this plan starts with a transport probe milestone.

## Plan of Work

Start by adding a focused protocol probe to `src/claude_backend.rs` development work, not as shipped production code. The implementer should run a small standalone `claude -p --input-format stream-json --output-format stream-json` interaction that intentionally reaches an `ExitPlanMode` tool call and captures the exact successful and declined `tool_result` payloads Claude accepts. Record the observed shape in this ExecPlan under `Artifacts and Notes` before relying on it in code.

Once the envelope is known, extend the Claude translator in `src/claude_backend.rs` so that when a `tool_use` block with `name: "ExitPlanMode"` completes, the translator emits two things. First, it should keep the normal `item/completed` tool-call row so the transcript stays faithful. Second, it should emit a synthetic request notification with an `id`, a method name dedicated to Claude plan exit approval, the original Claude `tool_use` identifier, and the structured fields needed by the overlay: `plan`, `planFilePath` if present, and `allowedPrompts`. The translator must also retain enough state to answer that request later.

Then extend `src/app/approval_state.rs` and `src/app/notifications.rs` so the app recognizes the synthetic Claude request and turns it into a `PendingApprovalRequest`. This request should use a dedicated approval kind such as `ClaudeExitPlanMode` rather than overloading an unrelated command approval kind. The detail lines should present the plan body in readable wrapped text, followed by a compact summary of each allowed prompt in the form `<tool>: <prompt>`. This approval should offer accept and decline. It should not offer “accept for session”, because the Claude tool call is about the current transition, not a reusable permission grant.

Finally, implement the backend response path in `src/claude_backend.rs`. `ClaudeClient::respond()` should detect the Claude exit-plan approval request, map the app’s accept or decline choice into the observed Claude `tool_result` envelope, and write that JSON line to the subprocess stdin using the original Claude `tool_use` identifier. If the app sends an unsupported choice, the backend should fail clearly instead of inventing behavior. `respond_error()` should either map explicit declines if the protocol requires an error-form result, or remain unused if both accept and decline are ordinary `tool_result` payloads. The app should clear the pending approval only after the backend acknowledges the write succeeded.

Keep the Codex backend untouched except where shared approval plumbing requires additive support for the new approval kind. The app must continue to block normal typing while any approval is pending, so the existing bug where `yes` becomes a user message cannot recur once a Claude exit-plan approval is active.

## Milestones

### Milestone 1: Prove the Claude `ExitPlanMode` reply envelope

At the end of this milestone, the repository maintainers should know the exact accepted and declined `tool_result` payloads Claude expects for `ExitPlanMode`. The work is a bounded subprocess probe and documentation update, not production code. Acceptance is a short captured transcript in this ExecPlan showing the successful response shape for at least the accept path, plus the implementer’s conclusion about whether decline uses a normal result or an error-shaped result.

### Milestone 2: Surface `ExitPlanMode` as a pending approval

At the end of this milestone, a live Claude `ExitPlanMode` tool call should create a pending approval overlay in `carlos` instead of falling through to transcript-only behavior. Acceptance is a focused test proving that the translator emits the synthetic approval request and an app-level test proving that normal text entry is blocked while that request is pending.

### Milestone 3: Reply to Claude and leave plan mode

At the end of this milestone, pressing the approval keys in `carlos` should cause the Claude backend to send a real `tool_result` response back to Claude for the original `tool_use` identifier. Acceptance is a backend-level test for the serialized response line plus a manual smoke test where a Claude session reaches `ExitPlanMode`, the user approves through `carlos`, and the session proceeds without trapping the user in plan mode.

### Milestone 4: Validate, review, and close out

At the end of this milestone, the new tests should pass, the release binary should be rebuilt and installed, and the engineering reviewer should have assessed the change. Acceptance is `cargo test`, `cargo build --release`, a refreshed `~/.local/bin/carlos`, and a recorded reviewer verdict in this ExecPlan.

## Concrete Steps

Run the following commands from the repository root at `/var/home/wegel/work/perso/carlos` unless noted otherwise.

1. Inspect the current Claude backend and approval plumbing before editing:

       sed -n '1,260p' src/claude_backend.rs
       sed -n '1,260p' src/app/approval_state.rs
       sed -n '1,320p' src/app/notifications.rs
       sed -n '140,320p' src/app/input_events.rs

2. Probe the Claude CLI outside the app to capture the `ExitPlanMode` reply envelope. The exact prompt may evolve, but the output must include the raw NDJSON lines for the `tool_use` and the accepted response:

       claude -p --input-format stream-json --output-format stream-json --verbose

   Feed a controlled prompt sequence until Claude emits `ExitPlanMode`, then answer it by writing a raw `tool_result` line. Record the observed accepted and declined payloads in this ExecPlan before coding the response path.

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

For the automated path, run `cargo test` and expect the full suite to pass with new coverage for Claude exit-plan translation and reply serialization. The new tests should prove three concrete behaviors: Claude `ExitPlanMode` tool calls become a pending approval request, normal input is blocked while that approval is pending, and approving or declining serializes a backend response bound to the original Claude `tool_use` identifier.

For the manual path, start `carlos` against the Claude backend and drive or resume a Claude session that emits `ExitPlanMode`. When the tool call appears, the app should render a modal approval overlay instead of accepting raw chat text. Press the accept key and observe that Claude leaves plan mode and continues the session. Repeat with decline if the transport probe establishes a supported decline path. At no point should typing `yes` appear in the transcript as a user message while the approval is pending.

Because this changes installed runtime behavior for `carlos`, a successful implementation must also include `cargo build --release` and a refreshed `~/.local/bin/carlos`.

## Idempotence and Recovery

The code changes in this plan are additive and can be applied incrementally. The protocol probe can be rerun as many times as needed, and its outputs should be recorded in this ExecPlan so a later contributor does not need to rediscover the reply envelope from scratch.

If the transport probe shows that decline is unsupported or that the reply format differs across Claude versions, stop and update this ExecPlan before proceeding. Do not guess a fallback schema in code. If a partial implementation leaves the backend compiling but the manual smoke test still traps the session in plan mode, remove or guard the new approval translation until the backend response path is correct; a visible modal that cannot successfully answer Claude is worse than the current transcript-only behavior.

## Artifacts and Notes

Known failing behavior before this ExecPlan:

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

That sequence proves the current bug: the answer path is not bound to the original tool call.

The first implementation milestone must replace this section with short captured examples of the real accepted and declined `tool_result` envelopes observed from a standalone Claude subprocess probe.

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
- In `src/app/notifications.rs`, add parsing for a synthetic Claude approval request method whose params include the original Claude `tool_use` identifier, `plan`, optional `planFilePath`, and `allowedPrompts`.
- In `src/claude_backend.rs`, retain the existing transcript item translation for Claude tool calls and add a synthetic approval request emission for `ExitPlanMode`.
- In `src/claude_backend.rs`, implement `ClaudeClient::respond()` so it can serialize and write the observed Claude `tool_result` payload for the stored `tool_use` identifier.
- In `src/tests/claude_backend_tests.rs`, add regression coverage for the translator and response serializer.

Plan change note (2026-04-06 / codex): Initial ExecPlan created after confirming from a local Claude session log that `carlos` currently treats `ExitPlanMode` as a transcript artifact instead of a real approval round-trip. The plan intentionally starts with a protocol probe because the failure logs do not reveal the accepted success envelope.
