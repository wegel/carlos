# Daemon contract

EP019a established one shared session owner; EP019b moved the terminal onto
that owner and added exact original source reads and watches. EP020 adds explicit
session stop/reopen, local device controls and an optional WebSocket listener.
The archived plans and `.agents/reports/` retain their checked release evidence.

Each `entry/source` chunk consumes at most 64 KiB, preserves controls/UTF-8 and
uses an entry/revision cursor. Plain text and tool output share watch, reply-byte
and socket limits. Large rows carry a source descriptor rather than an inline
body; newest and older pages shrink to fit the encoded frame budget.

EP019b also promotes typed rename and earlier-prompt edit commands. Both use the
existing ordered lane and private retry identity. The owner keeps a fork/send
chain pending until its actual native acceptance. An asynchronous Claude fork
keeps a separate edit lifetime after its initial native write; reset/drop retires
it, and late helper switches/failures carry their originating operation. The
native writers check permission before bytes, including Codex's fork validation
and mutation calls. No arbitrary native method enters the public protocol.

## Clients and commands

`session/send` captures an explicit `steer` or `queue` policy with each message.
The phone starts each new draft with Steer and offers a Steer/Queue menu while a
response runs and the composer has text. Queue applies to that draft only; the
phone does not restore an older saved Queue preference. Its empty composer still
replaces Send with Stop.

Session settings offers Rename session through the existing `session/rename`
command. A separate name draft leaves the message composer untouched. The phone
changes the visible title only after daemon facts confirm it, keeps an unconfirmed
name and never automatically retries a rename whose outcome is unknown.

An old-message editor first sends `prompt/select` with `source: human`,
`direction: at_entry` and the visible transcript entry as `anchor`. The owner
returns the immutable prompt record bound to that exact row. Its record id can
differ from the transcript id. The client reads the original through bounded
`prompt/text` chunks, then uses that record id in `session/rewind` only when the
user chooses Fork and send. Selecting or cancelling an editor starts no work.
Retired entries, another source/session and replies for a different selected row
are rejected. An older daemon can reject the additive `at_entry` direction with
an ordinary request error; it must be updated before this phone feature is used.

The terminal accepts a running daemon or colocated companion built from different
source code when both declare the same wire protocol. A source hash identifies
a build; it does not prove that its messages are incompatible. Protocol mismatches
and inconsistent greetings always fail before session work. Set
`CARLOS_STRICT_DAEMON_BUILD=1` to require exact build identities for managed
startup. This optional check is off by default. An explicit native Codex socket
must still match the running daemon's endpoint.

The managed CLI reads `$XDG_CONFIG_HOME/carlos/daemon.json`, or
`~/.config/carlos/daemon.json` when XDG_CONFIG_HOME is unset, when it starts a new
daemon. The optional file saves the phone listener across future CLI starts:

```json
{"listen":"10.0.0.2:33732"}
```

Use an address assigned to the computer's LAN or VPN interface. The file must
be a regular file owned by the user, with mode0600, and no more than4096 bytes.
Only `listen` and `allow_origin` are accepted. The CLI passes its path to
`carlosd --config PATH`; a manually started daemon reads a file only when this
option is supplied. Explicit `--listen` and `--allow-origin` arguments override
their respective saved values. Invalid settings stop startup with an error;
Carlos never silently replaces them or changes a running daemon. A saved device
grant belongs to the daemon catalogue and remains valid after a restart. Client
retry rights still expire on that restart, as described below.

Remove the `listen` setting to keep future starts local. Restart a running
daemon only after its active turns finish. The CLI leaves a running daemon alone
when the file changes; it does not stop sessions to apply preferences.

The daemon serves newline-delimited JSON-RPC on a private Unix socket. It checks
that each peer belongs to the same OS user. It opens no TCP listener by default.
The operator may supply `--listen IP:PORT` for loopback or a WireGuard address;
wildcard and multicast addresses are rejected. Each WebSocket text message
carries one JSON-RPC envelope. A client must finish `hello` before another command.

