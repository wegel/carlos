# Claude SDK controller

Carlos's Rust Claude backend uses this helper to run the official Claude Agent
SDK, Anthropic's library for controlling the Claude CLI. The helper accepts JSON
objects on stdin and writes JSON objects on stdout, one object per line.
[PROTOCOL.md](PROTOCOL.md) describes each message. Rust preserves those frames
and translates native messages where the terminal UI needs them.

The caller must provide the absolute path to its installed `claude` executable.
The helper never installs Claude, extracts credentials, or calls the Anthropic
Messages API itself. The native CLI reads its usual account, settings, and plugins.
The caller chooses which native settings files the CLI may read. Carlos should
normally send `["user", "project", "local"]`.

Build and check from the repository root:

```sh
BUN_BINARY=/home/wegel/.bun/bin/bun bash scripts/build-claude-controller.sh
```

The script fetches packages with the checked-in lockfile, checks TypeScript, runs
tests, and compiles `target/claude-controller/carlos-claude-controller`. It then runs
the same tests against that executable from temporary folders with only system
tools in `PATH`. Bun embeds its runtime and the SDK in the executable. Users need
the helper and their native Claude executable at launch; they do not need Node,
Bun, `node_modules`, or a package download. The build machine needs Bun and Python
3, which the fake CLI tests use. The script does not install the Carlos binary.

The package pins SDK `0.3.322`; `bun.lock` pins every resolved package. The build
omits SDK peer packages and optional native Claude packages. SDK `sdk.mjs` already
contains the runtime code this helper uses. The lockfile still records the SDK's
optional package metadata; the build does not fetch or bundle those executables.
The Linux x64 helper is about 100 MiB because it embeds Bun. macOS helpers are
about 59–64 MiB and Windows about 110 MiB, before archive compression.

The repository test runner builds this helper explicitly before terminal tests:

```sh
BUN_BINARY=/home/wegel/.bun/bin/bun bash scripts/test.sh --codex-contract
```

