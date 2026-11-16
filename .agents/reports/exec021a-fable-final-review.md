# EP021a end-of-plan review

**Verdict: PASS WITH ISSUES.** No BLOCKER and no code defect found; one MAJOR planning issue about what the iPhone actually needs, and six MINOR items.

## Scope and limits

- **Read:** `AGENTS.md`, `PROGRAM_PLAN.md` (first 110 lines), the final report, the direct engineering review, the command records and `tmp/main-screen.md`.
- **Code inspected:**
  - reply routing and budget: `client_panel_reads.rs`, `client_budget.rs`, `panels/mod.rs`;
  - task list and windows: `panels/tasks.rs`, `task_panel.rs`, `task_panel_drive.rs`, `task_panel_replies.rs`, `task_panel_windows.rs`;
  - details, activity, settings: `task_detail_panel.rs`, `task_activity_panel.rs`, `task_activity_replies.rs`, `settings_panel_drive.rs`;
  - daemon and core: `command_info.rs`, the result sort in `carlos-core/src/engine/results.rs`, the settings records in `carlos-proto`;
  - consumers: terminal adapters `tasks_windows.rs` and `tasks_details.rs`, and the second consumer `shared_panels/consumer.rs` and `wire.rs`.
- **Not read:** `docs/engine-architecture.md`, the EP021a ExecPlan, `exec021a-final-measurements.json`, and anything under `/tmp/carlos-ep021a` (access outside the worktree was denied).
- **Not run:** nothing was built or tested. Gate and measurement figures are taken from the report as stated.
- **Not verified:** the owner-private `WindowContext`/`ActivityContext` wrappers in core. I confirmed only the three shared settings records.

## Findings

### MAJOR (planning, not a code defect): most of the new shared surface is not on the specified iPhone screen

- **Evidence:** `tmp/main-screen.md` §11 asks the engine for the session list, paged entries, streaming, tool output and labels, approvals, context usage, turn start and Ralph. §10 lists model/thinking settings, rename and rewind as "not designed yet". None of the queue, context-defaults, goal, task list, older windows, task details, overview or activity readers appear on that screen.
- **Consequence:** the 4958 added lines buy terminal de-duplication and a proven ownership pattern. They do not directly make the v1 phone simpler; the code the phone needs is EP021b. The risk is EP022 exporting all nine panel families through UniFFI, each needing an owned mirror record, for screens that do not exist.
- **Action now:** none on EP021a code.
- **Action in EP021b/EP022:**
  - derive the UniFFI export list from §11, not from `carlos-client`'s public functions (177 outside `panels/` and tests);
  - leave task and queue panels Rust-only until a phone screen is designed;
  - give EP021b an explicit line budget.

### MINOR 1: `Effects` can be dropped silently, and close calls differ in whether that matters

- **Evidence:** there is no `#[must_use]` anywhere in `carlos-client`. The terminal discards the result at `src/app/remote/tasks_details.rs:22`, `settings.rs:208-209`, `settings_save.rs:105-106`, `queue.rs:111` and `tasks.rs:72`.
- **Why it is safe today:** those five closes return only panel changes (`queue_panel.rs:41`, `settings_panel.rs:57`, `context_panel.rs:58`, `task_overview_panel.rs:35`, `task_detail_panel.rs:80`).
- **Why it is a hazard:** `close_task_panel`, `close_task_activity` and `close_goal_panel` call `drive` and return outgoing frames whose request ids are already registered. A Swift caller dropping one leaves a native window open and a request slot occupied.
- **Action now:** add `#[must_use]` to `Effects`; it is one line.

### MINOR 2: both consumers hand-build the same `TaskDetailTarget`

- **Evidence:** `crates/carlosd/tests/shared_panels/consumer.rs:147-154` and `src/app/remote/tasks_details.rs:29-36` assemble the same window/revision/selected-id triple, each with an `expect`.
- **Consequence:** this is the cursor knowledge the plan meant to keep out of platforms, and a third copy would land in Swift.
- **Action in EP021b:** add one `Client` method that returns the target for the current selection.

### MINOR 3: adding a reader touches about seven hand-maintained lists, and the reply router has a wildcard

