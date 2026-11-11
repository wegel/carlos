# Local protocol version 1

Each line contains one UTF-8 JSON object. Every object has `v: 1`. Requests carry
unique string IDs for the lifetime of the helper:

```json
{"v":1,"id":"r1","method":"hello","params":{}}
```

The helper sends exactly one reply for each request it can parse and correlate:

```json
{"v":1,"type":"reply","id":"r1","ok":true,"result":{"protocolVersion":1,"sdkVersion":"0.3.322","methods":[]}}
{"v":1,"type":"reply","id":"r2","ok":false,"error":{"code":"not_ready","message":"Start the controller and wait for its reply first"}}
```

The real `hello` reply lists supported methods. Bad JSON gets an error with
`id: null`; a bad envelope retains its ID when the helper can read it. Clients
must match replies by ID because callbacks and SDK events can arrive between
them. Clients must keep reading until stdout reaches EOF, including after a
`closed` event: shutdown replies and replies cancelled by shutdown may follow it.
Clients must not reuse request IDs. The helper detects duplicates while a
request is active and among the most recent 1,024 completed IDs. It does not
retain older control IDs, so callers must enforce lifetime uniqueness themselves.
A detected repeated ID receives `duplicate_id` without replacing the active request.

## Start and prompts

`start` accepts one session per helper process. Wait for its successful reply
before sending prompts or model/effort/context controls. Callback answers,
interrupt, and shutdown remain available during startup. Its reply contains the SDK's full
native startup answer, including supported models and slash commands.

```json
{"v":1,"id":"r2","method":"start","params":{"executable":"/absolute/path/to/claude","cwd":"/absolute/project","settingSources":["user","project","local"],"model":"sonnet","effort":"high","permissionMode":"default"}}
```

`executable`, `cwd`, and `settingSources` are required. `model`, `effort`,
`permissionMode`, `sessionId`, `resume`, and `continue` are optional. Omit a choice to let the native CLI
use its settings. `settingSources: []` deliberately excludes those settings
files. `resume` names a native session ID; `continue: true` asks the native CLI
to choose its latest session. The helper rejects a request that selects both.
A new `sessionId` selects a fresh UUID and cannot be combined with resume/continue.
Absolute paths follow the host OS rules, including Windows drive and UNC paths.
The helper uses Claude's normal
`claude_code` system prompt and forwards partial assistant messages.

`supportedDialogKinds` is an optional array of unique nonempty names, each
trimmed and at most 128 characters, with at most 128 names. It defaults to `[]`.
The helper passes it to the SDK's native startup request. The CLI only requests
kinds that the host declares, so callers must list only kinds their UI can
display and answer. Registering a dialog callback alone does not declare support.
Carlos declares none for generic dialogs; it handles tool permissions and MCP
questions through their separate native callbacks.

The helper accepts native modes `default`, `acceptEdits`, `plan`, `dontAsk`,
`auto`, and `bypassPermissions`. Starting with `bypassPermissions` requires
`allowDangerouslySkipPermissions: true`. A later switch into bypass mode requires
the same explicit flag at start. The helper never adds that flag itself.

```json
{"v":1,"id":"r3","method":"prompt","params":{"uuid":"11111111-1111-4111-8111-111111111111","text":"Explain this project","origin":{"kind":"human"},"priority":"later","inputKind":"prompt"}}
```

Each prompt requires a UUID, nonempty text, origin, and priority. The caller must
reuse the prompt UUID when it correlates native events with its own transcript.
The helper rejects a prompt UUID it has already accepted. A successful prompt
reply means the helper queued the message; it does not mean Claude answered it.

Human prompts may carry Carlos display metadata `inputKind: "prompt" | "steer"`.
The caller captures that choice when it dispatches the UUID; priority alone does
not classify an input. A steer requires `now`. Omitted kinds remain unknown. The
helper stores the kind separately from source in Carlos SQLite, carries it through
validated forks, and never puts it in SDK prompt frames or native session files.

Only actual keyboard or dictation input gets `origin: {"kind":"human"}`.
An automatic Ralph follow-up gets `{"kind":"auto-continuation"}`. Callers can
use `{"kind":"unclassified"}` when they cannot identify the author. The helper
passes these origins unchanged to the SDK. It never invents a human author.

The SDK defines `priority: "now"`, `"next"`, and `"later"`. Carlos uses `now` for
steering and `later` for queued messages. F3 selects the Enter behavior while
Claude works; Enter on an idle session starts a new message.
The controller forwards the selected priority. Only `interrupt` asks the SDK to
interrupt a running turn; the controller never restarts Claude to send a message.

## Live controls