CI installs tested Bun `1.3.10` with the official
[setup-bun action](https://github.com/oven-sh/setup-bun/blob/main/README.md).
Neither Carlos nor the helper builds or downloads anything at app launch.

Each source file has one job:

- `protocol.ts` checks requests before the SDK sees them.
- `controller.ts` owns one SDK session and correlates request IDs with replies.
- `sdk.ts` starts the selected CLI and registers native permission and question callbacks.
- `callbacks.ts` keeps each unanswered question by its own SDK request ID.
- `form-answer.ts` checks server forms, native question answers, and typed values.
- `answer-process.ts` and `answer-worker.ts` isolate server patterns in a disposable copy of the helper.
- `formats.ts` uses pinned `validator` and `ajv-formats` checks for mailboxes, URIs, and calendar timestamps.
- `input-queue.ts` keeps the SDK input stream open between prompts.
- `output-writer.ts` caps queued bytes and waits for the host to read each frame.
- `stdout-stream.ts` writes to the actual host pipe, bypassing Bun's extra stdout buffer.
- `native-output.ts` caps native frames and unread SDK events through the SDK's supported process hook.
- `sessions.ts` calls the published session APIs and returns stable, bounded history pages.
- `session-archive.ts` reads full imported history through the SDK, including older compacted branches.
- `session-fork.ts` checks the exact cut and new UUIDs before Carlos switches sessions.
- `prompt-sources.ts` stores Carlos prompt authorship in its own SQLite file.
- `optional-usage.ts` isolates the optional account quota read and its deadline.
- `quota-windows.ts` converts its percentage/reset fields without mixing model buckets.
- `recent-ids.ts` checks active request IDs and keeps the most recent 1,024 completed IDs.
- `main.ts` connects the controller to pipes and closes it on EOF or process signals.

Tests run the real official SDK against `tests/fake-cli.py`. The fake CLI never
calls a model. Tests cover startup, live controls, prompt identity, concurrent
questions, cancelled questions, child events after the main result, bad input,
and shutdown while the SDK waits for input or a reply. A Python host genuinely
stops reading the OS pipe to check blocked output, callback floods, normal drain,
and bounded shutdown. Every test uses temporary Claude and home folders. Tests
never open the user's Claude sessions.

The caller must declare `supportedDialogKinds` only after its UI can display and
answer those kinds. The helper defaults to declaring none; it does not claim
generic dialog support. See the protocol's output limits and failure behavior
before connecting a host.

Carlos looks for `CARLOS_CLAUDE_CONTROLLER` first, then a helper beside the Carlos
binary, then `../libexec/carlos/`, then `target/claude-controller/` when Carlos runs
from its development `debug` or `release` folder. Windows uses the `.exe` suffix.
`CARLOS_CLAUDE_EXECUTABLE` can select the native CLI explicitly; otherwise Carlos
searches `PATH`, skipping files it cannot execute. The helper accepts native
absolute paths, including Windows drive and UNC paths.

Rust starts the SDK session without waiting for startup hooks. The user can
answer native permission requests before the SDK returns its startup answer.
Rust then reads the native model list and context counts, validates saved Carlos
choices, and applies accepted choices through live SDK controls. A saved Codex or
Grok model is discarded when native Claude metadata does not recognize it.
Native model observations stay separate from Carlos overrides, so an appearance
save does not pin a native model mode or effort setting.

Every submitted prompt has a UUID, author, and native priority. Actual keyboard
or dictation input is human; Ralph prompts and follow-ups remain automatic.
Rust joins results to their consumed prompt UUIDs and waits for native idle,
regardless of result/idle order. It captures interrupt intent before dispatch,
rolls it back on a failed control, and recognizes native `aborted_streaming` and
`aborted_tools` results. Failed turns stop automatic Ralph follow-ups.

Rust keeps at most 1,024 queued event frames and charges at most 8 MiB until the
UI handles them, including frames moved through intermediate queues. One writer
thread sends stdin frames of at most 4 MiB through an eight-frame queue. The
caller waits at most 500 ms for a write and closes a session whose acceptance is
uncertain. Shutdown gives the helper up to three seconds, including its write.
`CARLOS_CLAUDE_WIRE_TRACE` optionally records controller frames to a caller-chosen
local file; terminal tests use it alongside the fake CLI's own wire log.

Carlos now shows native tool approvals, AskUserQuestion choices and custom text,
plans, typed MCP forms, and browser consent requests. Each request keeps its own
editor and actual native ID. A cancelled request cannot erase another child's
draft. Main results do not dismiss unanswered child requests. Carlos preserves
the main message composer while a question is open and retains question drafts
when a reply fails. It escapes controls in native labels before drawing them.

- Use arrows and Space to select choices, or choose Other and type custom text. Tab moves through fields and a review page; Ctrl+S sends the answers.
- Alt+N declines and Alt+C cancels. Plain tool/plan/browser prompts also show y/n/c. A native default-to-no tool request requires two explicit approval keys.
- F4 switches queued requests, F10 interrupts Claude, and Ctrl+M opens live settings. Page Up/Down scroll metadata and long plans. F8 keeps its existing performance control.
- Browser requests show the requesting server, actual URL host, and link. Ctrl+Y copies the link. Accept means consent, not proof that the browser finished. Carlos matches early or later completion notices by server and ID without answering for the user.
- At 36 by 12 cells Carlos keeps a separate answer caret and visible controls. Smaller terminals show a resize message with decline/cancel/interrupt keys and preserve drafts.

The helper checks flat MCP schemas, the server's rules for answer types and
limits, including titled enums and enum arrays. Unsupported forms remain
decline/cancel-able. It checks server patterns in a disposable helper process,
with at most four checkers and a one-second work limit. No deadline applies while
a person thinks. Unsafe integer answers fail visibly instead of being rounded;
ordinary finite number fields retain the SDK's number behavior.

Carlos offers per-call allow/deny only. It does not offer persistent permission
rules or declare generic dialog kinds. Release archives pair both executables, notices, and a relinkable JavaScript
bundle on each shipped platform.

Carlos lists, resumes, reads, renames, and forks sessions through published SDK
functions. The picker includes native CLI and programmatic SDK sessions, uses
native config-folder/worktree rules, and pages the catalog. Continue selects a
specific SDK catalog session before Carlos loads it, then resumes that same ID.
New sessions choose their UUID before native startup. Startup hooks stay
answerable while history loads and the native query starts.

`/rename` saves a Unicode title through `renameSession`, without a model call or
extra prompt. Rewind forks before the selected prompt, keeping the original
session intact. Editing the first prompt starts an empty session. Carlos waits
up to five seconds for an exact live prompt UUID to reach native storage; it
never substitutes a stale anchor. Carlos checks the returned fork contents and
SDK-provided source UUIDs before switching its UI. Each replacement helper gets
a new generation number, so old cancellation/close/metadata frames cannot alter
its session. Failed startup leaves the edited prompt available as a draft. Carlos also keeps
unsent queued human drafts after closure: F5 loads one when the composer is empty
and leaves a newer composer draft untouched. Automatic work is discarded.

The SDK can copy a trailing hook-summary record while omitting it from the
normal message view. Fork checks therefore keep the SDK's canonical conversation
as their baseline and prove any hidden system suffix through the published import
store: exact source UUIDs, parent links, record order and opaque payloads must
match. Attachments can connect those links but never become hidden user/assistant
messages. Missing, altered or extra conversation rows still reject the branch.
Only a completely validated fork publishes Carlos source/label facts, in one
SQLite transaction. Normal history display continues to follow the SDK view.

The public SDK message reader returns only the active context after compaction.
The helper therefore uses the published `importSessionToStore` reader to fill a
private temporary SQLite cache, then asks `getSessionMessages` to read that cache.
The SDK chooses active branches and applies kept-message links. A small read-only
adapter uses the pinned native `logicalParentUuid` field to expand earlier
compacted history. It preserves every returned/opaque field, including child
records and unknown metadata, without changing Claude files. The importer/store
APIs are marked alpha; SDK and native-format changes require the fixture checks
before an upgrade. Every native mutation still belongs to the official SDK.

The helper targets 512 KiB pages and sends large individual records in bounded
chunks. It caps one record at 16 MiB, one imported cache at 256 MiB, cumulative
SDK cache reads at 512 MiB, visible rows at 100,000, and nested compactions at 256.
Two snapshots and at most two active reads are allowed. The SDK itself can
materialize a complete session while it reads; paging bounds the local wire and
Rust backlog, not that SDK allocation. Exceeding a limit fails visibly.

The native CLI sometimes drops the automatic origin on disk. Before sending a
prompt, Carlos saves its UUID/source in its own SQLite file below
`$XDG_STATE_HOME/carlos/` (or `~/.local/state/carlos/`), separated by native config
folder. A separate table stores the captured human prompt/steer display choice
for each UUID, carrying it through validated SDK fork UUID changes. Old inputs
without that choice stay plain; priority alone does not prove a steer. Carlos
saves no prompt text or login credentials there. Human keyboard and
dictation prompts remain editable after resume/fork, including consumed queued
prompts. Automatic, child, local-command, and compact-summary records do not
enter ordinary prompt history. Older main-thread records with unknown authors
stay labeled unknown; while rewind is open, F4 explicitly selects them for a new
human-authored branch. This choice does not relabel their original source.

The native answer contracts come from the official
[user-input guide](https://code.claude.com/docs/en/agent-sdk/user-input), pinned
SDK declarations, and [MCP form/browser specification](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation).

Carlos shows native thinking in its reasoning rows, including stream/snapshot
pairs without repeated text. A redacted-thinking estimate appears as progress,
never as hidden text or a billed token total. Native requesting, compacting,
running, waiting-for-user, and idle states stay separate. Context counts come
only from asynchronous native summary reads; stale replies cannot undo a newer
compaction or conversation reset. Native warnings, failed plugins/hooks, and
lost session writes remain visible without closing a healthy query.

F2 opens the task/quota panel. Arrows select a native task, Page Up/Down scrolls
its details, and x asks native Claude to stop a known running task. The panel
shows stop requested until a native event reports the outcome. It pins the
selected task above queued prompts and quota rows on a phone. Closing it restores
the composer or question caret and draft. Ambient watchers do not animate the
main status, and a replaced native background set cannot leave phantom work.
Child text, thinking, tools, and results stay out of the main conversation,
including loaded history. F2 retains native task details and stop controls. Child
questions remain answerable, and the main assistant can still report its findings.

F3 changes Enter between steering now and queuing behind current work. The status
bar shows the choice. Both choices keep real human origin; Ralph remains
automatic. Carlos never resends a native-accepted prompt. An interrupt can leave
native `still_queued` UUIDs waiting, and Carlos says so instead of claiming they
were cancelled.

Stable native rate-limit events report fractions from zero to one. Carlos keeps
the five-hour, global weekly, and model-specific buckets separate. The optional
experimental quota read reports percentages from zero to one hundred; the helper
normalizes its reset times and preserves the native report. It skips transcript
behavior scans, runs asynchronously, and cannot block startup or a message.
Unavailable, missing, or expired quota values remain absent. Only the shared
weekly bucket drives Carlos's existing weekly pace indicator.

A native `/clear` or other conversation reset clears the correct transcript once.
The advertised reset key names the screen; the next native init supplies the
actual session ID. Carlos keeps callback IDs, queued human UUIDs, drafts, chosen
model/effort, and surviving process tasks intact. It rejects a changed session ID
without an observed reset. A new helper generation still clears process state
and rejects every old helper frame.

The build script accepts --target with one of the four Rust release triples and
--output for the matching helper path. It pins Bun 1.3.10, records both target
names in claude-controller-build.json, emits Bun's input-module manifest, and
builds the optional relink/controller.js without a native runtime. The package
script checks each binary's platform/CPU header and takes JavaScript notices from
that actual module manifest. Rust notices come from the target's resolved Cargo
build inputs. [RELINK.md](RELINK.md) pins the Bun/WebKit sources and gives the
supported compiler command for a rebuilt runtime.

Every compiled build explicitly disables dotenv, bunfig, tsconfig, and
package.json autoloading. The package smoke test puts poisoned environment files,
a Bun preload, and an unavailable dependency in the working folder. It checks
that they stay inert, explicit process settings still select the expected SDK
session, and the checker can spawn itself with Node/Bun absent from PATH.
Windows runs the worker directly in that check; native SDK startup/control smoke
runs separately on every matching CI host. Linux terminal contracts exercise the
whole packaged Rust/helper/native path against localhost, with no account calls.
