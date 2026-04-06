# Raise the codebase to stormvault A-grade code quality

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

The carlos codebase (~18,600 lines of Rust) has grown organically through six ExecPlans that prioritized features and performance. The code works and performs well, but it does not meet the structural quality bar defined in `/var/home/wegel/work/perso/stormvault/CODE_STYLE.md`. That style guide defines "A-grade Rust" as code that an expert Rust programmer would find pleasant to read: files with visible architecture, functions you can read without scrolling, section landmarks, documentation, and no copy-paste repetition.

After this ExecPlan, every source file in carlos should pass a style-guide audit: files under 400 lines, functions under 60 lines, module-level and public-API documentation, section comments for visual architecture, grouped imports, and abstracted repeated patterns. The codebase should look like it was written by a disciplined team, not accumulated by iteration.

The key constraint is that none of this may regress runtime behavior, transcript fidelity, performance, or test coverage. This is a structural rewrite that preserves every observable behavior.

## Progress

- [x] (2026-04-06 16:00Z) Recorded file-size and function-size baselines in this plan.
- [x] (2026-04-06 16:30Z) Milestone 1a: Added `//!` module doc comments to all 29 source files. Committed as `6fa1a87`.
- [x] (2026-04-06 16:45Z) Milestone 1b: Added `// --- Section ---` landmarks to 10 large files (claude_backend, state, mod, render, input_events, notifications, transcript_styles, tools, notification_items, perf_session). Committed as `a126e48`.
- [ ] Milestone 1c: Add `///` doc comments to public types/functions/methods (deferred to incremental pass alongside later milestones).
- [x] (2026-04-06) Milestone 2: Split `claude_backend.rs` (1,594 → 6 files, all ≤399 lines).
- [ ] Milestone 3: Decompose oversized functions (the "outline pattern" pass).
- [ ] Milestone 4: Split remaining oversized files.
- [ ] Milestone 5: Eliminate repetition.
- [ ] Milestone 6: Import hygiene.
- [ ] Final validation and engineering review.

## Surprises & Discoveries

- The documentation pass split naturally into two commits (module docs, then section landmarks). Deferring `///` doc comments on individual public items to later milestones avoids a large churn-only commit and lets those docs be written alongside the code changes that will reshape the public APIs.

## Decision Log

- Decision: order the milestones as documentation first, then large-file splits, then function decomposition, then repetition, then imports.
  Rationale: documentation and section comments are the cheapest changes and immediately improve readability for everyone working on subsequent milestones. Large-file splits come next because they create the module boundaries that function decomposition will fill. Repetition elimination and import cleanup are last because they benefit from the final module structure.
  Date/Author: 2026-04-06 / human + claude

- Decision: keep this as a single ExecPlan rather than splitting into multiple.
  Rationale: the milestones are independently verifiable and incrementally shippable, but they share a single acceptance criterion (the style guide audit) and a single constraint (no behavioral regression). Splitting would create coordination overhead without benefit.
  Date/Author: 2026-04-06 / human + claude

## Outcomes & Retrospective

(To be filled as milestones complete.)

## Context and Orientation

The style guide that defines the quality bar lives at `/var/home/wegel/work/perso/stormvault/CODE_STYLE.md`. It is external to this repository and should not be copied in. The key rules, paraphrased for quick reference:

- **File size**: hard cap 400 lines, sweet spot 150-300. Split by concern, not line count.
- **Function size**: hard cap 60 lines, sweet spot 15-30. Use the "outline pattern" where the top-level function tells the story and details live in well-named helpers.
- **Visual layout**: module doc comment at top, section comments (`// --- Section Name ---`) as landmarks, public items before private items, types before impls before free functions.
- **Imports**: no glob imports, three groups separated by blank lines (std / external crates / crate-local), sorted alphabetically within each group.
- **Documentation**: every module gets `//!`, every public type/function/method gets `///` (one sentence, not restating the signature).
- **Repetition**: 3+ similar patterns must be abstracted.
- **Tests**: in separate `_tests.rs` files (already done). Test names describe what is verified.

The carlos source tree has two layers: root modules (`src/*.rs`) and the app module (`src/app/*.rs`). Tests live in `src/tests/*.rs`. The app module is the bulk of the code.

### Current file-size violations (files over 400 lines)