- `interrupt`, with no parameters, calls `Query.interrupt()` on the same session. Its native `still_queued` UUIDs remain accepted and can still run; an interrupt does not promise to cancel them.
- `model.set`, with `{"model":"opus"}`, calls `setModel`. `model: null` restores the CLI default.
- `effort.set`, with `{"effort":"high"}`, calls `applyFlagSettings`. Accepted values are `low`, `medium`, `high`, `xhigh`, and `max`; `null` restores the native model default.
- `permission.set`, with `{"mode":"plan"}`, calls `setPermissionMode`.
- `context`, with no parameters, calls `getContextUsage({detail:"summary"})` and returns the native answer unchanged.
- `capabilities`, with no parameters, returns the SDK's cached startup answer.
- `task.stop`, with `{"taskId":"native-task-id"}`, calls public `Query.stopTask`. Its reply is `{"taskId":"native-task-id","stopRequested":true}`. Only a later native task event reports whether the task stopped.
- `usage.optional` calls the optional experimental quota reader with `skipBehaviors:true`. It returns `{available,report?,windows?,reason?}`. Each normalized window has `key,label,usedPercent,resetsAt`; percentages use 0–100 and reset times use Unix seconds. The full native report remains unchanged.
- `shutdown`, with no parameters, cancels pending callbacks, closes SDK input, and closes the SDK query.

`perTaskStopAffordance` is an optional boolean startup flag. Omit it until the
host has actual per-task stop controls. Carlos now declares it because F2 offers
those controls. An ordinary main-turn interrupt then preserves native background
work according to the SDK contract. The helper never enables progress summaries,
which could cause extra model calls.

The helper isolates the explicitly experimental quota API behind `usage.optional`.
A missing method, failed read, or five-second read deadline produces an unavailable
answer; no startup or prompt depends on it. One original read can remain in flight,
so another read returns `read_pending` instead of accumulating native requests.
An unanswered user question has no such deadline. Stable `rate_limit_event`
frames remain the core quota source. Native controls may fail; the helper returns `sdk_error` with the SDK's message. The helper does
not silently relaunch the CLI when a model or effort control fails.

## Native questions

The SDK calls the helper when Claude needs permission, an MCP question, or a
dialog. MCP servers are tools supplied by plugins or other configured services.
The helper sends a separate callback frame for every unanswered native request:

```json
{"v":1,"type":"callback","callbackId":"native-request-id","kind":"permission","payload":{"toolName":"Bash","input":{"command":"pwd"},"toolUseID":"tool-id","agentID":"child-agent-id","title":"Allow this command?"}}
{"v":1,"id":"r4","method":"callback.respond","params":{"callbackId":"native-request-id","answer":{"behavior":"allow","updatedInput":{"command":"pwd"}}}}
```

`callbackId` is the actual SDK control request ID. Permission payloads retain all
SDK metadata except the non-JSON abort signal. Tool names, tool inputs, and
plugin-provided display text are untrusted strings. The Rust UI must display
them safely and respect `defaultToNo` and `suppressAlwaysAllowRule` metadata.
The UI must track callbacks independently, including callbacks from child agents.
The controller keeps reading replies and interrupts while any callback waits.

- `permission` accepts native `{"behavior":"allow"}` with optional `updatedInput`, or `{"behavior":"deny","message":"reason","interrupt":true}`. Persistent `updatedPermissions` writes currently return `unsupported_answer`; the request stays pending.
- `elicitation` accepts native `{"action":"accept","content":{...}}`, `{"action":"decline"}`, or `{"action":"cancel"}`.
- `dialog` accepts native `{"behavior":"completed","result":...}` or `{"behavior":"cancelled"}` for caller-declared kinds. A cancellation represents the user dismissing a supported dialog.

AskUserQuestion approvals must include unchanged `updatedInput.questions` and
an `updatedInput.answers` string map keyed by original question text. The helper
compares object contents without depending on object-key order. Multi-select
answers join labels with `, `; custom text is a string answer. ExitPlanMode uses
the native permission callback's supplied `input.plan` and `input.planFilePath`.
Its answer is allow/deny, never a new prompt containing the plan.

MCP form payloads retain `requestedSchema`, the server's rules for field types,
choices, and limits. The helper adds `formIssue` for unsupported shapes. It checks
flat strings, finite numbers, safe integers, booleans, single enums, and enum
arrays before settling an accept. It supports length/pattern/format and numeric
or selection limits; it rejects unchecked schema keywords and ambiguous choices.
Integer answers must stay between -9007199254740991 and 9007199254740991 so the
SDK cannot silently round them. Missing required fields use own-property checks.

Browser payloads add the parsed `urlHost`. A URL-mode accept contains only
`{"action":"accept"}` and means consent. Decline/cancel never include form
content. Native `elicitation_complete` messages stay raw SDK events; Carlos
matches server and elicitation ID from callback arrival, retains early completion
without auto-consenting, and ignores unknown or repeated completion notices.

