# Replace Carlos's Claude pipe reader with the official Agent SDK

## Scope and working rules

Carlos keeps its Rust terminal UI and uses a small local TypeScript helper to run
the official Claude Agent SDK. The helper starts the user's installed, unmodified
Claude CLI. The CLI keeps its normal account, settings, and plugins. No component
copies login credentials or replaces the agent with a direct Messages API client.
The helper never chooses bypass mode unless the caller explicitly enables it.

The user requested sequential work: one implementation subagent, a parent review
after each task, then corrections or the next task. We remain in Discussion Mode.
Do not enter Ralph Mode, edit its unrelated goals, install Carlos, or commit until
the parent/user authorizes that step. Preserve Codex and Grok behavior and leave
the sibling `vwl` repository and its experimental plugin alone.

## Progress

- [x] Parent verified installed Claude 2.1.322 and official SDK 0.3.322 against native declarations and official docs.
- [x] Parent ran the unchanged baseline: 444 Rust tests passed, 1 ignored; all 57 terminal workflows passed. Artifacts: `target/ui-artifacts/run-yXqU6T`.
- [x] Task 1: added a separate helper package, pinned SDK and lockfile, local protocol, SDK callbacks, process cleanup, and build entrypoint. Rust kept its existing backend at that handoff.
- [x] Task 1: source tests passed through the real SDK and deterministic fake CLI, without account access or inference.
- [x] Task 1: compiled helper passed the same tests from temporary folders with only system tools in `PATH`.
- [x] Task 1: `scripts/test.sh` passed 444 Rust tests (1 ignored) and all 57 terminal workflows; artifacts: `target/ui-artifacts/run-5M2klH`.
- [x] Parent re-reviewed task 1 and returned PASS after independently rerunning all 23 source and 23 compiled tests, with 112 assertions per run.
- [x] Task 2: connected Rust to the compiled helper; native startup callbacks, streams, controls, models/context, prompt IDs, and failed turns passed agent and parent checks.
- [x] Parent Task 2 review returned PASS WITH ISSUES; corrected the single minor empty-error fallback and added its regression.
- [x] Parent checked the final Task 2 correction and returned PASS after reading the fix and independently passing all 16 focused tests plus rustfmt.
- [x] Parent assigned Task 3 after its final Task 2 PASS. Carlos now has native question/form/browser/plan controls and independent drafts; the required runner passed, and parent returned final PASS after the narrow confirmation fix.
- [x] Task 4 replaces private Claude session readers/writers with official SDK list/read/rename/fork APIs, bounded history pages, saved source records, and helper generation guards.
- [x] Task 4 final required runner passed: 448 Rust tests (one ignored), 47 source and 47 compiled helper tests (253 assertions each), 103 terminal workflows, Codex contracts, and three native Codex monitor cases. Artifacts: `target/ui-artifacts/run-NkhbHr`.
- [x] Parent final Task 4 PASS, independently verified in `run-ULcUj5`.
- [x] Task 5 implements thinking/state/task views, per-task stop, queue/steer, quotas/notices, and native resets; the required runner passes on the final behavior.
- [x] Task 5 parent final review returned PASS before Task 6.
- [x] Task 6 builds paired packages, disables runtime autoload, adds native localhost contracts, fixes strict Clippy checks, compares release performance, and verifies one full subscribed-native UI turn.
- [x] Task 6 final required release/native runner passed in `run-A3NJP9`.
- [x] Parent Task 6 PASS; parent explicitly authorized the reviewed paired local install.
- [x] Installed the reviewed package with a durable rollback backup; actual installed launcher passed all four native localhost UI contracts, help, and 417 file checksums.
- [x] Applied the user's final local layout requirement: regular Carlos and helper executables in `~/.local/bin`, with the reviewed bytes unchanged; all four direct installed native UI cases passed again.

## Surprises & Discoveries

- SDK 0.3.322 removed the old V2 `createSession` API. The helper uses `query` with an open async input stream.
- The SDK needs `systemPrompt: {type:"preset",preset:"claude_code"}` and explicit `settingSources` to use native project/user behavior. The helper requires the caller to choose those sources.
- SDK `sdk.mjs` contains the runtime code this helper needs. Bun can compile it without installing the SDK's peer packages or alternate native Claude packages. The lockfile records optional metadata; the build omits those packages.
- The SDK forwards current user origins and priorities. The helper requires both; automatic Ralph prompts use `auto-continuation`, never `human`.
- Native permission requests include their own request ID and child-agent metadata. The helper keeps a map, so one unanswered child request does not replace another.
- The helper must read past main result messages. A fake CLI test sends a child-task event after the result and verifies that all fields survive.
- The SDK may still be awaiting startup or a control reply when Carlos closes stdin. The helper races each pending request against its own close signal and kills the SDK child.
- Bun embeds its runtime; the current Linux x64 helper is about 101 MiB. A later packaging task must account for that size.
- Parent review found that `onUserDialog` alone does not opt a host into native dialogs. The helper now requires caller-declared `supportedDialogKinds` and defaults to none. The fake CLI now checks the declaration before sending a normal dialog.
- SDK `Query.readMessages` reads CLI output independently of the public iterator and stores events in an unbounded internal queue. The helper now uses the published process-spawn hook to cap native frame size and unread event bytes/count before the SDK stores them.
- Bun's Node-compatible stdout and subprocess readers can buffer beyond their visible stream queues. The helper now writes directly to stdout's file descriptor. A separate Python host creates a real blocked OS pipe for the tests.

## Decision Log

- Keep the helper wire separate from Codex's UI wire. The helper sends correlated replies, raw SDK events, and native callback frames. Rust will translate only where the UI needs it.
- Require an absolute native CLI path. Never use the SDK's bundled fallback Claude executable.
- Preserve omitted model/effort/permission choices, rather than imposing SDK host defaults over native settings.
- Return permission and question answers through `canUseTool`, `onElicitation`, and `onUserDialog`. Never synthesize assistant tool results or pretend that automatic prompts came from a human.
- Do not accept persistent permission-rule writes yet. The future Rust UI must present those choices and the helper must validate them before it forwards `updatedPermissions`.
- Keep experimental SDK quota APIs outside core behavior.
- Keep every new source module below 400 lines and each function focused.
- Remember active control IDs plus the most recent 1,024 completed IDs. Require callers to preserve lifetime uniqueness; keep prompt UUID deduplication for the session.
- Cap queued helper output at 4 MiB; cap native lines at 4 MiB and unread SDK events at 8 MiB or 4,096 events. Await event writes, keep stdin independent, and stop the native child when a cap is exceeded. Never silently drop a normal event to fit the cap.
- After shutdown/EOF/failure only, allow stdout 1,000 ms to drain, then fail on stderr and exit. Ordinary unanswered user callbacks have no deadline.

## Parent review and corrections

Parent's first task 1 review returned FAIL. Parent independently reran the initial
9 source and 9 compiled tests successfully, then found three gaps: the helper did
not declare dialog kinds; stdout could accumulate without a byte cap; completed
request IDs remained in memory forever. The implementation now addresses all
three. The agent waited for parent review before starting Rust work.

The pinned published SDK has contradictory unknown-dialog comments:

- [`Options.onUserDialog`](</tmp/carlos-claude-research-5wa_berh/sdk/package/sdk.d.ts:1840>) calls cancellation the “required answer for an unrecognized” kind.
- [`SDKControlRequestUserDialogRequest`](</tmp/carlos-claude-research-5wa_berh/sdk/package/sdk.d.ts:5338>) says a host that did not declare a kind “must not answer it” and “never with {behavior: 'cancelled'}”. It says the CLI discards an error response while keeping the dialog pending.

