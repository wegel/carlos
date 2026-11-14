# Local daemon contract

EP019a implements this local contract on `iphone-spike`. The executable and
all three native factories pass sixteen deterministic process workflows, including
failure, pressure and shutdown checks. Final feature/release checks and measurements
pass; `.agents/reports/exec019a-final-report.md` records the evidence. The completed
headless step awaits owner review before EP019b.

The owner accepted EP019a on 2026-11-14. EP019b is active and adds exact original
source through `entry/source` reads/watches. Each chunk consumes at most 64 KiB,
preserves controls/UTF-8, and uses a format-specific entry/revision cursor. Both
formats share the existing watch count, reply-byte and socket limits. A large
plain row has an explicit source descriptor rather than inline body text; newest
and older pages shrink before cloning to stay within the encoded frame budget.
The terminal remains embedded while the rest of EP019b is being implemented.

## Clients and commands

The daemon serves newline-delimited JSON-RPC on a private Unix socket. It checks
that each peer belongs to the same OS user. It opens no TCP listener. A client
must finish `hello` before sending another command.

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

A live subscriber whose replay prefix has expired receives `session/snapshot`
with a complete bounded replacement page and its actual sequence. The server
never invents a reset sequence or silently skips a delta. An output watch that
loses queued chunks ends with `entry/output/reset`; the client explicitly reads
again or starts a fresh watch. A socket disconnect ends that socket's watches.

If one stored row or page cannot fit, the daemon returns an explicit error and
keeps the backend alive. It does not truncate source text. Smaller history pages
and bounded tool-output reads remain available. EP019b must resolve any remaining
oversized plain-text row limitation before terminal cutover.

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
- One daemon: 16 live peer connections, 16 active session owners and 256 saved
  session descriptions. Saved sessions do not start native processes until a
  client subscribes.
- Each peer: at most 16 session interests and 32 output watches. The daemon also
  bounds pending reads (256 across peers, with 32 MiB reserved for outstanding
  replies) and requests, rather than retaining arbitrary reply
  receivers for a disconnected peer.
- Each socket writer: 128 frames/sixteen MiB including the frame being written.
  Ordinary shared frames use at most eight MiB, leaving room for a replacement
  page or an explicit watch reset. Socket workers request 64 KiB kernel send/receive
  buffers; the OS may round those limits. A blocked write expires after 500 ms. If the
  writer cannot queue a required private reply or replacement, it closes that
  peer; the session and private results survive.
- The manager admits incoming frames through one FIFO with separate count/byte
  permits for ordinary commands, controls and reads. This preserves counter order
  without letting ordinary traffic consume all control admission.
- The retry store reserves independent room for ordinary and urgent operations:
  768 records/twelve MiB for ordinary work, 256 records/four MiB for controls.
  It reserves recovery space before dispatch. It evicts completed records first;
  a missing older counter never gains permission to execute again.
- The daemon retains at most 128 client identities, with a 24-hour inactive lease.
  A socket close does not cancel admitted work. The daemon may evict an inactive
  identity without recreating its old rights; pinned pending work still has a
  bounded record. Reads must not need an ordinary operation slot.

These byte budgets count encoded or admitted data. The final report measures
resident memory separately and accounts for decoded structures and native history.
Wire-byte limits do not describe an exact heap limit.

## Saved sessions and shutdown

The SQLite catalogue stores only the logical id, name, backend, working directory,
native thread id and saved Ralph state. It stores no transcript or job database.
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

Shutdown first cancels pending launches and signals every owner, then waits for
native cleanup. It stops only owned subprocesses and closes only its connection
to an attached native server. A stuck prompt-file read cannot hold shutdown
forever. Its worker has no session borrow, and retired permissions prevent a late
reply from reviving work.