The helper checks accepted form/question answers in a disposable copy of the
compiled helper. At most four checkers can run, each with a one-second work
limit and a bounded reply. A slow checker gets killed; the request stays pending
with a clear error. SDK cancellation, explicit decline/cancel, and shutdown kill
an in-flight checker. A second accept during a check gets `answer_pending`.
These work limits never apply while a user considers an unanswered question.

If the CLI nevertheless requests an undeclared kind, the helper emits an error
event with `error.code: "unsupported_dialog"`, the native request ID under
`callbackId`, and `dialogKind`. It does not expose a normal callback or fabricate
a user cancellation. It throws from the SDK callback so the SDK sends a native
error response; the CLI leaves the dialog unsettled. Rust must show this error
so the user can interrupt or close the session. Controls remain available.

A bad answer leaves the callback pending so the caller can correct it. An SDK
abort sends `callback_cancelled` with the callback ID. A late answer receives
`unknown_callback`. EOF and shutdown settle permissions as denied, MCP questions
as cancelled, and dialogs as cancelled. The helper never fabricates tool results
or plan approvals; it returns answers through the SDK's native callbacks.

## Events and exit

```json
{"v":1,"type":"event","event":"sdk","message":{"type":"system","subtype":"task_notification","session_id":"native-session-id","task_id":"child-id"}}
{"v":1,"type":"event","event":"callback_cancelled","callbackId":"native-request-id","reason":"sdk_cancelled"}
{"v":1,"type":"event","event":"closed","reason":"stdin_eof"}
```

The helper forwards every SDK message object unchanged under `message`. Session,
message, parent-tool, agent, task, and future fields remain intact. Clients must
tolerate new SDK message types and fields. The helper keeps reading after a
main result, so child tasks and session state can arrive afterward. The helper
asks the CLI to emit session state through `CLAUDE_CODE_EMIT_SESSION_STATE_EVENTS=1`.

The helper wraps native stderr in `event: "stderr"` with `text`. An SDK stream
failure sends `event: "error"`, then `closed` with `reason: "sdk_error"`. Other
close reasons include `shutdown`, `stdin_eof`, `sdk_eof`, `start_failed`,
`stdout_error`, `stdin_error`, `SIGTERM`, and `SIGINT`. The helper closes the child
process and resolves pending local replies when it closes. SDK failures exit
with code 1, normal EOF/shutdown with 0, SIGINT with 130, and SIGTERM with 143.
The helper does not retry a failed or closed session.

## Bounded output

The helper awaits each SDK event write and sends only one frame to its host pipe
at a time. It retains at most 4 MiB of queued stdout frames, including the frame
currently being written. Native callbacks and stderr can arrive synchronously;
if those frames would exceed the limit, the helper closes with `output_overflow`.
It keeps reading stdin and returning native callback answers while output waits.
The helper writes directly to the pipe because Bun's `process.stdout.write`
can otherwise buffer bytes beyond the Node stream's visible queue.

The SDK reads its native stdout independently of its public event iterator.
Waiting on our writer alone would therefore leave the SDK's private queue
unbounded. Through the supported `spawnClaudeCodeProcess` hook, the helper counts
native event bytes before the SDK reads them and releases each event's budget
when the public iterator yields it. The native reader allows at most 8 MiB or
4,096 unread events, and at most 4 MiB in any native JSON line. Control frames,
keep-alives, transcript mirror frames, and host-only session-state events do not
enter the SDK's public event queue and do not count toward its unread budget.
The helper closes with `native_backlog_overflow` or `native_frame_overflow` when
the native CLI exceeds those limits. These checks match the pinned SDK's frame
filter and must be reviewed when the SDK changes.

The helper immediately stops the native child on an output or native-budget
failure. Such failures exit with code 1 and also report their reason on stderr,
which the host can still read when stdout is blocked. After any close, the helper
allows stdout to drain for up to 1,000 ms. A host that still does not read causes
`output_drain_timeout`, exit code 1, and a stderr message. Abnormal pipe failures
can discard queued frames and leave a partial final JSON line; callers must
treat that exit as a failed session. A host that resumes within the grace gets
already queued replies in order. No deadline applies to an ordinary unanswered
user callback while the session remains open.

If an OS write remains blocked after fatal output cleanup, a final watchdog
terminates the helper three seconds after that failure. It never applies to a
user who is still considering a normal permission or question.

## Native session APIs

Session requests can run before `start`; they do not launch the native CLI or
send model prompts. Every request supplies `cwd`. Native environment/config
folder and SDK worktree rules apply.

