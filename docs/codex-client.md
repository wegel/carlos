# Native Codex client

Carlos launches the installed `codex app-server`. The native CLI reads its normal
login, environment and settings. Carlos does not bundle Codex or require an API
key. The client sends `initialize`, waits for success, then sends `initialized`
before any other request. Carlos opts into useful experimental methods.

The checked-in schema bundles come from `codex-cli 0.167.1`. Older per-method
schema files can remain for compatibility; the bundles identify current methods.
See [the schema notes](app-server-schema/README.md) and
[the official app-server guide](https://learn.chatgpt.com/docs/app-server.md).

Carlos resumes and forks with `excludeTurns: true`. It reads ascending full-turn
pages of up to100 turns, including each turn's items. This avoids a separate
native lookup for every tool or message. Servers that return unloaded items
instead still use the older per-turn item reader. Carlos keeps native IDs and
unknown fields, checks cursors and item arrays, and replaces the visible
transcript only after every page succeeds. The exact-item rewind guard still
rejects a steer before any fork.

A history read keeps one overall20-second deadline, at most65,536 pages, and a
64 MiB cap on fetched page JSON and assembled history. One blocking history
reply can use up to64 MiB outside the UI event stream. Only exact pending IDs
for public item pages, full-turn pages and the older full-history read get that
allowance. Ordinary metadata, asynchronous replies, events and writes retain
8 MiB limits. Carlos never silently installs a truncated snapshot. An older
server without turn pages can supply the older full-history read.

Some legacy native sessions expose turn IDs but omit their items. Only that older
path can use Carlos's read-only lifecycle reader. It merges exact turn/item IDs,
keeps server metadata and versions, and refuses contradictory shared order.
Inherited sessions and history-removal markers disable that recovery; modern
paged sessions remain authoritative. A matching eligible file that cannot be read
completely produces an error.

The transport workers bound stdin to eight queued frames and 16 MiB, pending calls to
64, and unhandled UI events to 4,096 frames and 16 MiB. Events stay charged while
Carlos forwards them through its UI queues. One reserved terminal notice reports
failure even when the normal event budget is full. A blocked write, broken pipe,
malformed frame, EOF or exceeded event budget closes the local connection and settles
waiting calls. A request deadline includes its pipe write. Callback writes
have a separate two-second bound; users have no deadline while
thinking about a question. Async controls enqueue bounded work without waiting
for the stdin pipe; a separate worker enforces their complete 15-second deadline.

Only an explicit list of read methods may time out without stopping healthy
native work. Carlos discards their late replies. Carlos permits one active large
history read and retains at most64 timed-out history IDs until their replies
drain. If that drain limit fills, another large history read fails clearly;
ordinary controls remain usable. A locally rejected write releases its ID.
Reconnect failures keep a short visible diagnostic even when the native error
contains a large body. A dispatched mutation whose
reply times out is uncertain, so Carlos closes the connection instead of retrying
it. Owned mode also stops its app-server; socket mode never stops the shared server. Async replies keep the method, native response and caller context. Ordinary
controls also retain their exact request parameters;
control parameters normally have a 64 KiB cap. Queue add/update accept prompt-sized
input within the 8 MiB wire limit. Their reply context keeps IDs instead of
duplicating the whole input; the full native reply remains intact. Pending local
control context has a separate 16 MiB bound. Native callback IDs retain their original
string or number type. Unknown additive native messages remain available to the
UI. Later refresh stages add richer native controls without inventing user input.

Codex questions, approvals and MCP forms keep their original request IDs, including
the difference between a numeric ID and a string ID. Each request keeps its own
draft. F4 opens a waiting question or moves to the next request. A nonblocking
question leaves the composer usable; a blocking request opens its editor. Root
turn completion does not erase a child's unanswered request. A thread switch
retires old requests and their late replies within the bounded replay window.

Question answers use Codex's question IDs and answer arrays. Ctrl+S sends an
explicit answer, Alt+N skips, and Alt+C interrupts that question's own thread and
turn. Esc returns to the composer only for a nonblocking question. Secret fields
stay masked in the editor, review page and optional input trace. Managed human
checkpoints offer one-time approval only.

Standard MCP forms keep numbers, booleans, enum values and omitted optional
fields as typed values. A Rust worker checks the complete schema and answer,
with no file/network schema retrieval and bounded linear regular expressions.
Codex forms work without the Claude helper. Unsupported schemas and extended
verification modes remain visible with decline/cancel controls. URL requests
show the actual host and link and require explicit consent; Carlos neither opens
the browser nor treats consent as proof that the server finished authentication.
The current Codex protocol exposes no separate browser-complete notice.

Carlos keeps at most 64 pending requests, 128 KiB per request including its ID,
and 4 MiB total request payloads. It remembers the last 2,048 retired IDs to
suppress duplicate delivery. These limits do not impose a deadline on the user.

F3 selects Steer or Queue for Enter. `/queue` or Alt+Q opens the queue without
sending a model prompt. Arrows select an exact native queued ID; e edits, d
deletes, s starts that entry when idle, and Alt+Up/Down reorders. Ctrl+S saves an
edit; Esc closes the editor or panel. The main composer keeps its separate draft.
A failed send returns its text to an empty composer or keeps it for explicit
recovery with v. Carlos blocks a second submission while that recovery waits.
Carlos also waits for pending queue controls/recovery before forking history.

Codex stores the queue and starts entries automatically after normal completion.
Carlos does not resubmit them at turn completion. Interrupting a turn leaves its
queue intact; s sends the explicit native start request. Acknowledgement confirms
that request only. Native consumed user items supply transcript rows, client IDs
and rewind anchors. Ralph waits for a current, empty native queue before starting
an automatic continuation; Carlos adds no separate automatic driver.

Carlos reads one queued record per page and shows a new list only when all pages
succeed, with limits of 4,096 entries, 16 MiB and 20 seconds. Changes from another
client invalidate an unfinished read. The selected ID stays pinned if that entry
disappears; arrows must select a replacement. Text previews stop after 1,024
characters and say when they do; Carlos retains complete native inputs.
Carlos edits only one plain text segment with no spans. Attachments, multiple
segments and unknown input fields remain intact and usable for start/delete/
reorder. An observed external edit makes an old draft unsavable. Codex has no
"save only if unchanged" field, so simultaneous edits still follow the server's
order; Carlos does not claim an atomic conflict guarantee or write private stores.

The settings panel publishes choices for future turns and later steps of a running
turn separately. Carlos shows pending, acknowledged and failed outcomes. Already
running model requests retain the choices they captured. Normal prompts inherit
native thread settings, including newer choices from another client. Carlos
publishes its saved defaults for new threads through the settings API. Resumed
threads keep their native model and effort. If Codex does not report a summary,
Carlos shows “Use thread setting” and omits that field until the user chooses one.
An explicit `CARLOS_REASONING_SUMMARY` overrides the summary for that launch. Only a
definitively missing settings method enables the older direct next-turn override
path. Rejected values remain in the settings editor for correction, while valid
runtime choices and saved Carlos defaults stay intact. Native validation/policy
errors keep their code and data; they never trigger that compatibility fallback.

Before starting an owned server, Carlos reads `codex features list` with a
two-second/64 KiB bound. When Codex advertises `step_model_switching`, Carlos
enables it for that process so explicit settings changes can reach later model
steps. The native CLI still enforces its managed policy. Carlos does not rewrite
native config; an older CLI starts without an unknown feature flag. A server
without the capability reports a truthful future-only or partial-failure result.
Persisted queues can start during native resume before Carlos's later settings
request is acknowledged; those requests use the native choices already in force.

Carlos preserves Codex server-frame order through its UI budget, so an assistant's
completion marker arrives before turn completion and Ralph's next decision.
Keyboard/mouse events stay responsive; only the terminal closed diagnostic can
bypass earlier Codex frames. Claude and Grok keep their existing event paths.


## Agents and background processes

F2 reads spawned descendants through `thread/list` with an ancestor and explicit
subagent sources. Carlos follows positive parent/source metadata through read-only
`thread/read` calls before trusting a relationship, including deeper descendants.
An older server that ignores the ancestor filter cannot make unrelated chats
eligible for a stop. Carlos never resumes a child merely to inspect it.

The panel retains a recent window of at most 256 agents and offers l for older
windows, r for latest. It reads background processes for the root and loaded
verified descendants, at most 512 records. Complete reads have an 8 MiB/20-second
budget; every page uses the remaining original deadline. Child details read public
item pages in batches of 20, retaining at most 128 items/8 MiB, with explicit older,
latest and display limits. Only the selected child gets a 16 KiB live output tail.
Native child report bodies stay in a separate 1 MiB cache, never the root transcript.
Empty native item pages do not prove no work happened: native 0.167.1 does not
retain a display item for every tool, including an interrupted clock-only wait.
Carlos does not synthesize those private records. Null OS/CPU/RSS metrics stay
unavailable. Idle is distinct from native task completion.

x first reads the selected agent's current turn, then sends `turn/interrupt` with
its exact thread and turn IDs. A process stop uses only
`thread/backgroundTerminals/terminate` with its exact owner/process IDs. Carlos
refreshes after stop replies; a vanished selection never redirects x to another
row. A slow exact-target stop reports that it is unconfirmed, keeps unrelated
work alive, discards its late reply and reconciles with reads. Carlos never retries
the mutation automatically. Pipe failures and uncertain unknown/history mutations
still close the owned server. Refresh, panel close and Carlos exit never call
`thread/backgroundTerminals/clean`, which would kill running processes.


## Explicit native goals

`/goal` or Alt+G opens root-goal facts and controls. Carlos sends only
`thread/goal/get` during startup/thread switches, never goal/set/clear or an
extra chat prompt. Every direct user mutation includes `origin: user`, which
native Codex records as explicit authorization. The owned CLI feature probe
adds a goals flag only when the installed CLI advertises it as default-off;
installed 0.167.1 reports goals stable true. Native feature/policy errors remain
visible, and no global config is rewritten.

New drafts are visibly paused. Active save/resume needs explicit confirmation
and refuses while Ralph or a pending Ralph toggle is on. Native goal execution
can start immediately after an active API change; Carlos does not schedule an
extra turn. Existing/restored driving goals stop Carlos's automatic loop while
retaining queued humans. Unknown facts, pending mutations and uncertain replies
block Ralph until a fresh native snapshot establishes a safe state. A malformed
or old response cannot restore a stale paused snapshot. Read failures keep the
healthy connection; uncertain mutations retain the existing fatal timeout rule.

Blank token budget omits tokenBudget, which keeps an existing native budget.
It never means remove. A new blank budget leaves the choice to native config.
Objective/budget saves send the latest observed native status explicitly; they
never rely on native's active default when the goal may have been cleared
concurrently. Limited/blocked/complete statuses remain unchanged by such edits.
Goal set has no conditional version field: unobserved simultaneous external
changes follow native last-writer order. Carlos detects newer observed facts,
keeps drafts, and never claims atomic conflict prevention or writes private files.
Counters come from native responses. Explicit clear followed by explicit create
starts a new goal; Carlos never invents a reset or a budget-removal sentinel.

The independent draft stays locked during a pending mutation and survives
failures, overlay changes and returning to the overview. Esc backs out of the
editor while keeping the draft, then closes the panel. Status-only pause/resume
never saves an unsent objective. F2 and F4 keep tasks and questions usable; forms
and plain approvals own paste/mouse focus above a hidden goal editor. Idle waits
include the goal-read timer and F2's refresh/read deadlines without busy polling
closed, older-window or static child-detail views.


## Local socket ownership and reconnect

`--codex-socket PATH` opts into an existing native Unix endpoint. Carlos connects
with the official client's local WebSocket handshake at `ws://localhost/rpc` over
that Unix socket. The URI does not open TCP. Carlos neither discovers a daemon nor
starts, installs, stops or reconfigures one. Socket mode needs Unix; default owned
stdio still works on the other release platforms. Native Linux 0.167.1 fixtures
verify this path; macOS socket behavior has not been exercised here.

The native `app-server proxy` command only relays bytes. It does not turn the
Unix endpoint into JSON lines, so Carlos uses a direct local socket and the
maintained Tungstenite codec. Mio waits for socket readiness or an outgoing-write
wake rather than adding a polling delay to every history page. Connect and HTTP
upgrade each have a two-second bound. The codec bounds one physical frame or assembled fragmented message at64 MiB.
The reader accepts content above8 MiB only for an exact eligible history reply;
ordinary messages keep their8 MiB limit. It handles ping/close frames without
another reader owning its state. Writes keep the existing frame/byte deadlines.
Closing Carlos shuts only this local descriptor. A lost shared connection never
marks a native turn failed; shared work may still be running.

F5 explicitly reconnects the current UI thread, including after a fork. Owned
mode reuses the feature flags checked at launch rather than starting another
feature probe during reconnect. One worker does handshake, exact resume and bounded public reads; it never republishes
old Carlos defaults or resends uncertain chat, queue/settings/goal mutations or
Ralph work. Initial resume/Continue uses the same asynchronous history path.
The composer and independent drafts remain usable, questions remain answerable,
and sends wait until a complete snapshot plus buffered native events are applied.
A failed read keeps the previous transcript and recoverable drafts. Repeated
failed attempts retain the warning about unanswered child requests until a
successful reattach shows it. A fresh connection clears retired callback IDs and removes old connection-local overrides.

Every frame/control/form result carries its connection generation. All generations
share one charged UI stream, including envelope bytes and the reserved terminal
notice (at most 64 KiB beyond the normal budget). An older frame cannot change a
newer connection, even when native numeric request IDs repeat. Carlos keeps at
most one reconnect worker and one pending off-stream snapshot:64 MiB history,
16 MiB queue, 8 MiB panel, 8 MiB catalog/runtime and 128 KiB goal. Its small ordered
ready ticket does not serialize that history into the16 MiB UI stream. Buffered
replay stays charged, with an additional 12 MiB/2,048-frame limit.

The server exposes no shared revision between a captured item read and its
streamed deltas. Carlos therefore never guesses a text overlap. Existing partial
items show native snapshots and get bounded asynchronous exact-item reads until
complete events arrive; new items stream normally. A completed command that omits
its body gets a distinct final read for at most five seconds. Older partial reads
cannot replace completion. If native cannot supply complete output, Carlos keeps
the last snapshot and shows that limit. The partial-item cache is bounded to 64 MiB
and 500,000 exact turn/item identities. Complete native item/turn events remain
usable regardless of when an older read reply arrives. Fresh model/effort/name
metadata and separate revision checks keep older buffered observations from
replacing later settings or renames; an unavailable summary stays unknown.

Native root resume replays ordinary pending requests for that root only. A real
child MCP form survives in the shared server but does not replay through root
resume. Carlos warns when it lost such a child request; F2 can inspect/stop that
child, or another client already attached to it can answer. Carlos does not
resume a child to manufacture a subscription. Exclusive verification-owner
requests do not replay. Carlos applies no deadline to an ordinary unanswered
question; read/connection failures remain independent transport failures.

Display keys include Codex's native turn ID, item ID and separate display-part
index. Reused item IDs across turns or IDs containing a printable part suffix
cannot collapse history. Raw native IDs still go into rewind checks and protocol
replies; Claude and Grok retain their previous keys.