- **Evidence:** four lists of eight in `client_panel_reads.rs:10-87`, two in `panels/mod.rs:64-121`, and nine `panel_changed` calls in `client_budget.rs:39-47`. The router falls through at `client_panel_reads.rs:29` with `_ => Some(result)`, while `command_info.rs:63-104` is exhaustive.
- **Consequence:** a forgotten `cancel_*`, `disconnected` or `retained_bytes` entry in EP021b leaks a request id or leaves a cache uncharged without a compile error.
- **Action:** make the `ReadReply` match exhaustive as the first EP021b commit. Do not introduce a trait or framework; the repetition is tolerable at eight families.

### MINOR 4: the 64 MiB budget evicts whichever session crossed the line (speculative for the phone)

- **Evidence:** `client_budget.rs:12-63` detaches the session whose reply tipped the total. On a phone that is usually the foreground session. It reports through a `RequestFailure` with `id: None`, code -32010 and prose. The constant is compile-time (`:9`), and the total is re-summed across all sessions on every driven dirty panel (`task_detail_panel.rs:165`, `task_panel_drive.rs:110`, `settings_panel_drive.rs:58`, `client_panel_reads.rs:53-55`).
- **Consequence:** the re-summing is cheap at the session counts involved. The policy and the untyped signal are what a phone would trip over.
- **Action in EP022:** decide a typed effect and a constructor-supplied limit against the real Swift call site.

### MINOR 5: internal-invariant `expect`s sit on reply paths (speculative; not a regression)

- **Evidence:** `task_panel_windows.rs:112,159,183-189`, `task_panel_replies.rs:113`, `task_activity_replies.rs:61`. I traced these and found none reachable.
- **Consequence:** a panic in the terminal is a crash report. Behind UniFFI it becomes an app abort or a poisoned client.
- **Action in EP022:** have the facade treat a panic as a fatal disconnect, or add a randomized reply-order test for `receive`.

### MINOR 6: the batch sort assumes per-client operation ids only increase

- **Evidence:** `crates/carlos-core/src/engine/results.rs:88-94` sorts by client string, then numeric id. This matches the narrow claim in the report: order within one owner batch only.
- **Open question:** I did not check whether a restarted client with the same identity can reuse lower ids while older operations are still pending. If it can, the sort would invert them.
- **Action:** confirm the allocator, or note the assumption beside the sort.

## Checks that held

- **Stale replies and late opens:**
  - a reply for a cancelled request is dropped without touching the panel;
  - a late window or activity open that was retired is closed by its returned id (`task_panel_replies.rs:24-31`, `task_activity_replies.rs:26-33`);
  - close queues drain before the `wanted` check (`task_panel_drive.rs:93`);
  - disconnect clears pending closes and marks the captured window stale (`panels/tasks.rs:187-198`).
- **Budget overflow path:** it drops the pending close list, but the following `Unsubscribe` removes the daemon's windows and activities for that session (`carlosd/src/manager/read_commands.rs:74-87`).
- **Readonly details after F4:** `retire_task_window_details` cancels the read, marks the panel stale and keeps accepted fields (`task_detail_panel.rs:109-126`).
- **Wire bytes:** `ContextSet`, `SettingsSet` and `ToolLabelsSet` use `#[serde(flatten)]` on the shared records (`carlos-proto/src/command.rs:82-98`), and core consumes the same types (`engine/dispatch.rs:171-180`, `admission.rs:21-23`).
- **`command_info.rs`:** exhaustive, no wildcard; mutation ingress is derived from the retry class, so a new mutation cannot land in the read allowance.
- **Prohibited items:** none found in the files read: no sockets, threads or clocks in `carlos-client`, no generic framework, no body truncation.

## Code quality and bloat

The code is disciplined. Each panel's state consists of the request ids, sequence-fenced future pages and retired/close lists that the protocol requires; none of it is a framework. The terminal adapters are thin: `tasks_windows.rs` is 72 lines of getters and `tasks_details.rs` keeps only display policy. The second consumer drives the whole flow in about 380 lines with no cursor logic, apart from the target assembly in MINOR 2.

The owner's worry is still partly justified. The total is 94,854 lines against 40,629 on main, EP021a added a net 2,787, and EP021b and EP022 will add more. This plan moved complexity to the right place but did not shrink it. Against the specified iPhone screen its benefit is mostly indirect. The line count will be decided by whether EP021b/EP022 build only what §11 needs.