Source files:

    claude_backend.rs      1,594   (4x over cap)
    state.rs               1,011   (2.5x over cap)
    perf_session.rs          824   (2x over cap)
    transcript_styles.rs     815   (2x over cap)
    mod.rs (app)             765   (2x over cap)
    input_events.rs          726   (2x over cap)
    tools.rs                 692   (1.7x over cap)
    render.rs                644   (1.6x over cap)
    notifications.rs         605   (1.5x over cap)
    overlay_render.rs        465
    notification_items.rs    463
    picker_render.rs         451
    runtime_settings_state.rs 432
    text.rs                  421
    tool_shell.rs            417

Test files (tracked but lower priority since the style guide allows test files to be larger when test setup is shared):

    ui_render_tests.rs     1,189
    runtime_tests.rs         849
    claude_backend_tests.rs  706
    notification_tests.rs    647
    input_tests.rs           522
    tool_tests.rs            456

### Worst function-size violations (functions over 60 lines)

    render_main_view           render.rs             ~350 lines
    translate_claude_line      claude_backend.rs      ~312 lines
    handle_key_event           input_events.rs        ~249 lines
    handle_item_notification   notification_items.rs  ~248 lines
    pending_approval_from_req  notifications.rs       ~233 lines
    run_conversation_tui       input.rs               ~193 lines
    pick_thread                terminal_ui.rs         ~183 lines
    handle_mouse_event         input_events.rs        ~176 lines
    parse_cli_args             mod.rs                 ~120 lines
    strip_terminal_controls    tool_shell.rs           ~94 lines
    command_summary_from_...   tools.rs                ~86 lines
    draw_rendered_line         render.rs               ~84 lines
    run_codex_backend          mod.rs                  ~80 lines
    run_claude_backend         mod.rs                  ~80 lines
    format_tool_item           tools.rs                ~79 lines
    format_tool_call_inline    tools.rs                ~77 lines

### Repetition hotspots

1. **Overlay drawing** (`overlay_render.rs`): `draw_help_overlay`, `draw_model_settings_overlay`, `draw_approval_overlay`, `draw_perf_overlay` all follow the same calculate-box / fill / border / draw-text pattern.
2. **Color/modifier conversion** (`transcript_styles.rs`): four functions (~120 lines) doing near-identical enum-to-enum mapping between ratatui and ratatui-core types.
3. **Count/append wrapping pairs** (`transcript_styles.rs`): `count_wrapped_*` and `append_wrapped_*` functions duplicate wrapping logic.
4. **History record parsing** (`claude_backend.rs`): `append_assistant_history_record` and `append_user_history_record` (~126 lines of repeated patterns).
5. **Settings cycling** (`runtime_settings_state.rs`): three `model_settings_cycle_*` methods follow an identical pattern.
6. **Duplicated function**: `reasoning_summary_text` exists in both `notification_items.rs` and `perf_session.rs`.

### What already meets the bar

These files need no structural work (though they may still need doc comments):

    main.rs (14), backend.rs (20), event.rs (42), viewport_state.rs (46),
    models.rs (64), tool_diff.rs (85), theme.rs (110), approval_state.rs (111),
    ralph.rs (131), context_usage.rs (136), input_history_state.rs (144),
    clipboard.rs (156)

Error handling throughout the codebase is already good (lightweight `?` / `.context()` style). Test placement in separate files is already correct. These do not need attention beyond documentation.

## Plan of Work

The work is divided into six milestones, each independently verifiable. They are ordered so that earlier milestones make later ones easier: documentation creates the landmarks that guide splits, splits create the module boundaries that receive decomposed functions, and repetition elimination works best once the final module structure is in place.

### Milestone 1: Documentation and visual architecture

This milestone adds no logic changes. It is pure annotation: module doc comments, section landmarks, and public API doc comments. It is the lowest-risk, highest-readability-impact change.

For every `.rs` file under `src/`:

1. Add a `//!` module-level doc comment as the first line. One sentence explaining what the module is for. Example: `//! Terminal UI setup, teardown, and raw-mode lifecycle.`

2. Add `// --- Section Name ---` landmark comments to files over 150 lines, separating logical groups. The groups depend on the file, but typical sections are: Imports, Types, Public API, Private helpers, Trait impls. In large files like `state.rs` or `claude_backend.rs`, use domain-specific section names (e.g., `// --- History parsing ---`, `// --- Event translation ---`).

3. Add `///` doc comments to all public types, functions, and methods. One sentence each. Do not restate the signature. Focus on what the item does or represents, not how it is called.

