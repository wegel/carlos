# Modularize the TUI runtime without regressing behavior or performance

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, contributors should be able to work on transcript rendering, picker overlays, input handling, protocol notifications, and application state without editing one giant file per feature. The user-visible behavior must remain the same, but the code should become easier to review, safer to change, and less likely to accumulate unrelated logic in single files such as `src/app/render.rs` and `src/app/state.rs`. The proof is twofold: the existing UI behavior still works, and the major runtime files become narrower in responsibility with no performance regression on large sessions.

## Progress

- [x] (2026-03-28 01:15Z) Created the modularization ExecPlan and registered it in `PROGRAM_PLAN.md`.
- [ ] Split `src/app/render.rs` into responsibility-focused modules while preserving all rendering behavior and tests.
- [ ] Split `src/app/state.rs` into focused state structures and helper modules without changing runtime semantics.
- [ ] Split `src/app/input.rs` and `src/app/notifications.rs` into narrower orchestration plus domain-specific helpers.
- [ ] Split `src/tests.rs` to mirror the runtime module boundaries.
- [ ] Re-run correctness and perf validation, update this ExecPlan, and move it to `.agents/done/` when complete.

## Surprises & Discoveries

- Observation: None yet.
  Evidence: Plan creation only; implementation has not started.

## Decision Log

- Decision: Treat runtime modularization as a dedicated ExecPlan instead of opportunistic cleanup.
  Rationale: The problem is architectural, not cosmetic. The main files mix multiple state machines and rendering domains, so safe progress requires staged boundaries, validation, and explicit invariants.
  Date/Author: 2026-03-28 / codex

- Decision: Preserve user-visible behavior first and use file splits plus ownership boundaries as the primary refactor tool.
  Rationale: The repo has recently done substantial correctness and performance work. A large rewrite would create unnecessary regression risk. Splitting by responsibility keeps the code buildable and measurable after each step.
  Date/Author: 2026-03-28 / codex

## Outcomes & Retrospective

No outcomes yet. This section must be updated after each major milestone and at completion with the modules created, behavior preserved, validation run, and any remaining architectural debt.

## Context and Orientation

`carlos` is a Rust terminal user interface that talks to `codex app-server` over JSON-RPC. The TUI is concentrated under `src/app/`. Today, several files have become large because they mix multiple concerns:

- `src/app/render.rs` contains style conversion, markdown and ANSI conversion, diff rendering, transcript layout, overlay rendering, the main screen renderer, and the resume picker.
- `src/app/state.rs` contains the main application state object, transcript cache bookkeeping, rewind/input history behavior, Ralph automation state, model settings state, approval state, selection state, and some rendering support logic.
- `src/app/input.rs` contains the main event loop and input handling for keyboard, mouse, approvals, scrolling, redraw cadence, and queued-turn submission.
- `src/app/notifications.rs` contains server request handling, protocol notification dispatch, history loading, tool item materialization, token usage updates, and animation timing helpers.
- `src/tests.rs` contains nearly all tests in one file, which no longer mirrors the runtime structure.

In this repository, “transcript rendering” means taking stored `Message` values and turning them into `RenderedLine` rows for the terminal. “Notification handling” means taking JSON-RPC notifications or requests from app-server and mutating `AppState` accordingly. “Picker” means the resume-session chooser shown by `carlos resume` when no explicit session id is provided.

The recent performance work is important context. Large sessions are now much smoother, and this refactor must not undo that work. The perf harness already exists through `carlos perf-session ...`, including synthetic generation and captured-session measurement. Use that harness after each major split.

## Plan of Work

The work should happen in staged, buildable slices that follow the existing dependency direction instead of rewriting everything at once.

Start with `src/app/render.rs`. Create new modules under `src/app/` that each own one rendering domain. The target shape is roughly:

- a transcript rendering module for building and counting rendered transcript blocks,
- a text styling module for markdown and ANSI conversion helpers,
- an overlay rendering module for help, approvals, perf, and model settings,
- a picker rendering module for resume and delete/archive dialogs.

Keep `render.rs` only as a thin composition layer if needed, or remove it entirely if the resulting modules have cleaner names. The important constraint is that picker rendering and transcript rendering no longer live in the same file.

Next, split `src/app/state.rs` by state ownership. Keep a narrow top-level `AppState`, but move logically separate state and behavior into smaller structs or helper modules. The expected boundaries are:

- transcript/render-cache state,
- input and rewind history state,
- Ralph automation state,
- runtime/model-settings state,
- approval-dialog state,
- selection and scroll state.

Do not change the external behavior of `AppState` unless a smaller public surface makes call sites cleaner. The goal is not API churn; it is to stop one file from being the owner of every mutable concern in the TUI.

Then split `src/app/input.rs` and `src/app/notifications.rs`. The event loop should remain easy to follow, but parsing and domain-specific handling should move out of the monolithic functions. `run_conversation_tui` should orchestrate, not contain every branch directly. `handle_server_message_line` should delegate by domain: turn lifecycle, item lifecycle, tool materialization, context usage, and approval requests.

Finally, split `src/tests.rs` to match the new runtime modules. Tests should live near the behavior they cover, either as module-scoped `#[cfg(test)]` sections or smaller test files imported from `src/`. The main requirement is that tests become discoverable by responsibility instead of remaining one giant sink.

