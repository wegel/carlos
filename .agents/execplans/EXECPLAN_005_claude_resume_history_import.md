# Import Claude local transcript history on resume/continue

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, `carlos --backend claude resume <SESSION_ID>` and `carlos --backend claude continue` should show the prior Claude transcript immediately instead of starting from an empty transcript plus a limitation note. The user-visible outcome is that a resumed Claude session in `carlos` looks like a real resumed conversation from the start of the TUI run, even though Claude’s print/stream-json transport does not replay history itself.

This matters because the current Claude backend already has enough runtime behavior to continue a session correctly, but the transcript starts blank until the next turn. That is materially worse than the Codex path and makes resumed Claude sessions hard to inspect safely. The goal of this ExecPlan is to add a best-effort local history importer from Claude’s persisted session files without weakening the existing Codex path or pretending that the on-disk format is a stable public API.

## Progress

- [x] (2026-04-06 16:09Z) Created this ExecPlan, registered it in `PROGRAM_PLAN.md`, and grounded the scope in the shipped Claude adapter plus the local Claude session-store format observed on disk.
- [ ] Implement local Claude session resolution and JSONL transcript import for explicit `resume <SESSION_ID>` and `continue`.
- [ ] Add focused tests for session-path resolution, local history import, and fallback behavior when local session files are unavailable or malformed.
- [ ] Run `cargo test`, `cargo build --release`, refresh `~/.local/bin/carlos`, collect engineering review, and close out the ExecPlan.

## Surprises & Discoveries

- Observation: Claude’s local session JSONL files already contain the raw ingredients needed to reconstruct transcript history: `type: "user"` records with prompt content, `type: "assistant"` records with text and `tool_use` blocks, and `tool_result` payloads recorded as user-side content objects.
  Evidence: the local file `~/.claude/projects/-var-home-wegel-work-perso-stormvault/82f024a0-f05c-4eff-b8e5-0f2272063cad.jsonl` contains `custom-title`, `agent-name`, `user`, `assistant`, and tool result records for the named session `media-pipeline-worker-pool`.

- Observation: the current `carlos` Claude startup path is already structured to accept synthetic history from a start response. `run_claude_backend()` calls `load_history_from_start_or_resume()` just like the Codex path, but today the Claude synthetic response includes only a placeholder thread id and no turns.
  Evidence: `src/app/mod.rs::run_claude_backend()` creates `start_resp` with `ClaudeClient::synthetic_start_response()`, then immediately calls `load_history_from_start_or_resume(&mut app, &start_resp)` before appending the no-history system note.

## Decision Log

- Decision: import Claude history by translating local JSONL records into the existing Codex-shaped history-item surface, then reuse `append_history_from_thread()` instead of adding a Claude-only renderer/history path.
  Rationale: the app already has one tested history-ingest path for `userMessage`, `agentMessage`, `toolCall`, `toolResult`, and related items. Reusing that surface keeps the feature localized to the Claude boundary and avoids duplicate transcript-building rules.
  Date/Author: 2026-04-06 / codex

- Decision: scope the importer as best-effort local replay only. If local session resolution or parsing fails, keep startup working and append a clear system note that local Claude history could not be reconstructed.
  Rationale: Claude’s disk format is not a published protocol contract. The importer should improve the happy path without turning unavailable local files or format drift into a hard startup failure.
  Date/Author: 2026-04-06 / codex

## Outcomes & Retrospective

Pending. On completion, Claude resume/continue in `carlos` should seed the transcript from local Claude session storage when available, preserve the existing live-stream translation for new activity, and fall back gracefully when local reconstruction is impossible.

## Context and Orientation

The Claude backend lives in `src/claude_backend.rs`. Today it starts a `claude -p --input-format stream-json --output-format stream-json` subprocess, translates live NDJSON lines into synthetic JSON-RPC-style notifications, and exposes a `synthetic_start_response()` that only seeds a placeholder thread id. The shared app startup logic in `src/app/mod.rs` uses `load_history_from_start_or_resume()` from `src/app/notification_items.rs` to seed transcript history from a start or resume response.

The important app-side constraint is that history loading already expects a Codex-shaped thread object:

- `result.thread.id` for the thread id.
- `result.thread.turns[*].items[*]` for history items.
- item types like `userMessage`, `agentMessage`, `toolCall`, `toolResult`, and `contextCompaction`.

The local Claude session store lives under `~/.claude/projects/`. Within that directory, Claude creates one subdirectory per working tree and stores one `<SESSION_ID>.jsonl` file per persisted session. The JSONL records are in chronological order and include transcript records plus metadata. For this ExecPlan, “best-effort local import” means:

- resolve the right session file for explicit resume or continue,
- parse only transcript-relevant records,
- convert them into the app’s existing history item shapes,
- ignore metadata that the renderer does not need,
- and never fail startup just because the local file is missing or partially unparseable.

## Plan of Work

Add a small local-session helper layer to `src/claude_backend.rs`. It should resolve the Claude projects root, map the current working directory to Claude’s project-directory naming convention for `continue`, and locate explicit session-id files for `resume <SESSION_ID>`. From there, implement a JSONL parser that reads each line as JSON, keeps transcript-relevant records in order, and constructs one synthetic thread object with an `id` and a single ordered `turns[0].items` list. The importer should translate Claude records into the same item shapes already used by the live adapter:

- prompt-bearing `user` records become `userMessage` items,
- assistant text blocks become `agentMessage` items,
- assistant `tool_use` blocks become `toolCall` items,
- matching `tool_result` content becomes `toolResult` items using the same formatting helper as the live stream path when possible.

Update `run_claude_backend()` in `src/app/mod.rs` to resolve the local session id before startup for `resume` and `continue`, build a richer synthetic start response that includes imported history when available, and only append a fallback system note when local reconstruction is unavailable. Keep the existing live `thread/initialized` behavior intact so a new session still upgrades from the placeholder thread id after the first real Claude turn.

Add focused tests in `src/tests/claude_backend_tests.rs` for session-directory encoding, continue-session selection, JSONL import into history items, and failure cases that should produce no imported history rather than crashing. Add or extend app-level tests only where the existing history loader needs explicit coverage for imported Claude item shapes.

## Milestones

### Milestone 1: Resolve and parse local Claude sessions

At the end of this milestone, the repository should be able to locate the correct Claude JSONL file for explicit `resume <SESSION_ID>` and for `continue` in the current directory, then parse that file into an internal synthetic thread object. Acceptance is a focused test that proves a local JSONL snippet becomes ordered history items without starting the TUI.

### Milestone 2: Seed imported history into Claude startup

At the end of this milestone, `run_claude_backend()` should seed `AppState` from the imported local Claude history when available, and should append a fallback system note only when no local history can be reconstructed. Acceptance is that the relevant tests pass and the startup path still behaves correctly for fresh Claude sessions.

### Milestone 3: Validate, review, and close out

At the end of this milestone, the full test suite should pass, the release binary should be rebuilt and reinstalled, the engineering reviewer should have signed off or returned only non-blocking guidance, and this ExecPlan should be ready to move to `.agents/done/`.