Do not add doc comments to private items unless the logic is non-obvious. Do not add `// increment counter` noise comments.

Acceptance: `cargo test` passes. Every file has a `//!` comment. Files over 150 lines have at least two section landmarks. Every `pub fn`, `pub struct`, `pub enum`, and `pub trait` has a `///` comment.

### Milestone 2: Split `claude_backend.rs`

This is the single worst file at 1,594 lines. It mixes type definitions, client construction, session management, history import/parsing, and line-by-line event translation. It should become a small module tree.

The target structure:

- `src/claude_backend/mod.rs` (~150 lines): `ClaudeClient` struct, `BackendClient` trait impl, public API surface. Re-exports the types callers need.
- `src/claude_backend/types.rs` (~80 lines): `ClaudeToolCall`, `ClaudeAllowedPrompt`, `ClaudeExitPlanApproval`, `ClaudeBlockState`, `ClaudeTranslationState`, `TranslateOutput` and related small types.
- `src/claude_backend/history.rs` (~250 lines): `append_assistant_history_record`, `append_user_history_record`, `synthetic_assistant_snapshot`, history import helpers.
- `src/claude_backend/translate.rs` (~300 lines): `translate_claude_line` broken into a dispatcher plus per-event-type handlers. The dispatcher function should be under 60 lines; each handler should be under 60 lines.
- `src/claude_backend/snapshot.rs` (~150 lines): assistant snapshot output synthesis.

The existing test file `src/tests/claude_backend_tests.rs` should continue to work unchanged because it imports through the public API.

Acceptance: `cargo test` passes. No file in `src/claude_backend/` exceeds 400 lines. `translate_claude_line` (or its replacement dispatcher) is under 60 lines.

### Milestone 3: Decompose oversized functions

This milestone applies the "outline pattern" from the style guide to the worst function-size violations. The goal is that every function in the codebase is under 60 lines, with the top-level function reading as an outline and details pushed into well-named helpers.

The functions to decompose, grouped by file:

**`render.rs` — `render_main_view` (~350 lines)**

Split into an outline function that calls:
- `compute_layout` — returns layout dimensions (areas for transcript, separator, input, status bar)
- `render_transcript_area` — fills the transcript buffer
- `render_separator` — draws the separator line with status indicators
- `render_input_area` — draws the input textarea and decorations
- `render_status_bar` — draws the bottom status line
- `render_overlays` — draws help, model settings, approval, perf overlays

Also decompose `draw_rendered_line` (~84 lines) into smaller pieces.

**`input_events.rs` — `handle_key_event` (~249 lines) and `handle_mouse_event` (~176 lines)**

Split `handle_key_event` into:
- `handle_key_event` (outline, <40 lines): dispatches to sub-handlers based on current UI state
- `handle_escape_sequence` — escape key / chord handling
- `handle_approval_key` — approval overlay key handling
- `handle_model_settings_key` — model settings overlay key handling
- `handle_input_key` — main text input key handling

Split `handle_mouse_event` into:
- `handle_mouse_event` (outline, <30 lines): dispatches by event kind
- `handle_scroll_event` — scroll wheel
- `handle_mouse_drag` — drag selection
- `handle_mouse_click` — click handling

**`notification_items.rs` — `handle_item_notification` (~248 lines)**

Split the match arms into per-item-type handler functions, each under 60 lines. The match statement itself becomes a short dispatcher.

**`notifications.rs` — `pending_approval_from_request` (~233 lines)**

Split each match arm into a named function: `approval_from_shell_request`, `approval_from_file_edit_request`, etc.

**`input.rs` — `run_conversation_tui` (~193 lines)**

Extract the inner event loop body into a `process_event` function. Extract server-message batching into `drain_server_messages`. The main loop should be ~30 lines: setup, loop { drain events, process, render, check quit }.

**`terminal_ui.rs` — `pick_thread` (~183 lines)**

Extract rendering into `render_picker_frame`, input handling into `handle_picker_input`. The loop becomes: wait for event, handle input, render.

**`mod.rs` — `parse_cli_args` (~120 lines), `run_codex_backend` (~80 lines), `run_claude_backend` (~80 lines)**

Extract the repeated backend-setup logic into a shared helper. Decompose `parse_cli_args` into: argument iteration loop, argument validation, defaults application.

**`tools.rs`, `tool_shell.rs`, `transcript_styles.rs`** — decompose any remaining functions over 60 lines.