- `sessions.list` accepts `offset` (default 0), `limit` (1..64, default 64), and `includeWorktrees` (default true). It always includes programmatic sessions and returns `{sessions,offset,nextOffset}` with unchanged SDK summary objects.
- `sessions.info` accepts `sessionId` and returns the SDK metadata object or null.
- `sessions.rename` accepts `sessionId,title` and saves the title through the SDK, then returns current metadata.
- `sessions.read` accepts `sessionId,offset,limit` and returns `{messages:[{message,source,inputKind?}],offset,nextOffset,snapshotId}`. `message` retains native fields; `source` is human, automatic, unknown, or system. Pass the returned `snapshotId` on later pages to keep the same read. A new read without that ID creates a snapshot.
- `sessions.children` pages the imported child's `{subpath,records}` catalog for the same snapshot. `sessions.metadata` pages opaque main metadata, or child entries when `subpath` is supplied. These records are preserved for later task views; they do not create actionable historical approvals or main-thread human prompts.
- `sessions.forkBefore` accepts `sessionId,beforeMessageId`. It waits for that exact prompt UUID, calls the published inclusive fork API at the preceding record, checks contents and mapped UUIDs, and returns `{sessionId,fresh:false}`. Before the first prompt it returns a fresh UUID with `fresh:true`; start it using `sessionId`, without resume. Missing/changed anchors fail instead of choosing a different cut.

Fork validation can extend the canonical SDK view only with a proven trailing
system suffix, such as copied hook summaries that the public message view hides
at its leaf. The read-only import cache must prove the exact cut, source mappings,
parent/order links and unchanged opaque payloads. This view is private to the
check; ordinary session pages do not gain guessed rows. Carlos source/input-kind
facts are saved atomically only after every prefix row and the final length pass.

Pages target 512 KiB. A larger record returns an empty array and
`chunk:{offset,totalBytes,data}` with base64 bytes, plus `chunkOffset` for the next
request. The client joins bytes until `chunkOffset:null`, parses the resulting
single record, and continues at `nextOffset`. It must keep the snapshot ID and
follow both offsets exactly. Each chunk carries at most 512 KiB before base64.
Records over 16 MiB fail explicitly; neither helper nor UI truncates them.

The helper uses official SDK readers plus a read-only temporary import/cache to
recover pre-compaction history. The SDK resolves current and earlier cached
branches, including kept segments/arbitrary kept UUIDs. The adapter reads the
pinned native compact-boundary logical parent to locate an earlier cache prefix.
It never locates, parses, rewrites, or copies native transcript/support files
itself. Forks and renames use SDK mutations only. Full cache/projection limits
and source-ledger details are in [README.md](README.md).

Rust adds helper generation IDs around its internal UI envelopes. They are not
part of this helper protocol. Rust rejects old-generation events and treats
Starting, Ready, and Closed separately. A late startup reply cannot reopen a
closed query; a replacement helper generation can start a new query.

The helper does not yet expose task-stop or background-task controls. Later
reviewed tasks can add methods in version 1. Callers should use `hello` to check
support. The public session contracts are documented in the
[TypeScript SDK reference](https://code.claude.com/docs/en/agent-sdk/typescript)
and [session guide](https://code.claude.com/docs/en/agent-sdk/sessions).

## Rust conversation and task state

The Rust reader stamps main frames and asynchronous replies with a conversation
number inside one helper generation. A validated native `conversation_reset`
opens one permission to adopt the next native `system/init.session_id`. The
reset's `new_conversation_id` is an opaque screen key, not that session ID.
Without that permission, a different init ID still fails. Main frames awaiting
the next init have an 8 MiB/1,024-frame cap; old-session main frames are dropped.
Context replies also carry the caller's context revision so a late answer cannot
restore counts from before compaction or a reset.

Actual callback IDs and process task events do not inherit a transcript lifetime.
A reset preserves pending questions, answer drafts, confirmation state, and the
native background live set. Only actual SDK cancellations withdraw callbacks.
The next native init adopts the real session identity without waiting for disk.
An in-memory accepted-input ledger retains at most 1,024 prompts/8 MiB of text
until completion. It restores surviving queued human prompts by UUID when native
Claude consumes them; it never resubmits them or guesses their author from text.
The clear command itself does not become an editable human prompt or Ralph result.

Native `background_tasks_changed` replaces the background live set. Carlos keeps
at most 4,096 live identities, 256 detailed task records/8 MiB, 4,096 recent run
IDs, and 4,096 outstanding stop states. Retained live/detail runs also guard
against older run IDs after recent-ID eviction. Stop errors retain at most 1,024
characters. Task edges alone cannot create phantom main turns or background
membership. Ambient and skipped-transcript watchers do not make Carlos look busy.
The child reader retains at most 128 active/recent stream cursors, evicting a
completed child first; the visible transcript remains intact. A genuine excess
of active child streams or live task identities stops the session explicitly.