Network clients supply a provisioned device token in the first `hello`. The
listener permits native clients without Origin; browser clients need the exact
configured `--allow-origin ORIGIN`. URLs, query strings and WebSocket subprotocols
never carry credentials. Compression is disabled. The HTTP upgrade accepts at
most eight KiB of headers; at most four unfinished upgrades/logins share a
five-second deadline. Initial messages are limited to four KiB. Authenticated
requests use the same four MiB limit as Unix requests. Malformed frames or a
missing, wrong or revoked token reveal no session data and start no native work.

Retry rights belong to either the local Unix user or one authenticated device.
A device needs both its access token and its private resume token to reconnect;
a different device cannot borrow those rights. Only Unix peers may add, list or
revoke devices. Revocation closes that device's sockets and forgets its retry
rights while native conversations and other devices keep running. See
`client-protocol-v1.md` for the operator commands and durable failure outcomes.

The greeting returns a fresh daemon instance id, a public client id, a private
256-bit resume token, and the next operation counter. A reconnect supplies the client id, daemon instance id and private token. Shared user rows may contain the public client id;
they never contain the secret. A retained id with a wrong secret fails. An
expired or unknown id gets a new client id with `resumed: false`. A restarted
daemon always has a new instance id and does not recreate old retry rights.

Each client increments a decimal operation counter. It must preserve all 64 bits
and must never wrap. `next_operation: null` means that this client used the last
counter; it may still read. Create, send, interrupt, approval answers and Ralph
changes all carry the same ClientOperation type. The daemon checks that its
client id matches the authenticated connection.

The manager admits counters in each connection's received order, before sending
urgent work to the owner's independent control lane. Processing a later
interrupt first must not make an earlier queued send look like an expired
retry. A second connection sharing an identity must coordinate its counters.

The manager retains a command's normalized typed bytes and its private result.
Another request id with the same operation and the same work returns the
pending or completed result. Different work with that counter fails. An older
counter missing from the retained window returns an expired/uncertain error and
never starts work. JSON-RPC ids pair envelopes with replies; they do not identify
native work. The manager saves results before notifying a socket, so a dropped
socket cannot consume the only recovery copy.

The owner keeps ordinary commands in arrival order. Interrupts and answers have
separate admission and workers. They still need the exact current turn or live
card. The owner claims a card once. It also admits only one in-flight interrupt for
a turn; duplicate stop intents cannot occupy every control worker. A worker validates offered choices and
structured answers before writing. A native write failure may be uncertain;
it does not allow another client to answer the card again.

The daemon never retries a possibly written command after a timeout, reconnect
or crash. The client keeps enough local command history to present an explicit
recovery choice if its server-side result expires.

## Session and output cursors

The owner numbers every shared session fact. Every subscriber receives that
single stream, including small output-change facts for collapsed tools. Expanded
tools use separate entry-bound output cursors. A client never advances its
session cursor by filtering out a bulk output notification.

For a subscribed session, the daemon queues the numbered facts through a body
chunk's sequence before sending that source/output reply or watch update. A
completed body read keeps its reserved bytes while those facts catch up. This
lets the client retire an older body version before a reply describes its
replacement. Direct reads without a session subscription need no such wait.

A live subscriber whose replay prefix has expired receives `session/snapshot`
with a complete bounded replacement page and its actual sequence. The server
never invents a reset sequence or silently skips a delta. An output watch that
loses queued chunks ends with `entry/output/reset`; the client explicitly reads
again or starts a fresh watch. A socket disconnect ends that socket's watches.

If one stored row or page cannot fit, the daemon returns an explicit error and
keeps the backend alive. It does not truncate source text. Smaller history pages
and bounded source/output reads remain available, including for plain text.

## Initial budgets

The native adapters already use bounded frames and event queues. The daemon's
initial budgets below reserve room for urgent work and recovery. Process tests
must exercise them before the listener is accepted.

- One request frame: four MiB. One response or notification frame: eight MiB.
  A send contains at most 512 KiB of source text. Structured answer limits also
  apply before native dispatch.
- One owner: 64 ordinary requests/eight MiB, 16 controls/two MiB and 64 reads/one
  MiB. The same permit covers queued and executing work and uncollected results.
- One owner: one ordinary worker, four control workers and two startup/read
  workers. Native queues keep their own frame, byte and write-deadline limits.
- One owner: 1024 encoded event frames/sixteen MiB. Encoding happens once; socket
  writers share those immutable bytes. A missing prefix requires a snapshot.