Acceptance: `cargo test` passes. `cargo build --release` passes. No function in any source file exceeds 60 lines (verified by automated scan). `carlos perf-session` shows no meaningful regression.

### Milestone 4: Split remaining oversized files

After milestone 3, several files will still exceed 400 lines even with better function decomposition, because they contain too many concerns. This milestone splits them by domain.

**`state.rs` (~1,011 lines) — Split AppState**

`AppState` has ~17 fields and a monolithic impl block. Group the fields and methods by concern:

- Keep `state.rs` as the `AppState` struct definition and construction (~150 lines).
- Move message/transcript mutation methods to `state_transcript.rs`.
- Move rewind-related methods to `state_rewind.rs` (if enough volume).
- Keep settings, display-mode, and lifecycle methods in `state.rs`.

Alternatively, if the prior ExecPlan's sub-state extraction pattern (already done for `RalphRuntimeState`, `ViewportState`, `RuntimeSettingsState`, etc.) is sufficient, the remaining split may be to simply move method groups into the sub-state modules and have `AppState` delegate. Choose whichever approach results in files under 400 lines with coherent concerns.

**`transcript_styles.rs` (~815 lines)**

Currently contains color conversion boilerplate, styled-text segment functions, multiple wrapping function variants, and counting function variants. Split into:
- `transcript_styles.rs` (~200 lines): core style types, segment building, the main `append_wrapped_message_lines` entry point.
- `transcript_wrap.rs` (~200 lines): all wrapping/counting logic (markdown, ANSI, styled lines).
- Move the color/modifier conversion boilerplate into a small `compat.rs` or eliminate it if the ratatui version mismatch can be resolved.

**`app/mod.rs` (~765 lines)**

Contains CLI parsing, backend selection, runtime defaults, model catalog, and the `run()` entry point. Split into:
- `mod.rs` (~150 lines): module declarations, re-exports, the `run()` entry point.
- `cli.rs` (~200 lines): `CliOptions`, `parse_cli_args`, env-flag helpers.
- `backend_setup.rs` (~200 lines): `Backend` enum, `run_codex_backend`, `run_claude_backend`, shared setup logic.

**`input_events.rs` (~726 lines)**

After milestone 3 decomposes the large functions, this file should be close to the cap. If still over 400, split key-event handling from mouse-event handling into two files.

**`render.rs` (~644 lines)**

After milestone 3 decomposes `render_main_view`, this file should shrink significantly. If still over 400, the sub-renderers extracted in milestone 3 can live in their own files.

**`notifications.rs` (~605 lines), `perf_session.rs` (~824 lines), `overlay_render.rs` (~465 lines), `picker_render.rs` (~451 lines), `tools.rs` (~692 lines), `text.rs` (~421 lines), `tool_shell.rs` (~417 lines)`**

Assess each after milestones 2-3. Some will have shrunk below 400 from function extraction. For those that haven't:
- `perf_session.rs`: split synthetic data generation into `perf_synthetic.rs`.
- `notifications.rs`: split approval-request parsing from server-message routing.
- `overlay_render.rs`: after repetition elimination (milestone 5), likely shrinks below 400.
- Others: evaluate case-by-case after prior milestones land.

Acceptance: `cargo test` passes. `cargo build --release` passes. No source file exceeds 400 lines. `carlos perf-session` shows no meaningful regression.

### Milestone 5: Eliminate repetition

Apply the style guide's "3+ similar patterns must be abstracted" rule.

**Overlay drawing** (`overlay_render.rs`): extract a shared `draw_overlay_box` helper that takes dimensions, title, and a content-rendering closure. The four overlay functions become thin wrappers that supply their specific content.

**Color/modifier conversion** (`transcript_styles.rs`): if these conversions exist because of a ratatui/ratatui-core version mismatch, investigate whether upgrading or unifying the dependency eliminates the need. If not, write a generic conversion or a macro.

**Count/append wrapping** (`transcript_styles.rs` / `transcript_wrap.rs`): unify the count and append paths. One approach: a single wrapping function that takes an `enum { Count, Append(&mut Vec<...>) }` mode parameter, or a trait-based visitor. The goal is that the wrapping logic exists once, not twice per format.

**History record parsing** (`claude_backend/history.rs`): extract shared structure between `append_assistant_history_record` and `append_user_history_record` into a common helper that takes the role-specific differences as parameters.

**Settings cycling** (`runtime_settings_state.rs`): write a generic cycle helper that takes the current value and the list of options, then call it from each settings method.

**Duplicated `reasoning_summary_text`**: keep one canonical version (in whichever module owns reasoning summaries) and have the other import it.

Acceptance: `cargo test` passes. No pattern is copy-pasted 3+ times. Grep for the specific patterns listed above confirms they have been unified.

### Milestone 6: Import hygiene

This is the final polish pass. For every `.rs` file:

1. Remove any remaining glob imports (`use super::*`, `use foo::*`). Replace with explicit imports.
2. Group imports into three blocks separated by blank lines: `std`, external crates, `crate::`/`super::`.
3. Sort alphabetically within each group.
4. Move `#[cfg(test)]` imports into test modules, not production code.
5. Remove unused imports (the compiler will flag these).