The helper follows the latter explicit transport contract for an undeclared
kind. It emits `unsupported_dialog` with the native request ID and kind, then
throws from the SDK callback. The SDK sends an error response. The helper never
claims the user dismissed that unknown dialog, and Rust can show the error while
the user interrupts or closes the session. Tests cover declared dialogs, absent
and unsupported declarations, and an unsolicited undeclared dialog.

The supported process hook caps the SDK's private event queue; merely awaiting
the public iterator would not cap it. Tests include a genuine blocked OS pipe,
native event and synchronous callback floods, callback answers and live model
controls before output resumes, complete ordered frames after drain, and bounded
shutdown/EOF when the host never reads. No model call is needed.

Parent then re-reviewed Task 1 and returned PASS. Rust work began only afterward.

### Task 2 review and corrections

Rust now forwards complete controller frames through a Claude-specific envelope.
The UI translates SDK messages; the TypeScript helper never emits Codex-shaped
messages. Rust starts the helper without waiting for SDK hooks, so native startup
permissions can appear and receive answers before the initialization reply.

Parent found these core issues during implementation, and the agent corrected
them before the full Task 2 review:

- A direct stdin write could hang before shutdown's deadline began. One bounded writer thread now owns stdin; callers wait at most 500 ms, input frames are capped at 4 MiB, and the shutdown deadline includes the write. Unknown acceptance stops the session instead of retrying. A rejected oversized frame leaves the session usable.
- An unbounded Rust event channel would relocate the helper's backlog. Rust now allows 1,024 queued frames and charges at most 8 MiB until the UI handles them, including frames moved through intermediate queues. An overflow stops the helper and reports a final error.
- PATH lookup could reject an unusable shadow before reaching a valid later executable. It now skips those entries. The helper parser uses the host's absolute-path rules, and Windows lookup includes native `.exe` names. Windows runtime/package checks remain for Task 6.
- The first interrupt fixture used an invalid terminal reason. It now uses SDK `aborted_streaming` and `aborted_tools`. The UI captures current prompt IDs before dispatch and rolls that intent back if the control fails. Terminal tests cover result-before-receipt and receipt-before-result, including Ralph stopping in both orders.
- A result can answer several consumed user UUIDs. Rust tracks the whole native list; terminal tests cover coalesced prompts as well as separate steering cycles and both result/idle orders.
- Substring filters could discard assistant text containing a protocol method name. Rust now filters the parsed method exactly; a terminal test streams both affected literals.
- Native startup observations could overwrite a saved Carlos model, while a shared saved Codex/Grok model could leak into Claude. Rust validates saved IDs against native values and resolved IDs, rejects foreign choices and their effort, and falls back to the observed native model. Native observations stay separate from explicit Carlos overrides; saving appearance alone does not pin a native model mode or effort.
- Rust checks the child handle under its lock before signaling and gives the helper its native shutdown grace before a fatal kill. Parent raised this as a final review point, then confirmed the existing guard and recorded no remaining PID finding.

The full parent review returned PASS WITH ISSUES after its independent green
run. Parent identified one remaining minor: an empty `errors` array produced an
empty string that hid `result`. Rust now ignores blank joined errors and blank
result strings, falls back to a useful `Claude turn failed`, and retains failed
turn status and Ralph stopping. The focused regression covers both fallback
cases. New public entrypoints now have concise doc comments.

Parent's final correction review returned PASS. Parent independently ran all 16
focused Claude tests and `cargo fmt --check`. Parent did not require another
broad run after the focused fallback regression; the full-run counts below are
the earlier 459-pass result, not a claim that all 460 non-ignored tests reran.

The mandatory runner now explicitly builds and checks the helper before terminal
workflows. CI installs tested Bun 1.3.10. Carlos still performs no build, package
download, or credential extraction when it launches.

An existing scanner test twice read pyte's resized copy of the old row before
Carlos drew the new frame. The test now waits for the fixture's `91%` text at
the new right edge before checking motion. Its original motion and row-equality
assertions remain unchanged; the renderer did not change.

### Task 3 findings and decisions

Carlos now draws a separate editor for each native question. AskUserQuestion
answers retain the original input and use its question text as map keys; selected
labels join with `, ` and Other carries the user's Unicode text. Native plans
show the callback's plan text and file path, then return allow/deny/cancel to the
same actual request ID. Carlos never sends a follow-up prompt to accept a plan.