- One daemon: 16 local peers plus 16 network peers, 16 active session owners and 256 saved
  session descriptions. Saved sessions do not start native processes until a
  client subscribes, unless explicitly stopped.
- Each peer: at most 16 session interests and 32 output watches. The daemon also
  bounds pending reads (256 across peers, with 32 MiB reserved for outstanding
  replies) and requests, rather than retaining arbitrary reply
  receivers for a disconnected peer.
- Each socket writer: 128 frames/sixteen MiB including the frame being written.
  Ordinary shared facts/output/source use at most 96 frames and eight MiB, leaving 32 frame slots and eight MiB for replies and a replacement
  page or an explicit watch reset. Socket workers request 64 KiB kernel send/receive
  buffers; the OS may round those limits. A Unix write expires after 500 ms. A WebSocket write expires after 500 ms
  without successful TCP write progress; a healthy throttled reader may take
  longer to receive a large frame, which stays charged until the actual flush. If the
  writer cannot queue a required private reply or replacement, it closes that
  peer; the session and private results survive.
- The manager admits incoming frames through one FIFO with separate count/byte
  permits for ordinary commands, controls and reads. This preserves counter order
  without letting ordinary traffic consume all control admission. Local and
  network peers have separate allowances, preserving local operator access.
- The retry store reserves independent room for ordinary and urgent operations:
  768 records/twelve MiB for ordinary work, 256 records/four MiB for controls,
  and 16 records/128 KiB for device controls.
  It reserves recovery space before dispatch. It evicts completed records first;
  a missing older counter never gains permission to execute again.
- The daemon retains at most 128 local plus 128 network client identities, with
  a 24-hour inactive lease.
  A socket close does not cancel admitted work. The daemon may evict an inactive
  identity without recreating its old rights; pinned pending work still has a
  bounded record. Reads must not need an ordinary operation slot.

These byte budgets count encoded or admitted data. The final report measures
resident memory separately and accounts for decoded structures and native history.
Wire-byte limits do not describe an exact heap limit. All network devices share
the network ingress allowance and the 128-entry network identity pool. A flooding
device can cause another network socket to close or an inactive lease to expire;
it cannot borrow that lease's private results or authorize replay. Local ingress
and identity allowances remain separate. Devices share the operator's session
controls; the daemon does not promise per-device scheduling or reserved retry slots.

## Saved sessions and shutdown

The SQLite catalogue stores only the logical id, name, backend, working directory,
native thread id, saved Ralph state, stopped flag and a Claude fresh-target flag.
It also stores at most 32 device ids, bounded names and BLAKE3 secret verifiers;
it never stores the device secrets. It stores no transcript or job database.
The name is the Carlos catalogue label and survives native history adoption.
Create stays pending until native startup finishes and the native address has
been saved. One worker owns disk writes. It saves changed durable fields, not every token or
context-window update. Two subscribers share one restore attempt. A failed
restore stays visible and never becomes a guessed new native session.

A subscription can first return explicit starting/restoring facts so startup
cards remain answerable while native history is being read. The owner then
publishes a numbered replacement after it adopts history. A client must keep
reading those shared facts before claiming the session is ready. A fresh or
restarted client always gets a snapshot; only a current-instance authenticated
reconnect may request replay from a saved cursor. Native waits and file reads run
outside both session writes and the manager. Captured handles refer to one
helper and conversation; queued frames check that permission again at the native
writer. Worker replies also check their original conversation before changing
facts. Ralph stays off after a crash, and the owner displays its saved-state
notice after history is adopted so a reset cannot erase it.

An explicit session stop joins its native owner before reporting success and
persists the stopped flag on the disk worker. Reconnect/subscribe never reopens
it. Explicit reopen restores the same saved native conversation, preserving
private results and advancing the shared cursor across the replaced owner. A
failed durable save reports uncertainty rather than claiming durable success.

Shutdown first cancels pending launches and signals every owner, then waits for
native cleanup. It stops only owned subprocesses and closes only its connection
to an attached native server. A stuck prompt-file read cannot hold shutdown
forever. Its worker has no session borrow, and retired permissions prevent a late
reply from reviving work.
