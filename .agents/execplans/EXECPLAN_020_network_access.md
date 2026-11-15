# EP020: release stopped sessions and serve authenticated network clients

This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. Work directly in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`. EP019b is approved and archived; do not edit it. The owner delegated approval and continued work through the phone spike. Obtain the final direct engineering review and Fable 5.2 medium whole-roadmap review after the whole EP is ready, address the findings, record delegated approval and continue to EP021. Do not install a spike executable, merge main or push.

## Purpose / Big Picture


A user must be able to stop a conversation's native processes without deleting the saved conversation or restarting the entire daemon. Another terminal must keep its own drafts when that happens, and an explicit reopen must restore the same native conversation. This comes first because EP019b's final review found that the daemon otherwise keeps all 16 owner slots until shutdown.

After that repair, a phone or other network client can connect to an explicitly configured WebSocket address and authenticate with its own device token. A WebSocket is one ordered two-way stream that phones and browsers already support. Each text message carries the same typed JSON-RPC request or reply used on the local Unix socket. Both transports must use the same session manager, numbered facts, private operation results and queue bounds. The local terminal remains a Unix-socket client; this EP does not build a phone screen or add speculative phone commands.

The daemon can run shell commands as its user. Network access stays off unless the operator supplies a specific listen address. Plain `ws` is intended for loopback or the configured WireGuard interface, which encrypts traffic between the user's devices. Do not configure a real external interface, firewall or device on the owner's behalf during this task. All acceptance checks use private directories and loopback ephemeral ports.

## Progress

- [x] (2026-11-15) Retire active and queued approval cards after final worker cleanup, even if the owner stops before reducing the native close message. The focused Core shutdown cases pass. One initial test incorrectly assumed cursor zero forced a snapshot; request a deliberately unmatched cursor to verify the final full snapshot instead. Public lifecycle routes remain incomplete.

- [x] (2026-11-15) Add explicit Core stop_session with EngineReply::Stopped carrying final updates and a separate native interrupt result. Preserve ordinary shutdown behavior. Five captured-writer cases pass for exact attached turn/bystander, lost acknowledgement, normal shutdown, confirmed idle read, and native activity without a safe turn ID. A deterministic held-ACK case then exposes a second request accepted after the reply slot was consumed; its before-failure is /tmp/carlos-ep020/attached-stop/duplicate-before.log. The stopping guard now passes all six cases and both full workspace feature/lint matrices; logs are /tmp/carlos-ep020/attached-stop/. The public manager/CLI path is still incomplete.

- [x] (2026-11-15) Add the pre-publication event-stream continuation and Launched.spawn_after API. Three Core cases pass: reset an existing client at exactly the old cursor plus one, reject a live owner rebase, and reject exhaustion without mutation. Expose EngineUpdate.seq independently of page encoding so a failed shared page cannot hide the final cursor. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-stream/. Manager replacement remains incomplete.

- [x] (2026-11-15) Give each Core owner a fresh internal UUID for settings/goal/queue/task capability hashes. Two new tests pass with identical logical/native IDs and connection counters: old queue/task addresses and task runs cannot match the replacement, and unrelated root text does not rotate the same owner's scope. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-namespace/. Event cursor continuity and manager routes remain incomplete.

- [x] (2026-11-15) Add the durable stopped flag and transactional catalogue v1→v2 migration. Three new cases pass with the original registry cases: exact stopped native identity after restart, repeatable old-schema upgrade, and rollback of both column/version for an invalid old row. Stopped metadata cannot record Ralph enabled. Both full workspace feature/lint matrices and all143 actual-daemon regressions pass (75.739s); logs are /tmp/carlos-ep020/registry/. Public stop/reopen remains incomplete.


- [x] (2026-11-15) Read the accepted EP019b result, final Fable findings, current manager/owner/transport/store interfaces and the architecture roadmap. Confirm clean closeout commit `07bad8c` and preserved protected main/installed executable.
- [x] (2026-11-15) Choose explicit stop/reopen before network work, durable stopped metadata, bounded off-manager joins, one network WebSocket owner per connection and operator-provisioned device tokens.
- [x] (2026-11-15) Add EngineHandle.shutdown_with_updates, a one-shot final report after native/worker cleanup. Move handle methods into their own module. Two new Core cases pass alongside existing shutdown cases: deferred-native closure with retained private acceptance, and exact private draft recovery despite an oversized shared page. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-report/{default,no-default}-{tests,clippy}.log.
- [ ] Implement manager typed session stop/reopen, persist stopped metadata and prove reuse of all 16 owner slots without an implicit restart. Collect any already-issued Live.updates reply before discarding the old Live; its outcomes may already have been drained from Core into that channel.
- [ ] Add terminal and command-line stop/reopen controls with visible/request assertions, including recovery when owner capacity is already full.
- [ ] Add durable per-device token records and local-only operator controls. Bind network retry credentials to the authenticated device and revoke existing connections promptly.
- [ ] Add the optional bounded WebSocket listener and its real clients without a new async runtime, second session owner or unbounded buffers.
- [ ] Run the complete feature/release/terminal/native/dependency checks, actual authenticated socket fault tests and matched renderer/socket/source measurements.
- [ ] Finish the direct engineering and Fable whole-roadmap reviews, record every disposition and delegated approval, archive this completed plan unchanged and continue to EP021.

## Surprises & Discoveries

Explicit owner stop can skip the final normalized native-close message. Retire both approval queues and the active card after worker completions, before publishing the final Closed state, so a closed session cannot keep a live question.

Explicit stop needs its own stopping flag: setting the normal shutdown flag first would stop the native event forwarder before an attached-turn interrupt could be acknowledged. Freeze the owner loop with stopping, retain the native reader until interruption/idle verification finishes, then run the existing shutdown. Once the owner consumes its stop reply slot, the stopping flag must also reject a second request; otherwise that request can remain pending forever. A held native acknowledgement reproduces this race without sleeps.

An attached Codex app-server is not owned by Carlos and may serve other conversations. Explicit stop must request interruption of only the captured selected native turn before closing its Carlos owner; it cannot kill that external server. Native interruption failure must stay visible rather than claim the remote turn definitely stopped. Owned helpers still close their owned process groups. Inspect the existing captured interrupt command and its bounded teardown before wiring the public stop action.

EngineUpdate previously exposed its cursor only inside successful encoded changes. A shared-page error can therefore hide a newer owner cursor even though a client received a smaller direct snapshot. Carry EngineUpdate.seq independently and seed the next owner from that actual final value, not only the manager's last successful broadcast. The oversized shutdown case now checks both private recovery and the retained final cursor.

A recreated owner starts EventState.seq at zero, while Replica.apply ignores events at or below its current sequence. Same-daemon stop/reopen therefore needs explicit cursor continuity and a reset/snapshot before new facts; a daemon-restart hello cannot solve it. Settings revisions currently hash the logical session/native thread/connection counters, which can repeat in a new owner. Before enabling replacement, prove stale settings/task/queue/goal and source/card references cannot match that replacement. Inspect existing per-entry namespaces and hash roots; prefer an internal fresh owner namespace plus carried sequence over adding a second logical session ID or a speculative generic transport layer.

The manager's apply_update expects Live to remain installed while it publishes changes. Retain that Live until the stop worker joins the owner, but gate all new mutations/reads and move its pending updates receiver and notice join into the worker. This lets the manager apply prior/final frames through the existing duplicate-sequence checks before dropping the owner. Also cover stop during native launch: cancel the captured launch and drain its exact late result instead of accepting a replacement or dropping a live native handle.

A final owner read alone is insufficient if the manager already requested Live.updates: Core may have collected private results into that pending reply before shutdown. The stop worker must consume that existing receiver as well as the final read, preserving exact outcomes before using canonical-command uncertainty for commands the owner never saw. Explicit final shutdown also marks deferred-native availability closed after late completions, rather than exporting a stale Ready state.


The accepted daemon has no public stop command. `Session.live` is taken only during `Manager::shutdown`; create/restore/open count live or launching owners against a hard bound of 16. Dropping a client correctly leaves a conversation alive, but it also leaves an abandoned owner allocated forever. Stopping must therefore free an owner while keeping its saved native address.

`Owner::run` already closes native transports, stops workers, collects completions and publishes final state when its stop flag is set. `EngineHandle::join` currently returns only `()`, so a per-session stop must explicitly collect final private outcomes. The manager must also settle commands admitted to its retry store but not yet seen by the owner. A plain handle drop would lose those results.

The catalogue currently has one sessions table with schema version 1. A saved description contains the logical ID, native address, backend, directory, name and whether Ralph was enabled. Add a stopped flag rather than erasing the native address. Otherwise a reconnecting client's automatic subscription would relaunch a session the user just stopped.

The Unix transport uses independent bounded reader/writer threads. A network WebSocket parser must belong to one thread; sharing it behind a mutex while a reader blocks would prevent outgoing facts from reaching the client. The network thread must wake on both socket readiness and queued output instead of copying the terminal's eight-millisecond polling loop.

The locked tungstenite version is 0.30.0. Its config allocates a read buffer eagerly and leaves the write-buffer maximum unlimited by default. Set every buffer/message bound explicitly. The existing BLAKE3 Hash type compares digests in constant time; keep the Hash type when checking token verifiers rather than converting it to byte slices for equality.

## Decision Log

- 2026-11-15, root: Reuse existing card retirement methods after workers join. Do not synthesize native answers or promote a queued question during stop.

- 2026-11-15, root: Rename the new Core API to stop_session and return a typed Stopped reply with independent native confirmation. Interrupt only the captured attached Codex turn through existing guarded ControlWork with a two-second stop deadline; ordinary controls retain ten seconds. If no safe local turn is known and no send is in flight, read native thread status without materializing history and confirm idle; an active/unknown result remains uncertain. Keep ordinary daemon teardown unchanged and never kill the unowned app-server.

- 2026-11-15, root: Use one private UUID in EventState and the existing five digest roots to retire replacement-owner controls. Entry/card/source addresses already use their allocator's fresh UUID, so keep them unchanged. No new wire field, logical ID, timer or generic scope framework is needed. Preserve the same owner's scope across native conversation reset and let existing helper-generation guards handle native replacement within that owner.

- 2026-11-15, root: Reopen must preserve logical session identity while advancing the numbered stream beyond the stopped owner and publishing a reset. Add failing-before/passing-after client tests for this boundary. Give replacement owners fresh internal capability scope where existing hashes could repeat; verify actual shared hash roots before choosing the minimal change. Old operation retries still return retained outcomes before inspecting the replacement.

- 2026-11-15, root: Add a one-shot shutdown read, not another polling queue. Register its bounded reply before setting the stop flag; return final changes/outcomes after native cleanup. Keep existing shutdown/join callers and collect in-flight manager replies in the stop worker. Move only handle methods to engine/handle.rs to preserve file size caps.


- 2026-11-15, root: Treat session stop/reopen as the first required milestone, as Fable explicitly allowed in its EP019b final review. Do not enable network access before its actual multi-client/native acceptance checks pass.
- 2026-11-15, root: Stop explicitly rather than silently evicting idle conversations. A stop keeps saved native metadata, turns Ralph off, closes native processes and publishes closure. A reconnecting subscription never reopens an explicitly stopped session. Only a new explicit reopen/open operation may do so.
- 2026-11-15, root: Keep the manager free of waits. Retain stopping owners in the 16-owner count until their native processes and threads finish; perform joins in bounded lifecycle workers. Reject reopen while that exact owner is stopping instead of launching a second owner.
- 2026-11-15, root: Use the existing provisional JSON-RPC protocol and private operation identity for stop/reopen. Do not expose generic native method JSON. An identical retry returns the retained outcome and cannot stop a replacement owner.
- 2026-11-15, root: Keep the local Unix listener and its kernel credential checks. Add explicit TCP/WebSocket access with tungstenite and standard threads; do not add tokio, axum, compression or a timed delta merger.
- 2026-11-15, root: Provision device credentials through an authenticated local operator command. The CLI writes a fresh 256-bit token to a new mode-0600 file and sends only its verifier to the local daemon. The daemon stores a verifier and bounded device name/ID, never plaintext tokens. Network clients cannot call operator methods.
- 2026-11-15, root: Authenticate network clients in their first hello message, not a URL/query string. Bind private resume rights to the device, independently of the public client name. Revocation closes that device's sockets and prevents old private leases from being resumed under another device.
- 2026-11-15, root: Leave the terminal-only dense Unicode index out of the daemon, client library and phone. Preserve its measured limit in the handoff. Do not introduce the source-body cap or file/mmap fallback rejected in EP019b.

## Outcomes & Retrospective

The Core foundations and durable stopped metadata now pass their checks. Core stop_session returns final facts/private results plus native confirmation; event continuation preserves attached clients; fresh internal owner UUIDs retire stale control addresses. The catalogue migration preserves native addresses and rejects inconsistent stopped/Ralph metadata. The public manager, CLI/terminal lifecycle actions and network listener are still incomplete. Next wire the bounded manager stop worker, exact old-owner results and durable acceptance, then prove actual native process/16-slot behavior.


The first owner shutdown-report piece is implemented and its two new cases pass. Both workspace feature/lint matrices pass. The public session stop/reopen command and durable stopped metadata are still incomplete. Next implement the manager stop worker, including its existing pending updates reply and unconsumed retry records, then prove real-native owner release. The accepted EP019b pair and measurements remain frozen for matched comparisons.

## Context and Orientation


`crates/carlos-core/src/engine/owner.rs` owns one mutable session and native backend. `engine/mod.rs` defines commands, replies and EngineHandle; `engine/native.rs` and `engine/workers.rs` close native I/O and settle work. The handle exposes a stop flag and join, but no final report. Move handle methods into a focused `engine/handle.rs` if adding the shutdown read would cross the 400-line file cap. Preserve the existing 60-line function cap and explicit imports.

`crates/carlosd/src/manager/mod.rs` owns the session catalogue, client connections and bounded retry store. `manager/state.rs` holds Session/Live. `manager/lifecycle.rs` launches sessions and saves descriptions. `manager/updates.rs` routes numbered shared changes and private outcomes; reuse it for the final shutdown report. `manager/startup/open.rs` already attaches existing owners and reopens a closed Claude helper. Add the logical-session reopen path here or in a focused lifecycle module; never create a new logical ID for an explicit reopen.

`crates/carlosd/src/registry.rs` and `manager/store.rs` own SQLite metadata and one disk worker. `retry/mod.rs` retains canonical typed commands and private outcomes under each ClientOperation. Its reserved bytes include recovery space. Use those retained commands to settle operations the owner never consumed during stop; never broadcast that text.

`crates/carlos-proto/src/commands.rs` defines public commands; the sibling command/rpc modules define method names, decoding and strict field checks. `crates/carlos-client/src/client_requests.rs`, `operations.rs` and its tests retain commands and outcomes without platform I/O. Extend these existing routes for typed lifecycle and device operations. Keep protocol version 1 provisional.

`src/app/remote/local_commands.rs`, `events.rs`, `status.rs` and `help.rs` own terminal commands and local hints. `src/app/cli.rs` parses startup/perf options. A capacity-full user needs a command-line stop path that does not first create or restore another native session. Add focused CLI/remote modules rather than expand the already capped files. Keep the terminal's existing private composer intact after stop and reopen.

`crates/carlosd/src/server.rs` currently accepts only verified UnixStream connections through a bounded channel. `transport/mod.rs`, `reader.rs`, `ingress.rs`, `outbox.rs` and `wire.rs` own bounded framing. Extend this boundary for accepted network peers; keep one Manager and one Session owner. `crates/carlosd/src/main.rs` parses explicit socket/state paths. Preserve old invocations and managed-terminal startup with no network listener by default.

`tests/daemon/support.py` starts actual daemon/native fixtures and records both directions. `tests/ui/test_workflows.py` drives the terminal through real private daemons. New lifecycle tests belong beside these fixtures. Add real WebSocket tests under `tests/daemon/` and run them from the normal script. `crates/carlosd/examples/perf_replay.rs` and its replay modules already use the shared synthetic Core generator and bounded production page/output shapes over a real Unix pipe. Extend that evidence through a loopback WebSocket while keeping its scope distinct from actual-daemon admission.

## Plan of Work


### Milestone 1: stop and reopen one logical session safely


Add typed `session/stop {session, operation}` and `session/reopen {session, operation}`. Both use the current retained operation counter and exact logical session ID. Stop belongs to the control reserve so a full ordinary queue cannot prevent it. Check existing identical operations before looking at the current owner; a retry from an old owner generation must never stop a new owner. Reopen is an explicit ordered lifecycle action; coalesce concurrent opens of the same stopped session and reject a stop/reopen race until the old owner has actually joined.

Add an EngineHandle shutdown method that registers a one-shot final update read before setting the stop flag. The owner must close native work, drain completions, publish closure and return its final bounded EngineUpdate/outcomes. Keep the current shutdown/join API usable by existing callers. The manager captures the current sequence, moves the old Live into a bounded stop worker, and continues serving other sessions. Count that stopping owner until join completes. Do not abandon its native processes, reuse their PIDs unsafely or block the manager on a join.

Route final owner outcomes through the existing private result path. Then finish every remaining pending/queued retry record for that logical session conservatively from its retained canonical command. Preserve the original Send/Rewind/Queue text under its own ClientOperation, mark unconfirmed work uncertain and never replay it. A successfully accepted old command stays accepted. A late old completion cannot become an outcome for a replacement owner. Close or fail outstanding reads/watches for the stopped owner without silently retargeting them.

Add a stopped boolean to SavedSession and migrate catalogue version 1 transactionally, defaulting existing rows to false. Reject newer versions without rewriting them. Stopped metadata retains the native address and disables Ralph. Show Closed for a stopped description; automatic subscribe must reject with an explicit-open remedy rather than launch it. Save only changed metadata on the existing disk worker. A stop reports accepted only when the owner has joined and the stopped description is durable; a save failure reports uncertainty, keeps the in-memory owner stopped and does not claim durable success. Explicit reopen clears the flag only through the checked launch/save path. Preserve backend-specific resume behavior, including Claude's failed-fresh-branch identity and Grok's saved native session.

Prove this with an actual 16-owner fixture: fill all slots, reject owner 17, stop one, verify its native PID/helper/descendants exit, then create owner 17 while the other 15 stay alive. Reopen the stopped logical ID after a slot is freed and verify its native identity/history. Test stop during held send/settings/queue work, exact private drafts for two clients, duplicates after replacement, concurrent stop/reopen, disconnect before reply, catalogue failure and daemon restart. A subscription or socket reconnect must not reopen a stopped session. Keep all original deadlines and use gates that wait for actual requests/results rather than sleeps.

### Milestone 2: give the terminal and operator a usable lifecycle command


Add `/session stop` and `/session reopen` to the terminal's local command/help routes and typed client requests. The commands act on the attached logical session, keep any newer composer draft and show private pending/failure results. Update the accepted closed-helper workflow only for an explicit new reopen action; uncertain text still needs a separate Enter. F5 retains its documented transport/native retry behavior unless a concrete UI test justifies changing it.

Add `carlos session stop ID` and `carlos session reopen ID`, with explicit `--daemon-socket PATH` support and the normal managed-local route. These commands greet the daemon, submit exactly one operation, wait for its retained result and exit without first creating or restoring a session. Print the logical ID and actual outcome, not native keys or private drafts. A user at full owner capacity must be able to release a slot through this command. Add terminal screen/request and command-line process tests, including two terminals with different drafts.

### Milestone 3: provision and revoke device access locally


Add a bounded device record store on the existing disk worker/SQLite connection. Store a random opaque ID, a single-line bounded name and a BLAKE3 verifier of a fresh 32-byte random secret. Use the existing getrandom dependency and add the already-used blake3 crate directly where needed. Keep secrets out of diagnostics, Debug output, URLs, command canonical bytes and daemon metadata. Load only a bounded number of active devices and reject malformed/newer store rows without rewriting them.

Provide `carlosd device add --socket PATH --name NAME --token-file PATH`, plus local list/revoke commands. Generate and create the token file with exclusive create, mode 0600 and no symlink overwrite. Send the verifier through local-only typed operator methods and wait for its durable result. Repeated admission cannot silently issue another credential. Revocation frees the active-device slot and closes that device's existing connections; a disk failure cannot claim durable revocation. Network peers must be rejected from every operator method even with a valid device token.

Keep a connection's trusted Unix identity separate from a network device identity. Extend retry leases with that scope. A device needs both its access token and its private resume credential; the public client name is never authority. Another valid device cannot resume the first device's client lease or retrieve its private outcomes. Local terminal behavior and current private reconnect tests stay unchanged.

### Milestone 4: serve the same commands over bounded WebSockets


Add optional `--listen ADDRESS:PORT` to carlosd; reject wildcard addresses and keep default startup Unix-only. Bind port 0 in tests and report the actual address without secrets. The operator selects loopback or its WireGuard address. Set TCP_NODELAY; offer no compression. Permit native clients with no Origin header. For browser requests, require an explicitly configured exact allowed Origin; reject unmatched origins during upgrade. Do not put the bearer token in the URL or subprotocol.

Use the locked tungstenite 0.30.0 server handshake with a fixed deadline, bounded concurrent handshakes and bounded headers. A handshake/read must run off the manager. Keep reserved local connection capacity so unauthenticated network peers cannot block the operator's Unix stop/revoke commands. Require a valid first hello token before any session/list/private data or native launch. Bound unauthenticated input to a small greeting frame and a short deadline; a second hello, binary protocol frame, malformed UTF-8/JSON, oversized aggregate fragmented message or unknown method must fail without dispatching backend work.

After authentication, reuse the existing request decoder, ingress reserves, outbox byte/count bounds, snapshot recovery and private results. Set explicit WebSocket read/write/message/frame limits rather than its defaults. Keep a charged outgoing frame until the actual WebSocket flush completes. Handle ping/pong and close frames correctly, cap native write waits and disconnect a slow client without stalling another client or session. One network thread owns each WebSocket; wake it for both TCP readiness and outbox pushes with a bounded wake descriptor, not a periodic redraw timer or a mutex held across read. Keep the existing Unix reader/writer behavior and credential checks.

Test the actual listener with authenticated and rejected peers: two clients share one native session, different drafts/results stay private, a running source watch survives reconnect from its cursors, revocation closes live sockets, stale credentials never replay a command, ping/fragmentation work and oversized/slow/incomplete handshakes stay bounded. Include an admitted callback race and held native send to prove the same manager/control behavior as Unix. The client library remains pure; tests supply transport I/O around it.

### Milestone 5: prove correctness, speed and the whole roadmap


Run both Rust feature matrices and strict lint, architecture/dependency checks, the full repeatable terminal runner in release mode with installed Codex/Claude contracts, package smoke and the new lifecycle/network cases. The current root shell has uid 0 below a uid 1000 home; use a documented private test-script copy that changes only the artifact root to /tmp and the absolute cwd, as EP019b did. Keep product path protections and original assertions/deadlines unchanged. Preserve failed traces and investigate a recurrence of the earlier unexplained timeout; do not erase it by rerunning until green.

After all builds/tests finish, freeze matching binaries/helper and source hashes. Run `carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48` and the captured Core fixture against the accepted EP019b pair with alternating matched samples. Measure actual authenticated WebSocket admission/source transfer versus Unix with the same deterministic native fixture and exact 1,000 native prompts. Extend the shared synthetic owner/page replay over a real loopback WebSocket and report its separate scope. Do not add a production fixture-injection command merely to benchmark. Record startup/binary bytes, kernel RSS, idle wake behavior, decode/transfer costs and p95/max, not only a successful handshake.

Obtain the final engineering and Fable 5.2 medium review of EP016–022's complete objective. Address concrete defects and state why optional redesigns are declined. Keep the dense terminal-index limit and any new bounds visible. Complete all four living sections, record delegated approval, move this completed plan unchanged into `.agents/done/`, update PROGRAM_PLAN.md and write EP021. Continue without the former per-EP human gate.

## Concrete Steps


Run all commands from `/var/home/wegel/work/perso/carlos-daemon`; shell tool calls use the required rtk wrapper. Start with source-oriented search and named file reads before edits. Keep commits small and buildable with scoped imperative subjects.

    rtk cargo test --workspace --all-targets
    rtk cargo test --workspace --all-targets --no-default-features
    rtk cargo clippy --workspace --all-targets -- -D warnings
    rtk cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract

The final command's root-owned harness needs the private artifact-root adaptation described above in this environment. Record the exact adapted command and output paths. It must run the same tests and deadlines. Add lifecycle/network tests to the normal checked script so CI and ordinary users run them without an account or model call.

For manual private smoke tests, build matching debug executables and start carlosd in a fresh mode-0700 directory. Start with Unix only, stop/reopen a fixture conversation and verify the recorded native PID/count. Provision a disposable device token file, then start a loopback listener with an exact allowed browser Origin only if testing browser behavior. Use only the temporary daemon/native fixtures, never the owner's installed executable, real sessions or actual network address.

## Validation and Acceptance


The user can stop a logical session from a terminal or the command line, free one of 16 owner slots, and explicitly reopen the same saved conversation. Another terminal keeps its different draft. Reconnecting and subscribing never undo a stop. Duplicate old operations cannot affect a replacement owner. Every admitted private draft gets an accepted/rejected/uncertain result rather than remaining pending forever. Disk failure is visible and does not claim durable success.

The daemon remains Unix-only by default. A loopback WebSocket client with a provisioned token sees the same checked session facts and private results as a Unix client; a missing, malformed, wrong or revoked token sees no session data and starts no native work. A different device cannot borrow private retry rights. The operator can revoke a device while it has an active socket and can still reach the private Unix controls when network capacity is full.

The same actual terminal/native workflows and exact large-source copy checks pass after the new lifecycle/transport paths. Slow or malicious network peers cannot block a different client's interrupt, answer or local input. Matched measurements preserve interactive speed and explain any real fixed costs. No result may hide behind disabled native checks, increased test timeouts or a new body-size cap.

## Idempotence and Recovery


Use new private fixture directories for each run. Stop only the specific test-owned daemon/PIDs and verify their children exit; leave the owner's long-running Carlos/Codex processes alone. Save older traces with source/binary identities. Do not overwrite a token file or unsafe socket/catalogue path. The SQLite migration must be transactional, repeatable and reject newer versions before rewriting rows. Preserve a copied version-1 fixture for failing-before/passing-after checks. A failed token provision/revoke or session stop uses its retained explicit outcome; it never automatically retries native work under a new operation.

## Artifacts and Notes


Accepted EP019b baseline: runtime source `23d6e70775c623b8106074957efb103d`, implementation `cbdca8f`, closeout `07bad8c`. Frozen release pair: `/tmp/carlos-ep019b/review-accepted-pair/identity.json`. Final measurements: `.agents/reports/exec019b-final-measurements.json`. Root's delegated approval and complete Fable findings are in the archived plan and its final reports. Main remains `a29b1a1e3720e48b09e2ca984fb16c29b5857ea3`; installed Carlos SHA-256 remains `d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3`.

The pinned library references checked while writing this plan are the official tungstenite 0.30.0 WebSocketConfig documentation and BLAKE3 Hash documentation. The needed facts are embedded above: set explicit buffers/aggregate message bounds; a single thread owns WebSocket protocol state; BLAKE3 Hash equality retains constant-time comparison. No external article is required to execute this plan.

## Interfaces and Dependencies


Keep carlos-client free of sockets, clocks, threads, terminal and native backend packages. Add only the lifecycle/operator command data it needs. Use `ClientOperation`, the existing RetryStore reserves and the same bounded `OperationResult` for every mutation. Use typed `SessionId` and a new opaque device ID; never use a native process/session string as a public client address.

Extend EngineHandle with a one-shot final update read associated with explicit shutdown; it must not block the caller or expose SessionState. Keep normal shutdown and join callers working. Put manager stop workers, stopped metadata and exact old-owner completions in focused modules. New modules must stay below 400 lines and functions below 60 lines.

Add tungstenite 0.30.0 and blake3 as direct daemon dependencies using versions already in Cargo.lock. Standard threads, nonblocking sockets, the existing ingress/outbox and a bounded wake descriptor are sufficient. Do not add an async runtime, generic transport framework, HTTP application server, cloud broker or device-pairing service. The future phone can use its native WebSocket API around the same pure client; richer native history remains EP021 and the actual Swift/UniFFI app remains EP022.

Revision 2026-11-15: create EP020 after approved EP019b. Incorporate the final review's owner-release/reopen repair as the first required milestone and specify the complete authenticated network outcome and its acceptance checks.

Revision 2026-11-15: add and test the Core one-shot final report. Record the in-flight owner-result receiver that a manager stop must also drain, and keep the public lifecycle milestone incomplete until actual process/slot tests pass.

Revision 2026-11-15: persist stopped metadata with a checked, repeatable migration and record the manager live-report/launch lifetimes that the next public stop slice must preserve.

Revision 2026-11-15: correct the recorded daemon-run elapsed time to75.739s from its actual log. Before recreating an owner within the same daemon instance, verify numbered-event continuity and stale capability rejection; the existing daemon-restart handshake is not a substitute for that proof.

Revision 2026-11-15: preserve cursor continuity and stale control references across same-daemon owner replacement, before adding the manager stop/reopen routes. Registry commit04edc7c passes both feature/lint matrices and143 actual-daemon cases in75.739s; the recorded earlier elapsed time is corrected above.

Revision 2026-11-15: add the minimal private owner UUID to existing control digests and test stale controls without changing public command shapes. Carry the event sequence before wiring manager replacement.

Revision 2026-11-15: continue replacement streams with a forced reset, reject live rebasing/exhaustion, and retain the final cursor independently of a shared-page encoding failure.

Revision 2026-11-15: complete both owner-stream Rust feature/lint matrices and preserve the owned-versus-attached backend stop boundary.

Revision 2026-11-15: implement attached-turn stop confirmation and separate stopping/transport-shutdown flags; preserve the held-ACK failing regression before applying the duplicate-request guard.

Revision 2026-11-15: finish all attached-stop cases and both feature/lint matrices, with the held-ACK before/after proof. Core foundations are checked; public stop/reopen remains the next required work.