## Milestones

### Milestone 1: Split transcript and picker rendering

At the end of this milestone, transcript layout/counting code and picker/modal rendering code no longer share the same source file. A contributor changing the resume UI should not need to touch diff rendering or markdown wrapping code in the same file. Run `cargo test` and at least one `carlos perf-session` command after the split. Acceptance is that the UI still renders correctly and perf numbers for large sessions remain within noise of the current baseline.

### Milestone 2: Split `AppState` into focused ownership domains

At the end of this milestone, `AppState` still exists but becomes a coordinator over smaller state components instead of a giant field bag. A contributor working on Ralph mode, selection, or model settings should be able to find those fields and helpers in a focused module or sub-structure. Run `cargo test` and a perf-session validation after the split. Acceptance is unchanged runtime behavior with simpler ownership boundaries.

### Milestone 3: Split input orchestration and notification handling

At the end of this milestone, the main event loop and the main notification dispatcher both delegate substantial work to narrower helpers or modules. The event loop should read like orchestration logic instead of a full subsystem implementation. The notification path should separate protocol decoding from state mutation by domain. Run `cargo test` and repeat perf validation. Acceptance is unchanged UI behavior and no measured slowdown in typing, scrolling, or transcript updates.

### Milestone 4: Split tests to mirror the runtime structure

At the end of this milestone, `src/tests.rs` is no longer the single home for nearly everything. Tests are grouped by responsibility in a way that mirrors the runtime layout. Run `cargo test` and ensure the module split did not lose coverage. Acceptance is green tests with easier discoverability and reduced merge-conflict surface.

## Concrete Steps

All commands below should be run from the repository root: `/var/home/wegel/work/perso/carlos`.

1. Record the starting file-size baseline:

       find src -type f -name '*.rs' -print0 | xargs -0 wc -l | sort -nr | head -n 15

   Keep the output in this ExecPlan’s `Artifacts and Notes` section after the first implementation commit.

2. Before each milestone, identify the functions and types that will move:

       rg -n '^pub\\(super\\)? fn |^fn |^impl ' src/app/render.rs src/app/state.rs src/app/input.rs src/app/notifications.rs

3. After each meaningful slice, run:

       cargo test

4. After each runtime-affecting slice, run:

       cargo build --release
       cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

5. After each milestone, run at least one large-session perf check. Prefer both a real captured session and a synthetic one if time permits:

       carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48

   And, when a suitable captured session is available:

       carlos perf-session /path/to/captured-session.jsonl --width 160 --height 48

6. Keep this ExecPlan current after every slice by updating `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`.

## Validation and Acceptance

Validation is mandatory after every milestone.

The minimum acceptance bar is:

- `cargo test` passes after each meaningful change.
- `cargo build --release` passes after each runtime-affecting milestone.
- The installed `~/.local/bin/carlos` binary is updated after runtime-affecting milestones.
- `carlos perf-session` shows no meaningful regression in large-session redraw, append, or layout timings relative to the baseline captured at milestone start.
- Manual smoke checks still work for:
  - `carlos resume`,
  - `carlos continue`,
  - scrolling and selection,
  - model settings overlay,
  - approval dialogs,
  - Ralph mode markers and continuation behavior.

Final acceptance for this ExecPlan is not “the files are shorter.” Final acceptance is that the runtime is split into coherent modules, the user-visible behavior remains intact, and performance remains at least as good as the current baseline.

## Idempotence and Recovery

These refactors should be done in small, buildable commits so recovery is always a normal Git operation rather than a large rollback. Module extraction is naturally idempotent if each step keeps the same tests green. If a split causes ambiguity or a perf regression, stop the milestone, record the discovery in this ExecPlan, and land the smallest safe intermediate structure rather than forcing the full split in one patch.

When replacing the installed binary, use the temporary-file-and-rename pattern already used elsewhere in this repository so active running instances do not block deployment:

    cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

## Artifacts and Notes

Add short terminal transcripts here as work proceeds. The first artifact to add during implementation is the baseline file-size report and the first perf-session baseline used for comparison.

## Interfaces and Dependencies

No new external library is required for this ExecPlan. The work should stay within the existing Rust crate layout under `src/app/` plus `src/tests.rs` or replacement test modules.

The key interfaces that must remain valid during the refactor are:

- `crate::app::run()` as the top-level CLI entrypoint.
- `crate::app::input::run_conversation_tui(...)` or its replacement orchestration entrypoint.
- `crate::app::notifications::handle_server_message_line(...)` or a compatibly named dispatcher.
- `crate::app::state::AppState` as the central runtime state object, even if it becomes thinner internally.
- `carlos perf-session ...` as the performance validation tool.

If new submodules are introduced, keep names literal and responsibility-based. Prefer names such as `transcript_render`, `picker_render`, `runtime_settings`, `approval_state`, `turn_notifications`, or similarly direct names. Avoid vague names like `utils`, `misc`, or `helpers`.

Revision note: Created this ExecPlan on 2026-03-28 to track architectural modularization after large-session performance work made it clear that the main remaining debt is responsibility mixing in the core TUI files.