The agent checked the official [user-input guide](https://code.claude.com/docs/en/agent-sdk/user-input),
pinned `sdk.d.ts`/`sdk-tools.d.ts`, and [MCP form/browser specification](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation).
The ExitPlanMode input declaration lists only deprecated `allowedPrompts` plus
unknown fields. An authorized isolated probe ran installed Claude 2.1.322 through
the compiled SDK helper against a localhost fake Messages endpoint. Its real
callback enriched input with `plan` and `planFilePath`, plus `toolUseID` and
`requiresUserInteraction`. Evidence: `/tmp/carlos-plan-probe-on0z0odc/frames.jsonl`;
probe script: `/tmp/carlos-native-plan-probe.py`. The probe used an isolated config
and fake local key, never the user's account or a real model endpoint.

Parent and agent found these issues while reviewing Task 3, and the agent corrected them:

- Server regular expressions could block the helper's input reader. The helper now checks accepted answers in a disposable copy of itself, caps concurrent checkers at four, kills a checker after one second, and kills it on native cancellation, explicit decline/cancel, or shutdown. The native question stays pending after a bad answer; no deadline applies while a person thinks.
- Prototype names could satisfy missing fields, and ambiguous enums could hide constraints. The helper now checks own properties and exact supported schema shapes. It rejects unsupported constraints visibly. Pinned `validator` 13.15.35 and `ajv-formats` 3.0.1 handle mailboxes, URIs, dates, and RFC3339 timestamps, including leap seconds, without normalizing the user's value.
- The first AskUserQuestion checker compared serialized object-key order. It now compares object contents; a real-SDK regression covers reordered keys and rejects changed questions or missing answers.
- F8 already toggles performance tools. Native questions now advertise F10 for interrupt and retain F8's existing behavior. Terminal tests exercise F10 both before startup finishes and while a form waits; live model/effort controls leave drafts intact.
- Small overlays could hide controls or the caret. Carlos now pins the requesting server/tool above a separate answer caret, bounds footer/body rows, supports editing at 36 by 12 cells, and shows resize/decline/cancel/interrupt below that size. The native paste handler consumes fallback paste without changing either draft. The 18 by 10 check pastes extra text, resizes back, and confirms the original answer remains.
- A browser server can finish before the user answers its callback. Carlos now tracks the server and elicitation ID from callback arrival, retains early completion without auto-consenting, handles completion once before or after consent, and drops declined/cancelled records.
- JavaScript can round an exact Rust integer. The helper now rejects integers outside its safe range with a precision error and leaves the original editor text intact. Tests reject `9007199254740993`, accept `9007199254740991`, and retain ordinary finite number behavior.
- The old history path could create an actionable plan approval with a synthetic ID. With parent agreement, Carlos no longer replays that approval. A terminal test resumes a historical pending plan and answers only the actual native startup request. The parser still existed at that Task 3 handoff; Task 4 removes it below.

The first full Task 3 run passed 464 Rust tests (1 ignored) and all 36 source and
36 compiled helper tests, with 188 assertions per helper run. Two of 89 terminal
workflows found phone-test issues: a footer clipped its last word, and a test sent
arrow keys before the resized question frame appeared. The footer now fits;
traces confirmed native choice movement 0→1→2, and the phone test now waits for
the actual resized frame before checking the next visible choice. Temporary
traces were removed. Parent agreed with that focused correction. Artifacts:
`target/ui-artifacts/run-BX60yZ`. The final required runner passed; see below.

### Task 3 parent review and focused correction

Parent independently ran the full required command and returned PASS WITH ISSUES:
465 Rust tests passed (1 ignored), all 36 source and 36 compiled helper tests
passed with 188 assertions each, all 89 terminal workflows passed, and Codex
contracts plus all three native-monitor workflows passed. Parent artifacts:
`target/ui-artifacts/run-maxxYi`; parent also passed the diff whitespace check.

Parent identified one minor: default-to-no question prompts named y even though
their answer editor submits with Ctrl+S. Carlos now chooses the initial and
second-confirmation wording from the actual presence of answer fields. Forms
name Ctrl+S and Alt+N; plain permissions retain y and n. The same native ID still
owns the two-step approval state. The focused terminal regression selects an
AskUserQuestion answer, checks that the first Ctrl+S sends no callback answer,
checks that y on the review page neither approves nor changes that answer, then
checks that the second Ctrl+S returns the original native answer.

Parent requested targeted checks for this wording-only correction, without
another broad run. All five question Rust tests and all 16 SDK Rust tests passed.
All 35 Claude terminal workflows passed, including the new Ctrl+S case and both
existing plain-permission confirmation cases. Artifacts:
`target/ui-artifacts/task3-confirmation-correction`. Rustfmt and the diff whitespace
check passed. The earlier full-run counts above remain 89 terminal workflows;
this focused check includes the newly added case and does not claim another full
run. At that Task 3 handoff, the next session task still awaited parent review.

## Checks and evidence

Run the complete helper check from the repo root:

```sh
BUN_BINARY=/home/wegel/.bun/bin/bun bash scripts/build-claude-controller.sh
```

The script installs only pinned build packages, checks TypeScript, runs the real
SDK/fake CLI tests, compiles the helper, then reruns the tests against the compiled
executable. The helper never downloads a package on launch. Test homes and Claude
folders are temporary; the fake CLI makes no model call.

Task 1 result: the full script passed on 2026-11-10 with Bun 1.3.10. Both source
and compiled runs passed 9 tests and 54 assertions. Tests also verified native
callback cancellation and shutdown during an unfinished startup or control call.

After the first parent review, the same full script passed with 23 source tests
and 23 compiled-helper tests, 112 assertions per run. The corrected helper keeps
controls live through a blocked OS pipe, preserves 16 SDK frames and 23 replies
after the host resumes, and exits through explicit failure paths for permanent
blockage, native event floods, and synchronous callback floods. No shared Rust
code changed, so the parent did not require another Rust/UI run for these fixes.

After any backend wire change, repository rules also require:

```sh
bash scripts/test.sh
```

Parent's unchanged synthetic baseline (1,000 turns, 160 by 48 terminal): replay
51.06 ms; full layout 15.90 ms; scroll p95 0.89 ms; typing p95 0.68 ms. Parent
preserved the installed pre-migration binary at `/tmp/carlos-before-claude-sdk`.

Task 2 agent check:

```sh
BUN_BINARY=/home/wegel/.bun/bin/bun bash scripts/test.sh --codex-contract
```

The agent's final full run passed 459 Rust tests (1 ignored), 25 source helper
tests and 25 standalone helper tests (121 assertions each), all 70 terminal
workflows, the installed Codex 0.167.0 contract checks, and all three native
Codex monitor workflows. Artifacts: `target/ui-artifacts/run-Zb4Tmn`.
Parent independently passed the same checks. Parent artifacts:
`target/ui-artifacts/run-A1yxvK`.

After the focused empty-error correction and doc comments, the agent ran:

```sh
cargo fmt
cargo test claude_sdk_tests -- --nocapture
git diff --check
```

All 16 focused Claude UI/SDK tests passed. Parent explicitly did not require
another broad run for this message-only correction and documentation.

The optional `cargo clippy --all-targets -- -D warnings` check reported 21
diagnostics. A clean `git archive HEAD` snapshot produced the same 21 diagnostics
with the same toolchain, so the agent left those unrelated fixes outside Task 2.
Baseline log: `target/claude-controller/clippy-baseline.log`.

### Task 3 final checks

The agent ran the required command after the phone corrections:

```sh
BUN_BINARY=/home/wegel/.bun/bin/bun bash scripts/test.sh --codex-contract
```

All 465 Rust tests passed (1 ignored), all 36 source and 36 compiled-helper
tests passed with 188 assertions per run, all 89 terminal workflows passed, the
installed Codex 0.167.0 contracts passed, and all three native Codex monitor
workflows passed. Artifacts: `target/ui-artifacts/run-JUBRAO`.

The 19 new terminal workflows assert both screen contents and native/helper
messages. They cover single/multi/Unicode questions, typed form defaults and
limits, correction after validation/reply errors, independent child drafts,
active/queued cancellation, both sides of parent-result order, browser consent
and early completion, plan allow/decline/cancel with no extra prompt, unknown
dialogs, startup/live controls, default-to-no, suppressed rules, historical fake
plan removal, and phone editing/resize/fallback.

Parent requested `Server config source: unknown` when MCP supplies no source,
without mentioning SDK internals. The agent also replaced direct map indexing
with safe lookups for incomplete native server metadata and added its regression.
The final full run includes both changes. Rustfmt and the diff whitespace check
pass. The latest optional strict Clippy run reported the same 21 pre-existing
diagnostics, with no question-module diagnostic; log:
`target/claude-controller/clippy-task3.jsonl`.

The final code also passed all 80 filtered Claude Rust tests serially and all 34
Claude terminal workflows (`target/ui-artifacts/task3-final-claude`). One earlier
parallel filtered run exposed two existing HOME-mutating fixture locks:
`claude_backend_tests::env_lock()` and `runtime_tests::runtime_env_lock()` do not
protect each other. The tilde-config assertion failed and poisoned its mutex;
no new question test failed. The serial rerun and final default-parallel full
runner passed. Task 4's legacy session fixture replacement should remove or share
those locks. Task 3 adds no process-environment mutations to Rust tests.

The repository style script's `--all` mode cannot read the tracked old pipe files
removed by uncommitted Task 2. A direct check of all present Rust source/test
files passes its module-size and explicit-import rules; the new UI functions
also stay within 60 lines. No commit was made to work around that script limit.

### Native subscription check

Parent checked the installed native CLI's filtered auth status: logged in via
`claude.ai`, first-party provider, Pro subscription. Neither `ANTHROPIC_API_KEY`
nor `ANTHROPIC_BASE_URL` was set. Parent then ran the compiled helper in a fresh
temporary folder, selected no model override, and sent one minimal unclassified
prompt at native `later` priority. The real installed CLI returned
`CARLOS_SUBSCRIPTION_OK` with success, no error, and clean shutdown. This checked
the helper, SDK, real native executable, and subscription together without
copying tokens or requiring an API key. Parent already answered the user's
subscription side question; do not reopen it. A full Rust UI/native subscribed
turn remains a later final check. The implementation agent made no real model call.

## Outcomes & Retrospective

Carlos now shows native AskUserQuestion choices/custom text, typed MCP forms,
browser consent, and plan approvals through actual SDK callback IDs. Carlos retains each
child's draft, handles active/queued cancellation by ID, keeps the main composer
intact, and preserves unanswered children across a main result. Carlos escapes
native labels before display. Per-call allow/deny remains the only rule choice;
Carlos declares no generic dialog kinds and keeps their error/interrupt path.

Carlos session list/read/rename/fork now use the SDK as described below. Richer
task state, explicit queue controls, final packaging, performance comparison,
and the final full Rust/native turn remain later reviewed tasks. No commit or
install has been made; Task 5 must wait for parent review of Task 4.

### Task 3 final parent review and Task 4 start

Parent passed Task 3 after reading the form confirmation correction, running
its two native form/plain-permission terminal cases in
`/tmp/carlos-parent-task3-confirm`, five question Rust tests, and the whitespace
check. The earlier independent full run `run-maxxYi` passed 465 Rust tests
(one ignored), 36 source and 36 compiled helper tests (188 assertions each),
89 terminal workflows, Codex contracts, and three native Codex monitor cases.
The narrow text fix did not trigger another broad run.

Task 4 now replaces private Claude file readers/writers with published SDK
session functions. No other agent, commit, or install is authorized.

Real native/local fake Messages probes used only a local endpoint and fake key.
`/tmp/carlos-session-api-probe-mu9l7yxy` verifies custom config paths, Unicode
rename surviving the next turn, and public compaction records.
`/tmp/carlos-session-api-probe-wqloe8gw` verifies a human prompt queued while
another turn runs: the consumed public record retains human origin and has no
queued placeholder flag. An otherwise identical automatic continuation lacks
origin. Carlos must preserve runtime fields beyond the TypeScript declaration
and persist its own submitted sources outside Claude storage. A queued flag
alone cannot prove automatic authorship. Completed `/compact` command records
are local command transcript entries, not editable prompt text.

The plain SDK message reader returns only the active post-compaction chain.
The later public-import probe below found the earlier entries, so Carlos retains
them through SDK readers rather than dropping pre-compaction history.

### Task 4 decisions and parent corrections

Carlos now uses published `listSessions`, `getSessionInfo`,
`getSessionMessages`, `renameSession`, and `forkSession`. The helper lists
programmatic SDK and native sessions together, honors the native config folder
and worktree rules, and pages catalog/history replies. Continue resolves a
specific SDK catalog ID before the UI loads and resumes it. New queries choose
a UUID before startup; initialization may not emit system/init until a prompt.

The plain message reader omits older compacted records and some system metadata.
Parent found the published alpha `importSessionToStore` reader and authorized a
small version-pinned read-only adapter for opaque entries. A local probe proved
that the importer supplies the full original entries and that `forkSession`
accepts pre-compaction streamed user UUIDs directly. The helper fills a temporary
SQLite cache through the importer, then asks `getSessionMessages` to read each
cached prefix. The SDK resolves active/abandoned branches and kept-message links;
Carlos reads only the compact boundary's `logicalParentUuid` to find older
history for display. No production code locates or parses private JSONL files,
rewrites UUIDs, or copies native support trees. SDK functions own all mutations.

Parent noticed that a raw parent-link walk could skip boundaries when compact
kept messages point into older records. The helper now delegates those links to
the public SDK reader. Real-SDK fixtures cover kept segments, arbitrarily ordered
kept UUIDs, duplicate UUIDs, abandoned branch exclusion, and forks before both
precompact and postcompact humans. An initial fixture put an abandoned branch
after every active record but declared an older compact logical parent; the SDK
fork then disagreed with the requested prefix, and Carlos correctly rejected it.
The final fixture places the abandoned branch before the later active entries,
as normal native append order does. Carlos retains the strict disagreement guard.

Parent also required canonical content checks and an exact final fork length.
Carlos compares stable message contents without relying on object-key order or
optional derived flags, validates native `forkedFrom` UUID metadata when supplied,
and rejects missing, extra, or changed fork rows before switching sessions.
The helper waits at most five seconds for the exact selected live UUID; shutdown
can interrupt that wait. It never falls back to a stale prefix. The first prompt
returns a fresh empty session UUID, with no cloned history.

A 512 KiB page target initially rejected larger valid records. Carlos now sends
those records in base64 chunks, with exact offsets and a 16 MiB reassembly cap.
The helper bounds each import at 256 MiB, total cached-prefix materialization at
512 MiB, visible rows at 100,000, nested compactions at 256, retained snapshots at
two, and active readers at two. The public SDK still materializes complete
arrays internally; paging bounds host frames and Rust backlog, not that SDK
allocation. Limit failures remain explicit. Unknown and child records remain in
the cache and have separate paged metadata methods for Task 5.

The native CLI preserves human origin but can drop automatic continuation origin
on disk. Carlos records each submitted UUID/source before dispatch in its own
SQLite file below XDG_STATE_HOME/carlos, separated by native config-folder hash.
It stores no prompt text or credentials. Public positive human metadata remains
authoritative; Carlos's ledger distinguishes otherwise identical automatic text.
Consumed queued human prompts remain editable; a queued flag alone cannot prove
automatic authorship. SDK-derived local-command/compact-summary/child records do
not enter ordinary human history. Legacy main-thread unknown-source prompts stay
labeled unknown and require explicit F4 selection while rewind is open; only the
new edited prompt gets Human origin. No text matching guesses authorship.

Rust loads history in pages before requesting asynchronous native startup. Hooks
stay answerable before the startup reply. Rewind runs the SDK fork off the UI
thread, starts a new helper generation, then queues the edited human prompt until
that helper is ready. Old close/cancel/model frames cannot affect the new helper.
Starting, Ready, and Closed are separate states. A closed helper rejects a new
submit visibly and preserves the draft. Parent caught the after-switch startup
failure path and late successful startup replies: failures now stay Closed,
restore the edited draft, and cannot reopen until an explicit new generation.
Pre-switch fork failure keeps the original ready session and rewind edit.
Explicit model/effort choices survive a switch; inherited native observations do
not become accidental overrides. Controls and callbacks retain their own IDs.

Claude `/rename` now saves through the SDK without a model prompt or transcript
change. A real-native/local-fake endpoint probe confirmed a Unicode name survives
the next turn. Terminal checks also verify rename after restart and unchanged
rewind content. Old private-session parser/writer tests and the HOME-mutating
picker fixture have been replaced; helper/test processes use isolated environment
values rather than process-wide HOME changes.

Task 4 focused checks before the full runner: all 46 source and 46 standalone
helper tests passed, 251 assertions each; 55 filtered Claude Rust tests and six
new shared-routing session lifecycle tests passed. Terminal checks cover repeated
edits, empty first edit, rename/restart, picker/Continue, unknown-source branching,
compaction, closed drafts, fork startup hooks, pre-switch failure, post-switch
startup failure, and paged history. The exact final full-run counts and artifact folder are recorded below.

The first full Task 4 runner (`run-pqC1BT`) passed 446 Rust tests (one ignored),
46 source and 46 compiled helper tests (251 assertions each), and 101 of 102
terminal cases. The new first-edit test sent Esc after a text delta but before
native result/idle, accidentally interrupting the turn. Its new native result/
idle fence fixed that test assumption. The next full runner (`run-pxV9ER`) passed
all 102 terminal cases, the same Rust/helper counts, Codex 0.167.0 contracts, and
all three native Codex monitor workflows. Removed parser fixtures explain the
lower Rust count; Carlos did not skip remaining tests.

A final parent read caught concurrent snapshot insertion exceeding the two-cache
limit. The helper now evicts again after an awaited read and a simultaneous-read
regression passes. The final lifecycle sweep also found unsent human prompts
queued during failed startup. Carlos now preserves every such draft, drops only
automatic work, and leaves a newer composer untouched. F5 loads saved drafts only
into an empty composer. Edited prompts receive the same recovery if the user has
typed a newer draft. Repeated close/error frames cannot duplicate saved drafts.
A new terminal case and focused Rust checks pass; one final full runner will
include these corrections and the corresponding tests before handoff.

The optional strict Clippy check still fails on existing diagnostics: its test
build now reports 20, versus the verified baseline/Task 3 count of 21. Removing
the private history scanner also removed its `lines_filter_map_ok` warning. No
new Task 4 module adds a diagnostic. Log:
`target/claude-controller/clippy-task4.jsonl`. The source/module style checks and
whitespace check pass; the repository style script cannot read tracked files
removed by this uncommitted migration, so the agent checked all present files.

### Task 4 final checks and handoff

`rtk proxy bash scripts/test.sh --codex-contract` passed on the final source:
448 Rust tests passed, one ignored; 47 source and 47 standalone helper tests
passed with 253 assertions each; all 103 terminal workflows passed; installed
Codex 0.167.0 contracts and all three native Codex monitor workflows passed.
Artifacts: `target/ui-artifacts/run-NkhbHr`. This includes startup queued-draft
recovery, edited/newer-composer recovery, and the concurrent snapshot cap check.
A preceding attempt caught a strict TypeScript possibly-undefined assertion in
the new cache test; the agent added the explicit non-null assertion and reran the
whole required runner successfully. No production behavior was changed for that
TypeScript test correction.

A separate real-native/local-fake Messages probe created a temporary Git main
repository and linked worktree. The native query ran in the linked worktree;
SDK list/info/read/rename/fork calls from the main repository found its session.
`includeWorktrees:false` excluded it. The original had four messages and its
inclusive-prefix fork had two, with the expected new UUIDs. Script:
`/tmp/carlos-session-worktree-probe.ts`; evidence:
`/tmp/carlos-session-api-probe-q3s8xy07/linked/public-apis.json`, plus root
stdout/stderr logs. This used fake credentials and only a localhost endpoint.

Rustfmt, whitespace, all present Rust module/import checks, and new/changed
function-size checks pass. The final sources add no new Clippy diagnostic; the
optional strict run remains red on 20 older diagnostics after this task removed
one obsolete scanner warning. The agent removed the old independent HOME picker
mutex/fixture and its unused imports. No new test mutates process-wide HOME.

Parent independently passed focused session/fork checks during review, then
started its final broad run after the agent reported the green result. Task 4
now waits for that parent verdict. No commit, install, extra agent, account model
call, or Task 5 work occurred. Remaining material limits: the read-only opaque
compact/fork metadata adapter is tied to the pinned SDK/native format; SDK reads
can still materialize full arrays within the stated caps. Rich child/task views,
task stop/thinking controls, release packaging, final performance comparison, and
the full Rust UI/subscribed-native check remain later reviewed tasks.

### Task 4 parent PASS and Task 5 start

Parent returned final PASS after independently running
`scripts/test.sh --codex-contract`: 448 Rust tests passed (one ignored), 47 source
and 47 compiled helper tests passed (253 assertions each), all 103 terminal
workflows passed, and Codex contracts plus three native monitor workflows passed.
Parent artifacts: `target/ui-artifacts/run-ULcUj5`. The final whitespace and
source review also passed. Task 5 now covers thinking/state/task views and stop,
explicit queue/steer choices, native quotas/notices, and native conversation
resets. The same sole implementation agent continues without commits, installs,
other agents, or Task 6 work.

### Task 5 contracts, implementation, and interim review

Carlos now renders native thinking blocks in its reasoning rows. The real native
localhost probe emitted separate one-block assistant snapshots with the same API
message ID but different native UUIDs. The translator therefore deduplicates each
block instead of dropping an entire same-ID snapshot. Redacted-thinking estimates
remain progress only. The thinking probe recorded estimate 43 but billed output 3;
Carlos never substitutes the estimate for native usage. Evidence:
`/tmp/carlos-session-api-probe-kso3eu37/live.jsonl`; probe scripts
`/tmp/carlos-task5-thinking-probe.py` and `/tmp/carlos-task5-state-probe.ts`.

Carlos reads native status to show requesting, compacting, requires-action,
running, and idle labels. Carlos gives each compaction attempt one changing
notice, including failure and boundary messages. Context reads use only `getContextUsage({detail:'summary'})`, carry a
context revision, and cannot restore old counts after compaction/reset. Native
plugin/hook/settings text and session-writes-lost notices remain visible without
blanket failure of healthy sessions. Unknown additive messages stay ignorable.

F2 shows actual native task/run IDs, descriptions, summaries, status, usage,
output path, and parent metadata. The CLI sends its complete background task list; Carlos replaces the previous
list with it. Ambient/skipped watchers do not make Carlos busy. Carlos keeps
recent details after a task leaves that list and updates one transcript row per
task ID. Carlos compares run IDs as JavaScript plain strings (UTF-16 order),
without parsing an undocumented format. Parent caught late terminal progress overwriting summaries and recent-run
cache churn forgetting an active newer run. The tracker now freezes terminal
progress and also checks retained live/detail run IDs. Focused churn/order tests
pass. Public `stopTask` runs asynchronously; its acknowledgement only says stop
requested. Carlos now declares `perTaskStopAffordance` after adding that UI. It
never forces `agentProgressSummaries`, which could launch auxiliary model calls.

F3 selects visible Enter behavior: native now/steer or later/queue. Actual keyboard
and dictation prompts remain human; Ralph remains automatic. Native results can
coalesce UUIDs or precede idle; Carlos completes each owned UUID once and releases
Ralph only for an actual owned turn. Native automatic task-notification turns
have no human UUID and cannot consume the next waiting human. Interrupt replies
retain native `still_queued` UUIDs and show that those accepted inputs can still
run; Carlos neither cancels them through private controls nor resubmits them.

The stable stream quota contract reports utilization fractions 0–1 and Unix reset
seconds. The optional experimental usage API reports percentages 0–100 and ISO
reset dates. Primary source: official Python SDK
[RateLimitInfo and ConversationResetMessage comments](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/types.py).
The pinned TypeScript declaration supplies the optional API's percent units.
Carlos keeps buckets separate, validates sparse additive unifiedWindows data,
and uses only shared seven_day for the weekly badge. Missing/expired/unavailable
values remain absent. The helper isolates the long-named experimental method,
uses skipBehaviors:true, permits one original in-flight read, and returns after
five seconds without making startup or prompts wait. The real native fake-API-key
probe returned rate_limits_available:false/null and emitted no quota event despite
representative localhost headers. No account/auth workaround was used; deterministic
fixtures plus the official unit comments cover available values.

Native reset evidence required a distinction between the opaque screen key and
the next real native session ID. The official Python SDK explicitly documents it;
our real-native/localfake probe confirmed that only the observed next init ID
exists through SDK info/read after the following prompt. Evidence:
`/tmp/carlos-session-api-probe-n_za1k1a/persisted.json`; probes
`/tmp/carlos-task5-state-probe.py` and `.ts`. Parent approved a single validated
reset permit that adopts the next native init ID, while ordinary mismatched init
IDs still fail. Public persisted reads corroborate the probe only; production
prompt delivery does not wait for disk flush. Main events/control replies carry
an epoch inside each helper generation, and context replies also carry revision.

Parent's real-native background Bash probe proved that /clear keeps process tasks
alive: reset(old ID), an old-ID background level, actual-new-ID init, clear result,
then new-ID task completion and one native automatic notification turn. The output
file legitimately retains the original session folder. Evidence:
`/tmp/carlos-parent-reset-task-erfnezv5/frames.jsonl`; script
`/tmp/carlos-parent-reset-task-probe.py`. Parent's pipelined clear+human probe also
proved that the next accepted human runs once in the new session, with no idle
between results and no SDK user replay. Its exact UUID persists in the actual new
session. Evidence: `/tmp/carlos-session-api-probe-yh7dr_t9/pipeline.json`; script
`/tmp/carlos-parent-pipeline-reset.ts`. Carlos therefore keeps an in-memory accepted
UUID/text/source ledger and restores surviving human text only when native UUID
markers show consumption. Clear itself is not an editable human or Ralph turn.
All those probes used isolated fake credentials and localhost Messages endpoints,
with no remote/account inference.

Callbacks run independently of the SDK event iterator, so an early child callback
can overtake a buffered reset event. Actual native callback IDs and cancellation
remain authoritative; Carlos never assigns them the screen's last-seen epoch.
Parent caught reset_for_forked_thread consuming a queued callback while the active
one was temporarily removed. Carlos now removes/restores the entire queue around
transcript-only reset, preserving every draft and confirmation. A real terminal
case keeps a typed native form, two queued child permissions, an early new callback,
and exact cancellation/answer IDs through a reset, including phone caret return.
Parent also caught phone task focus being hidden below accepted prompts, foreground
stop silently unavailable, and lifetime child/stop caches. Selected task details
now precede prompts/quotas, known running foreground IDs can call public stopTask,
and unavailable controls say so. A separate bounded stop-state map survives detail
row eviction. Completed child cursors retire before a healthy long session reaches
the 128 simultaneous/recent cursor cap; a 180-child churn regression retains all
visible output. The transcript keeps snapshot UUIDs to avoid duplicate replay.

Interim checks passed: 84 filtered Claude Rust tests; 51 helper source/standalone
tests (272 assertions each in the current full runner); targeted real-terminal
state checks in `/tmp/carlos-task5-ui-second` (ten cases), with later phone focus
and reset queue checks in `/tmp/carlos-task5-ui-third` and
`/tmp/carlos-task5-ui-reset-final`. Earlier test failures exposed a fixture's
hardcoded foreign session ID, a redacted-thinking start clearing its progress
label, and a fake callback counter counting SDK cancellation as a user answer;
those fixtures/reader assumptions were corrected. The required full runner is
active; its final result and artifact folder will be recorded before handoff.

Task 6 remains separate. Parent verified that Bun 1.3.10 compiled helpers otherwise
autoload .env and bunfig from the working folder. Task 6 must disable both with
--no-compile-autoload-dotenv and --no-compile-autoload-bunfig and prove hostile-cwd
environment isolation before installation. No Task 6 build flags or install work
were changed during Task 5.

The first full Task 5 run (`target/ui-artifacts/run-OTePrL`) passed 469 Rust tests
(one ignored), 51 source and 51 standalone helper tests (272 assertions each),
and 112 of 113 terminal cases. The queue/task hint crowded the older-session F4
rewind hint at 54 columns. Carlos now yields that row to rewind; the focused old
unknown-source terminal case passes again. Codex contracts did not run in that
attempt because the terminal stage correctly stopped the script on failure.

Parent's next narrow read caught default task selection changing silently as the
native live set changed. F2 now pins the initial selected ID, keeps it after
completion while its row exists, and disables stop if that row is evicted. It
never silently chooses another task; arrows explicitly select a new target.
A two-task terminal test finishes A after opening the panel: x sends no stop for
B; Down then x sends exactly B's native ID. A cache-eviction unit check also passes.
Parent also pointed out the pinned reset declaration's explicit wording that
trigger is informational: consumers reset on every conversation_reset and treat
missing/future trigger values as unspecified. The reader's overly strict trigger
whitelist was removed; current-session/event/UI-key/one-shot guard checks remain.
New focused cases cover both omitted and future trigger values, plus the existing
foreign-session refusal. Focused terminal artifacts:
`/tmp/carlos-task5-selection-fix`. A fresh full runner will cover the final code.

### Task 5 final checks and handoff

The second full attempt (`run-5WNqeF`) passed 471 Rust tests (one ignored), both
51-test helper runs, and every visible terminal assertion, but a terminal wire
poll parsed a native JSONL record before its final newline arrived. The agent
changed the native/helper test pollers to read complete newline-terminated byte
records; malformed complete records still fail. This also avoids decoding half
a Unicode character. The focused pending-form/settings/interrupt case passes at
`/tmp/carlos-task5-wire-read-fix`. A final child check also uses the task's retained
tool_use_id when older child frames lack agent_id and their terminal event omits
the caller. A 140-child regression proves those cursors retire correctly.

The final required command passed:
`rtk proxy bash scripts/test.sh --codex-contract`.
Artifacts: `target/ui-artifacts/run-6SyPt4`.

- Rust: 472 passed, one ignored.
- Helper: 51 source and 51 standalone tests passed, 272 assertions each.
- Terminal: all 114 workflows passed, including phone task focus, reset callback drafts/caret, selected task completion, steering, queue survival, and the older-session rewind hint.
- Installed Codex 0.167.0 contracts passed; all three native Codex monitor workflows passed.
- Rustfmt, whitespace, present Claude module/function sizes (400/60), and public entrypoint comments pass.
- Strict Clippy remains red on the same 20 older test-build diagnostics as Task 4; there are no new Task 5 diagnostics. Log: `target/claude-controller/clippy-task5-final.jsonl`.

After the green runner, the agent added only two missing public-entrypoint doc
comments and this handoff text, with no behavior change. Task 5 waits for parent
review. No commit, install, additional agent, remote/account model inference, or
Task 6 implementation occurred.

Material limits remain explicit: the optional quota method and alpha session
reader APIs can change on SDK upgrades; additive unifiedWindows quota data is
validated best effort. Carlos does not combine model-specific quotas or infer
missing values. A genuine excess of active native child streams/live task IDs
stops the session through shutdown instead of hiding controls or growing memory
without a bound. Recent task details/run tombstones are bounded, while retained
live runs protect active identities after cache churn. Task 6 still owns shipped
platform packaging, hostile-cwd environment checks, strict Clippy cleanup,
performance comparison, and the final full Rust UI/subscribed-native check before
any approved install.

### Task 5 parent PASS and Task 6 start

Parent independently passed the final required runner in `run-oUzQtF`: 472 Rust
passed (one ignored), 51 source and 51 standalone helper tests (272 assertions
each), 114 terminal workflows, Codex contracts, and all three native monitors.
Parent's final read and whitespace check passed. Task 6 now prepares paired
release packages, runtime environment isolation, native Claude localhost/UI
contracts, strict lint fixes, docs, and release/performance checks. Installation
still requires the parent's explicit final authorization; no commit or publishing
is authorized. The same sole implementation agent continues.

### Task 6 package and runtime choices

The Python helper builder pins Bun 1.3.10 and SDK 0.3.322, maps each Rust release
target to Bun's matching compiled target, and writes the helper next to Carlos.
The package builder checks both executable headers and CPU targets before it
copies the pair. It includes the licenses of bundled JavaScript/Rust components,
pinned Bun/WebKit notices and source references, checksums, and a JavaScript
bundle that users can rebuild with a modified matching Bun runtime. RELINK.md
uses Bun's actual pinned build instructions and --compile-executable-path.
No native Claude executable or authentication material enters the archive.

All four Bun runtime autoload flags are disabled: dotenv, bunfig, tsconfig, and
package JSON. The standalone smoke test uses a poisoned working folder and an
empty PATH, checks the schema worker without Node/Bun, and proves that an
explicit process CLAUDE_CONFIG_DIR still selects the intended SDK session.
Parent independently confirmed the original dotenv defect is fixed:
`/tmp/carlos-parent-dotenv-h851etdg/parent-autoload-fixed.json`. Parent also
passed standalone package smoke in `/tmp/carlos-parent-final-package-smoke`
and actual installed-native idle controls in
`/tmp/carlos-parent-final-native-controls`, with no inference.

Release jobs now use Ubuntu 22.04 x64, macos-15-intel x64, macos-latest ARM64,
and Windows x64. Each host builds its matching pair and runs standalone and
actual native CLI startup/control smoke checks. Linux also runs the full PTY
suite. The Windows/macOS jobs have not run locally or been published. Their
helpers were cross-built and their headers checked locally; that proves the
target, not runtime behavior. The Linux helper is 104,771,661 bytes; the Intel
Mac helper is 67,179,936 bytes, ARM Mac 62,175,632, and Windows 115,529,216.
Cross-build evidence: `target/package-helper-checks/results.json`.

### Task 6 native contracts and release fixes

Four new contracts copy the Rust/helper pair outside development lookup paths,
then drive the installed unmodified Claude 2.1.322 through the real terminal.
The native process uses isolated homes, fake credentials, and a localhost-only
Messages endpoint. Cases cover streams; SDK list/Continue/resume/Unicode rename;
first/later fork edits with unchanged original files; actual native question and
plan callbacks; background Bash completion/stop; clear with a queued human UUID;
and persisted human-only rewind after resuming. Native tool policy remains
authoritative, including when it already permits the fixture's Bash command.
A separate terminal cap test proves a child-stream overflow closes the native
process, drops automatic work, and leaves typed human text recoverable.

Strict Clippy now passes without blanket allows. The scoped fixes group drawing
coordinates, use idiomatic iterator/number helpers, simplify matches, and clean
older test diagnostics. Both default and no-default-features Clippy passed;
`cargo build --release --no-default-features` passed. The later full release
runner restores the default-feature release artifact. The style script includes
present untracked Rust files and skips deleted tracked files.

The first release attempt, `run-kkXuCM`, passed the Rust/helper/terminal/Codex
stages and three native cases. The session case picked a fork sharing the
original title. The test now gives the original a unique persisted title and
explicitly selects its visible picker row. The next complete agent run,
`run-mNNHhn`, passed 472 Rust tests (one ignored), 51 source plus 51 standalone
helper tests (272 assertions each), package smoke, 115 terminal workflows,
Codex contracts plus three native monitors, and all four native Claude cases.

Parent's independent `run-rVQUVT` passed every stage except the native background
resume case. A native task notice legitimately started work before the startup
reply; the test pressed Escape while that work still ran, so Carlos correctly
interrupted it instead of opening rewind. The test now waits for the latest
new-helper native state to be idle and for the visible activity badge to clear.
It keeps all persisted history assertions and uses no fixed delay. Six fresh
focused repeats passed in `/tmp/carlos-task6-native-resume-fence-1` through `-6`.
No production binary changed for that correction. A final full runner follows.

### Task 6 performance and single subscribed UI proof

The agent compared five alternating runs of the old and new release executables
using 1,000 turns, 24 tool lines, 160x48 cells, and seed 1. Median before/after
milliseconds: replay 48.12/46.68, full layout 14.70/14.17, scroll p95 0.84/0.85,
typing p95 0.64/0.59. Evidence: `target/release-checks/performance.json`.
Parent independently repeated the same alternating comparison: 46.55/46.57,
14.12/14.14, 0.80/0.79, 0.59/0.59. Evidence:
`/tmp/carlos-parent-final-perf-6nw7_izz/results.json`. Neither run shows a material
regression; the earlier baseline's higher numbers reflect host variation.

The one authorized live UI proof succeeded and must not be repeated. A fresh
temporary Carlos session used the packaged pair, normal native Pro login, no API
key/base URL override, no Ralph, disabled hooks/plugins, and denied tools. Its
only prompt was “Reply with exactly CARLOS_NATIVE_UI_OK. Do not use tools.”
The actual SDK result was success, is_error false, result exactly
CARLOS_NATIVE_UI_OK; the assistant row matched and no tool ran. Private evidence:
`/tmp/carlos-task6-subscribed-ui-one/proof.json` (directory 0700, files 0600).
The manual probe records filtered authentication status only and never copies
credentials. All other model requests in Task 6 went to localhost.

### Task 6 final checks and review handoff

The final required command passed:
`rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract`.
Artifacts: `target/ui-artifacts/run-A3NJP9`.

- Rust: 472 optimized tests passed, one ignored; strict default-feature Clippy passed.
- Helper: 51 source and 51 standalone tests passed, 272 assertions each.
- Package hostile-cwd/self-worker smoke passed without Node/Bun in PATH.
- Terminal: all 115 deterministic workflows passed, including actual process exit after a native child cap.
- Installed Codex 0.167.0 contracts and all three native monitor cases passed.
- Installed native Claude 2.1.322: all four packaged localhost UI contracts passed.
- Parent independently passed all four corrected native cases using the earlier archive's extracted pair in `/tmp/carlos-parent-final-native-claude`. Its other release checks had already passed in `run-rVQUVT`.
- Rustfmt, present-file Rust style, whitespace, and changed Python syntax/module/function limits passed.

The final Python size audit found two older fake-CLI dispatchers above 60 lines.
The agent extracted their branches into helpers without changing the fixtures'
behavior. Afterward, source and standalone helper suites each passed all 51
tests/272 assertions, and all 12 activity terminal cases passed in
`/tmp/carlos-task6-fixture-extract-final`. One earlier focused activity run, made
concurrently with helper stress tests, timed out waiting for the existing task
stop error screen: native wire recorded the error, while the helper reply stayed
pending until cleanup. Evidence remains in `/tmp/carlos-task6-fixture-extract`.
The isolated case and serial 12-case rerun passed without a change. That result
did not explain the failure. The native error's request ID was `x667uf88if`,
matching the native request; helper request `rust-6` remained pending until
shutdown and then received `closed`. The screen wait expired after eight
seconds. The old fake CLI recorded the error before printing/flushing stdout,
so its log cannot prove the helper or SDK received those bytes before shutdown.
No SDK defect or root cause is established by that trace.

Bounded follow-up passed 4,000 fast error controls and another 4,000 after a
prompt/idle with 10 ms between controls, each across four concurrent compiled
helpers. Evidence: `/tmp/carlos-task6-stop-errors-627yw0f1` and
`/tmp/carlos-task6-stop-errors-nazhxyus`. A temporary compiled observer logged
native forwarding, SDK pending-map registration/lookup/handler calls, and fake
CLI post-flush records: 500 complete terminal stop-error runs passed in
`/tmp/carlos-task6-ui-flush-t0huuhze`. Another 500 passed with the unchanged
packaged helper in `/tmp/carlos-task6-ui-flush-7ozjmzd6`. The SDK registers its
pending handler before writing the request; the proposed registration race was
ruled out by its pinned source. Temporary probes did not alter product binaries.

An earlier terminal repeat also exposed a separate test fence: F2 could open
before Carlos received the task row, correctly pinning an empty selection.
Parent accepted the bounded investigation and requested two fixture corrections:
the fake CLI now records server frames only after stdout flush, and the phone
workflow waits for its visible answer before opening F2. Future wire evidence
therefore proves flush completion. The original unexplained failure remains
recorded, without a speculative SDK/runtime workaround. After those two fixture
changes, all 12 activity terminal cases passed in
`/tmp/carlos-task6-fixture-flush-final`; both source and standalone helper suites
passed all 51 tests/272 assertions each. Python syntax/400/60 checks and whitespace
also passed. Product/archive binaries remain unchanged from the full green run.

Final Linux package: `target/releases/final-linux-x86_64-sdk-0.3.322/`.
Archive: `target/releases/carlos-linux-x86_64-sdk-0.3.322.tar.gz`, 45,245,051 bytes,
SHA-256 `66a3286cd6cdaef7ae7da57c652dc8cbb509605a6df0c36ceaa8bc71de0bd532`.
All 417 packaged file checksums passed. The package contains 410 notice files
under `third-party-notices/`, their inventory/source records, plus
`relink/controller.js` and `relink/README.md`. Carlos is 10,699,040 bytes and the
helper is 104,771,661 bytes. No runtime rebuild/download is required.

The existing `/var/home/wegel/.local/bin/carlos` remains the old regular file;
no helper is installed next to it or in the fallback libexec folder. Proposed
install after explicit parent approval: copy the entire reviewed package to
`/var/home/wegel/.local/lib/carlos/releases/0.1.0-claude-sdk-0.3.322-cd12cd6e/`,
back up the old executable, then atomically switch `~/.local/bin/carlos` to that
folder's Carlos executable. Its compiled helper stays adjacent. The later smoke
must launch that actual installed symlink with CARLOS_CLAUDE_CONTROLLER unset,
use only localhost fake credentials, and avoid the test harness's package-copy
step. No further live turn, commit, publishing, or install has occurred.

Parent review and explicit local install authorization are the only remaining
steps. Foreign-host CI execution and a full modified Bun/WebKit rebuild remain
unverified locally; neither is claimed as a Linux runtime test.

### Parent Task 6 PASS and authorized local install

Parent returned PASS after reviewing the final package, workflows, notices, lint
fixes, corrected test fences, and the bounded stop-error investigation. Its
independent `run-rVQUVT` passed all 472 Rust tests, 51+51 helper tests, 115 terminal
cases, Codex contracts, and three monitors; the corrected four native cases then
passed on the extracted reviewed archive pair in
`/tmp/carlos-parent-final-native-claude`. Parent also passed the final post-flush
activity cases in `/tmp/carlos-parent-final-activity-postflush`, archive identity,
format/style/whitespace, standalone/native idle controls, and performance checks.
Parent explicitly authorized the local paired install and localhost-only smoke.

The reviewed package now lives at
`/home/wegel/.local/lib/carlos/releases/0.1.0-claude-sdk-0.3.322-cd12cd6e/`.
`/home/wegel/.local/bin/carlos` atomically switched to a symlink selecting its
Carlos executable. All notices, relink files, and checksums were copied with the
pair. The durable old executable remains at
`/home/wegel/.local/lib/carlos/backups/carlos-before-claude-sdk-20261111T013316Z-4e7b9a66`.
The installer synced the copied files and backup before switching the launcher.
Its first final path assertion compared `/home` against the equivalent resolved
`/var/home` alias; normalized-path verification then passed without reinstalling
or changing the reviewed files.

Installed SHA-256 values:

- Carlos: `cd12cd6e99ac7db84a2f0e1d027638c8ab48ad3eb07ecfced8008eb6c1c5bd87`.
- Helper: `278fdcc4539b1973d00b62ad2075af079dc18bfa2ea0d0dbc2a961938ccc2744`.
- Rollback executable: `4e7b9a6654ecfc7c077322a25572271d5546697b92c3e15bbf86e885e45b3c59`.

All 417 installed file checksums and `--help` passed independently for both agent
and parent. The final smoke launched the actual `/home/wegel/.local/bin/carlos`
symlink, bypassed the test's package-copy function, and checked Linux process
executable paths to prove Rust loaded the adjacent installed helper. It also
checked that CARLOS_CLAUDE_CONTROLLER was absent and every native process used
isolated homes/config, fake credentials, and the local test endpoint.
All four native UI cases passed in 7.173 seconds, including callbacks, native
plan approval, sessions/rename/forks, and background stop/reset/pipelined input.
Evidence: `/tmp/carlos-task6-installed-smoke-ihb8olbm/`, including
`installed-executables.json`, `installed-help.txt`, and `result.json`.
Install/backup hashes: `target/release-checks/local-install.json`.
Parent independently reviewed the direct-launch smoke driver and its successful
result, then verified the durable backup matches the pre-task installed binary
byte for byte. Parent confirmed all requested work is verified; no further test
or implementation change remains.

The native Claude executable's hash remained unchanged after smoke. No login or
user settings were changed, no additional live account turn ran, and no commit,
push, release publishing, or extra agent occurred. The local migration and
authorized install are complete. Matching foreign-host CI runtime checks and a
modified Bun/WebKit rebuild remain the explicit unverified platform limits.

### User correction: regular executables in the bin folder

The user required `~/.local/bin/carlos` to be the executable itself, not a
symlink. Parent explicitly authorized that correction. The agent verified the
reviewed package again, copied both executables to temporary regular files in
`~/.local/bin`, synced them, and replaced the helper first and Carlos second.
Both `/home/wegel/.local/bin/carlos` and
`/home/wegel/.local/bin/carlos-claude-controller` are now regular executable files
with the same reviewed hashes recorded above. No sibling helper existed before
this install, so no older helper needed a backup. The durable original Carlos
backup and the complete versioned package, notices, relink files, manifest, and
checksums remain intact. README now documents regular copies as the normal
local install path.

The first regular-file smoke passed three native cases but failed the session
picker's exact ID assertion. Its screenshot selected the original while a
subsequent request opened a fork. The test could send another Down before the
previous Down's redraw, then press Enter with an extra key still pending.
Evidence remains at `/tmp/carlos-task6-installed-regular-smoke-6w0ovpvz`.
The smoke restored the old regular Carlos while the agent investigated. Parent
approved a narrow test correction: after every Down, wait until the selected
screen row's index and text change before considering another Down. The exact
original session ID assertion remains. No product binary changed or rebuild ran.

The agent restored the exact reviewed Carlos as a regular file and reran all
four actual-installed native cases. All passed in 7.679 seconds; `--help` passed.
Evidence: `/tmp/carlos-task6-installed-regular-smoke-bishmdv5/`. The driver forbids
the package-copy helper, checks both regular files, launches the actual bin path,
and verifies Linux process executable paths for both Rust and its sibling helper
under `~/.local/bin`. It proves the helper override is absent and native calls
use isolated homes/config, fake credentials, and localhost only. Final hashes
match the reviewed package; the original backup and native Claude executable
hashes remain unchanged. Updated metadata:
`target/release-checks/local-install.json`. Focused Python syntax/400/60 and
whitespace checks passed. No additional live turn, commit, push, or publishing
occurred. The regular-file layout supersedes the earlier symlink install.