Acceptance: `cargo test` passes. `cargo clippy` passes (or pre-existing clippy issues are unchanged). No glob imports remain. Imports are visually grouped and sorted.

## Concrete Steps

All commands below should be run from the repository root: `/var/home/wegel/work/perso/carlos`.

1. Record the starting baselines:

       find src -type f -name '*.rs' -print0 | xargs -0 wc -l | sort -nr | head -n 20

   To find functions over 60 lines, use an approximate scan:

       rg -n '^(\s*pub(\(.*\))?\s+)?(async\s+)?fn ' src/ --type rust

   Manually inspect the largest functions in each file. Record in the Progress section.

2. After every meaningful code change, run:

       cargo test

3. After every runtime-affecting change, run:

       cargo build --release
       cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

4. After each milestone, run a frozen-session perf check:

       target/release/carlos perf-session /tmp/carlos-perf-session-019cdf51-snapshot.jsonl --width 160 --height 48

   If the snapshot file is unavailable, use `--synthetic`:

       target/release/carlos perf-session --synthetic --width 160 --height 48

5. After each milestone, run:

       cargo clippy 2>&1 | head -50

   Note any new warnings introduced by the changes.

6. Keep this ExecPlan current after every slice by updating `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`.

## Validation and Acceptance

The minimum acceptance bar for each milestone is:

- `cargo test` passes with no new failures.
- `cargo build --release` passes.
- `carlos perf-session` shows no meaningful regression on the frozen snapshot (or `--synthetic`) relative to the current baseline.
- No regression in transcript fidelity, selection/copy behavior, tool rendering, reasoning rendering, approval/Ralph flows, or Claude/Codex backend behavior.

The overall acceptance bar for this ExecPlan is:

- **No source file exceeds 400 lines** (test files may exceed this if justified, but should be noted).
- **No function exceeds 60 lines.**
- **Every module has a `//!` doc comment.**
- **Every public type, function, and method has a `///` doc comment.**
- **Files over 150 lines have `// --- Section ---` landmarks.**
- **No glob imports.**
- **Imports are grouped (std / external / crate-local) and sorted.**
- **No pattern is copy-pasted 3+ times.**
- **The final engineering review is not `FAIL`.**

Final acceptance is not "the files are shorter." Final acceptance is that the codebase reads like it was written by a disciplined team: files have visible architecture, functions tell stories via the outline pattern, documentation orients the reader, and repetition has been abstracted. An expert Rust programmer should find it pleasant to read.

## Idempotence and Recovery

Each milestone produces small, buildable commits. If a split reveals no real boundary or worsens readability, stop and record that discovery rather than forcing a mechanical extraction. If a function decomposition makes the code harder to follow (excessive indirection for trivial logic), keep the function as-is and document why in the Decision Log.

Use the temporary-file-and-rename pattern for binary installation:

    cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

All changes are additive refactors with no behavioral change. Any commit can be reverted independently. If a milestone introduces a perf regression, bisect the commits within that milestone and revert the offending change.

## Artifacts and Notes

Current baseline (from end of EXECPLAN_006, approximate):

    full_layout ~48 ms
    full_draw   ~1.0 ms
    append_total p50 ~0.68 ms

File-size baseline recorded at plan creation (top 15 source files by line count):

    claude_backend.rs      1,594
    state.rs               1,011
    perf_session.rs          824
    transcript_styles.rs     815
    mod.rs                   765
    input_events.rs          726
    tools.rs                 692
    render.rs                644
    notifications.rs         605
    protocol.rs              494
    overlay_render.rs        465
    notification_items.rs    463
    picker_render.rs         451
    runtime_settings_state.rs 432
    text.rs                  421
