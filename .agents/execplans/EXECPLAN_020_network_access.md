# EP020: release stopped sessions and serve authenticated network clients

This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. Work directly in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`. EP019b is approved and archived; do not edit it. The owner delegated approval and continued work through the phone spike. Obtain the final direct engineering review and Fable 5.2 medium whole-roadmap review after the whole EP is ready, address the findings, record delegated approval and continue to EP021. Do not install a spike executable, merge main or push.

## Purpose / Big Picture


A user must be able to stop a conversation's native processes without deleting the saved conversation or restarting the entire daemon. Another terminal must keep its own drafts when that happens, and an explicit reopen must restore the same native conversation. This comes first because EP019b's final review found that the daemon otherwise keeps all 16 owner slots until shutdown.

After that repair, a phone or other network client can connect to an explicitly configured WebSocket address and authenticate with its own device token. A WebSocket is one ordered two-way stream that phones and browsers already support. Each text message carries the same typed JSON-RPC request or reply used on the local Unix socket. Both transports must use the same session manager, numbered facts, private operation results and queue bounds. The local terminal remains a Unix-socket client; this EP does not build a phone screen or add speculative phone commands.

The daemon can run shell commands as its user. Network access stays off unless the operator supplies a specific listen address. Plain `ws` is intended for loopback or the configured WireGuard interface, which encrypts traffic between the user's devices. Do not configure a real external interface, firewall or device on the owner's behalf during this task. All acceptance checks use private directories and loopback ephemeral ports.

## Progress

- [x] (2026-11-16) Connect typed local device add/list/revoke to the existing disk worker and unscoped private operation results. Four actual-daemon workflows pass (1.143s) at /tmp/carlos-ep020/device-controls.log: durable restart/private recovery/no native launch, old-add retry after revoke, native send while device SQL waits and failed add/revoke with explicit retry. Add an independent16-operation/128KiB result reserve and16-command/64KiB ingress reserve; preserve FIFO and skip idle waits while ingress remains queued. Central retry-token Debug redaction passes the focused protocol/client/daemon tests; all1650 default workspace tests, strict lint and the400/60 audit pass. The next slice is the operator CLI. The listener and actual network-device authentication remain unimplemented.
- [x] (2026-11-16) Add catalogue version4 with at most32 named device verifiers. Preserve every saved session during v1/v2/v3 upgrades, reject future versions and malformed device rows transactionally, and permit only exact duplicate adds rather than credential/name replacement. Revocation frees capacity and is repeatable. All12 registry cases pass, including four new restart/capacity/v3-upgrade/NUL-and-type cases; default workspace1646 tests, strict lint, architecture19, dependency guard and400/60 source audit pass. BLAKE3 is already used by Core and is now explicitly reviewed as a direct daemon dependency. Device writes still need the existing disk-worker/manager/operator route; no token is issued yet.
- [x] (2026-11-16) Add named DeviceAdd/DeviceRevoke payloads, bounded operator metadata and a redacted canonical AccessToken. Move hello's existing fields into one named Hello payload without changing its wire shape; add Client/Connection.begin_with_token without sockets, timers or secret storage. The manager still rejects nonempty tokens on Unix, and operator dispatch/storage remains unimplemented. Three new protocol cases and one pure connection case pass with all138 protocol/client cases; default workspace1642 tests and strict lint pass. Preserve both bearer and private-resume bytes in wire greetings while hiding them from request Debug. Update the provisional fixture's former placeholder token and include the new method names in the daemon decoder.
- [x] (2026-11-16) Add typed DeviceId and device-scoped retry leases without enabling network access. An exact private credential cannot cross devices or become a Unix lease. Keep128 local identities independently of128 network identities and evict only inactive identities in the same class. Revoking one device forgets all its leases/results; late owner results stay ignored. All12 retry tests, including three new privacy/reserve/revocation cases, pass; default workspace1638 tests and strict lint pass. The400/60 source audit passes. The manager still admits Unix peers only; device records, operator commands and actual network authentication remain next.
- [x] (2026-11-16) Pass the complete adapted stock release gate at runtime commit9da3d3b: all161 daemon cases93.033s,310 terminal workflows417.256s,23 installed-Codex cases33.409s and10 packaged installed-Claude cases27.776s. Helper72 cases, architecture19, package/selection/source/dictation/ANSI/ownership checks and release Rust/lint also pass. Command: bash /tmp/carlos-ep020/run-lifecycle-gate.sh --release --codex-contract --claude-contract; exit0, log /tmp/carlos-ep020/lifecycle-release-gate.log and artifacts /tmp/carlos-ep020/lifecycle-gate-Nfi7bZ. Keep the earlier failed debug trace. Device/network changes still require this EP's final full checks.
- [x] (2026-11-16) Preserve the exact fresh-Claude UUID across explicit stop, daemon restart and reopen, then adopt saved history once the first prompt creates it. Reuse the existing metadata resolver and register cancellation before its first helper call. Carry a typed Core resume kind; persist only the Claude fresh bit in catalogue version3. Never save Grok's unconfirmed starting placeholder. All three new actual-backend workflows pass (3.352s), then the complete daemon suite passes161 tests (93.198s); traces /tmp/carlos-ep020/native-targets and lifecycle-current-regression. Both Rust feature matrices pass (1635 default,1592 without defaults), both strict lint matrices pass, and the source-size audit remains399/60. The complete release/terminal gate remains part of this EP's final checks.
- [x] (2026-11-16) Obtain the owner's explicitly requested mid-EP whole-roadmap bloat review from canonical claude-fable-5-2 at medium effort, read-only session6426dd2a-bdc0-44f8-b1d2-d3b087fe019c. Preserve the full report and metadata in .agents/reports/exec020-fable-bloat-review.{md,json}. It finds some avoidable panel/read/command-wrapper code, while considering the shared owner/replica/retry/transport design proportionate. This is not final EP approval. Check its concrete suggestions before changing behavior; it could not diff main or read the temporary count artifact.
- [x] (2026-11-15) Replace direct Grok/Claude Child handles with one private owned-family handle. Create a dedicated process group, observe exit with WNOWAIT before reaping its leader and kill remaining owned descendants before releasing that ID. Both new isolated leader-live/leader-exited checks and all seven inherited-pipe shutdown checks pass. Three committed actual-daemon supervisor cases now pass for Grok, the compiled Claude helper/native CLI and Codex while a bystander stays alive (6.683s); /tmp/carlos-ep020/owned-family-after.log. Preserve the explicit unowned-pipe latency fixture by moving its holder into its own session; never signal that escaped holder on backend stop. Backend target-kind persistence remains next.

- [x] (2026-11-15) Wire typed stop/reopen through the protocol, pure client and daemon. Keep joined stops pending until their exact stopped metadata is saved; preserve private results from both owner receivers and recover unconsumed canonical commands without resending. A separate bounded ingress/result reserve keeps stop available when ordinary/native-control work fills. Four actual-daemon cases pass: 16-slot release/reuse/history, stopped restart/subscription, held SQLite save/reopen order and exact queued drafts for two clients. The next three actual cases also pass: failed stopped-row save, cancelled held launch and coalesced reopen with exact stream continuity. Two real attached-socket cases then pass: selected-turn interruption/card retirement without closing an external bystander connection, and visible lost-ACK uncertainty followed by explicit reopen. Both workspace feature matrices and strict lint pass; the full prior daemon suite plus the first seven new cases passes150 tests in76.617s at /tmp/carlos-ep020/manager-regression-venv.log. The first full attempt used system Python and failed111 imports because websockets belongs to target/ui-venv; it was an environment error, not150 runtime failures. Actual helper/descendant teardown and terminal/CLI checks remain required before this milestone is complete. First traces: /tmp/carlos-ep020/manager-first; corrected queued-work check: /tmp/carlos-ep020/manager-third.

- [x] (2026-11-15) Retire active and queued approval cards after final worker cleanup, even if the owner stops before reducing the native close message. The focused Core shutdown cases pass. One initial test incorrectly assumed cursor zero forced a snapshot; request a deliberately unmatched cursor to verify the final full snapshot instead. Public lifecycle routes remain incomplete.

- [x] (2026-11-15) Add explicit Core stop_session with EngineReply::Stopped carrying final updates and a separate native interrupt result. Preserve ordinary shutdown behavior. Five captured-writer cases pass for exact attached turn/bystander, lost acknowledgement, normal shutdown, confirmed idle read, and native activity without a safe turn ID. A deterministic held-ACK case then exposes a second request accepted after the reply slot was consumed; its before-failure is /tmp/carlos-ep020/attached-stop/duplicate-before.log. The stopping guard now passes all six cases and both full workspace feature/lint matrices; logs are /tmp/carlos-ep020/attached-stop/. The public manager/CLI path is still incomplete.

- [x] (2026-11-15) Add the pre-publication event-stream continuation and Launched.spawn_after API. Three Core cases pass: reset an existing client at exactly the old cursor plus one, reject a live owner rebase, and reject exhaustion without mutation. Expose EngineUpdate.seq independently of page encoding so a failed shared page cannot hide the final cursor. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-stream/. Manager replacement remains incomplete.

- [x] (2026-11-15) Give each Core owner a fresh internal UUID for settings/goal/queue/task capability hashes. Two new tests pass with identical logical/native IDs and connection counters: old queue/task addresses and task runs cannot match the replacement, and unrelated root text does not rotate the same owner's scope. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-namespace/. Event cursor continuity and manager routes remain incomplete.

- [x] (2026-11-15) Add the durable stopped flag and transactional catalogue v1→v2 migration. Three new cases pass with the original registry cases: exact stopped native identity after restart, repeatable old-schema upgrade, and rollback of both column/version for an invalid old row. Stopped metadata cannot record Ralph enabled. Both full workspace feature/lint matrices and all143 actual-daemon regressions pass (75.739s); logs are /tmp/carlos-ep020/registry/. Public stop/reopen remains incomplete.


- [x] (2026-11-15) Read the accepted EP019b result, final Fable findings, current manager/owner/transport/store interfaces and the architecture roadmap. Confirm clean closeout commit `07bad8c` and preserved protected main/installed executable.
- [x] (2026-11-15) Choose explicit stop/reopen before network work, durable stopped metadata, bounded off-manager joins, one network WebSocket owner per connection and operator-provisioned device tokens.
- [x] (2026-11-15) Add EngineHandle.shutdown_with_updates, a one-shot final report after native/worker cleanup. Move handle methods into their own module. Two new Core cases pass alongside existing shutdown cases: deferred-native closure with retained private acceptance, and exact private draft recovery despite an oversized shared page. Both full workspace feature/lint matrices pass; logs are /tmp/carlos-ep020/owner-report/{default,no-default}-{tests,clippy}.log.
- [x] (2026-11-16) Implement manager typed session stop/reopen, persist stopped metadata and prove reuse of all16 owner slots without an implicit restart. Collect the already-issued Live.updates reply before discarding Live; preserve final private outcomes. Actual native process-family, fresh/saved target, restart and failure cases now pass with all161 daemon cases.
- [x] (2026-11-15) Add /session stop and /session reopen plus carlos session stop/reopen ID. Commands greet the existing explicit/managed daemon without a native session launch; invalid CLI commands exit unsuccessfully. Four real terminal/CLI cases pass, including full16-slot capacity and two independent composers. A complete adapted stock gate passes155 daemon cases (82.648s) and309/310 terminal cases (425.991s), then stops at a check that assumed old task addresses survive owner replacement. Update that workflow to reject old selection and require a fresh arrow selection before Enter. All five focused cases then pass (4.056s), along with workspace tests and strict lint. The failed full gate remains /tmp/carlos-ep020/lifecycle-gate.log with artifacts /tmp/carlos-ep020/lifecycle-gate-xxpYHI; it did not reach the later selection/contracts commands. A complete final gate is still required.
- [ ] Add durable per-device token records and local-only operator controls. Bind network retry credentials to the authenticated device and revoke existing connections promptly.
- [ ] Add the optional bounded WebSocket listener and its real clients without a new async runtime, second session owner or unbounded buffers.
- [ ] Run the complete feature/release/terminal/native/dependency checks, actual authenticated socket fault tests and matched renderer/socket/source measurements.
- [ ] Finish the direct engineering and Fable whole-roadmap reviews, record every disposition and delegated approval, archive this completed plan unchanged and continue to EP021.

## Surprises & Discoveries

The expanded ingress can retain256 commands while one manager pass handles224. Sleeping after that pass could leave admitted operator work queued unnecessarily. Keep each bounded pass's native/read/save work, then continue immediately when ingress is nonempty. The new queue test fills every non-operator class, proves the separate bounded operator reserve and drains all256 requests. No counter order changes.

SQLite text length and text substr can hide bytes after an embedded NUL. A truncated text projection could therefore accept a corrupt id or verifier's valid-looking prefix. Device loading slices bounded BLOB bytes and then decodes text, checks original column types and rejects embedded controls/noncanonical values. Fault tests inject hidden NULs into id, name and verifier plus a BLOB name; every open fails without rewriting the row or version. Keep those byte projections when changing the loader.

The provisional protocol fixture used a literal placeholder token, not a deployed credential. Canonical device tokens now require a32-hex device id, a dot and a64-hex secret; the local fixture uses its actual empty-token greeting. The exhaustive daemon decoder fixture also checks every named method, so new operator payloads must be recognized there even before their manager implementation. Both regressions now pass without weakening those tests.

The old128-client identity bound is independent of the16 live sockets. A device could repeatedly greet and disconnect to fill inactive leases even while its socket count stayed low. Preserve the existing local identity allowance separately when network leases are introduced. Socket reserves alone would not preserve the operator's ability to greet and revoke.

The fresh-Claude defect now has failing-before/passing-after evidence. A UUID does not prove a transcript exists: preserve a New intent until the existing sessions.info check finds saved metadata for that same UUID and directory. A typed NativeResumeKind prevents Grok's unconfirmed temporary label from entering the catalogue. The failed fresh-branch test also proves stop/reopen keeps the chosen branch and retained private edit without sending it again.

A separate actual Claude probe reproduces the next backend-specific restore defect: create a fresh UUID with no saved transcript, stop it, then reopen. The stop succeeds but reopen is uncertain because the manager always turns the UUID into ClaudeLaunchMode::Resume. The existing in-owner reopen code retains LaunchPlan::New and checks SDK metadata before deciding between the same fresh UUID and saved history. Preserve that fact across owner release/catalogue restart, and never save Grok's temporary grok-starting placeholder as a confirmed native address. Before trace/script: /tmp/carlos-ep020/fresh-reopen-before.log and probe_fresh_reopen.py. Do not start network access until these checked backend-specific cases pass.

An isolated actual-daemon process-family probe reproduces surviving descendants after accepted Grok and Claude stops; owned Codex passes. The probe uses the compiled Claude controller and native CLI fixture, not a stub controller. It cleans only captured test children under an isolated subreaper. Before traces and exact PIDs/results are /tmp/carlos-ep020/owned-descendants-before.{json,log}; script /tmp/carlos-ep020/probe_owned_descendants.py. Fix this before network access. Grok kills only its direct Child; forced Claude helper teardown also lacks an owned-group boundary. Never signal a cached process/group ID after its leader has been reaped.

The complete terminal run exposes one old assertion that depended on stable task IDs across a daemon restart. Fresh owner UUIDs intentionally retire those IDs. The client keeps accepted child history, rejects Enter for the missing selection and needs a fresh arrow selection before opening a new child reader. Keep those guards; update the screen guidance and repeatable workflow, including different old/new opaque task addresses. All five focused terminal/CLI cases pass after that correction. This was a deterministic changed-address contract, not an unexplained timeout.

A stop cancels old owner reads before replacement. Its final report must contain a full bounded snapshot, so a client waiting for a gap-repair snapshot can adopt Closed facts even though its original read was cancelled. Remove the unnecessary cursor argument from the new internal stop API and retain EngineUpdate.seq independently. Ordinary update reads still use their incremental cursor.

The Grok fixture acknowledges a prompt when its stdin write succeeds, before the turn finishes. Accepted input must stay accepted during stop. The two-client recovery case explicitly queues later text behind the visible running turn, instead of mistaking a completed stdin write for an unconfirmed prompt. The first fixture attempt used an unsupported trigger and the second omitted queue policy; their failures and wire traces remain under manager-first/manager-second.

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

- 2026-11-16, root: Reuse one typed Write enum on the existing disk channel for session descriptions and device actions. Keep at most one pending write per device, reserve pending new grant slots, and activate grants only on durable success. A queued revoke drops its old retry rights and blocks existing device records immediately; a failed durable delete stays visibly uncertain and blocked for this daemon lifetime. Check local scope before any operator admission. Existing identical operations return their retained result before checking current grant state, so an old accepted add cannot undo a later revoke.
- 2026-11-16, root: Redact credentials at their types. AccessToken already hides bearer bytes; extend the existing opaque-id macro's Debug policy only for ResumeToken and derive Hello Debug from those protected fields. This also protects derived greeting-result Debug without repeating redaction in every container. Keep encoded wire values unchanged. Reuse one Effects.read_reply helper for the three existing identical reply wrappers rather than adding another operator-specific request-id layer.
- 2026-11-16, root: Keep devices in the existing SQLite catalogue and reuse its transaction/version checks. Store only DeviceInfo plus a BLAKE3 Hash, bounded to32 records; no bearer secret, transcript or native job table. Exact duplicate provision is idempotent, while a changed verifier or name under an existing id fails. Validate both session and device rows before accepting schema version4. Queue these methods on the existing disk worker next, rather than adding another manager-owned connection or I/O lock.
- 2026-11-16, root: Keep device access tokens separate from retained private resume credentials. The platform supplies a token for each network greeting; the pure client only returns its encoded outgoing request. Make the bearer token's Debug redacted and omit both credentials from Hello Debug. Use the named Hello payload to keep the command enum below its cap while avoiding a second field definition. Operator payloads carry only verifier/metadata, never bearer secrets.
- 2026-11-16, root: Bind each retained retry lease to Local or Device(DeviceId), checked together with its private resume credential. Retain the old local hello API and reserve128 identities for each transport class; expired/evicted rights never become an automatic resend. Device revocation forgets the affected records, relying on complete's existing absent-record guard for late owner results. Authenticate device tokens in the manager before calling hello_in; this lease store does not itself grant device access.
- 2026-11-16, root: Record the checked mid-EP review decisions in .agents/reports/exec020-bloat-review-disposition.md. Share panel read settlement/paging and remove its terminal duplicate at the beginning of EP021, before Swift/web consumers. Keep captured older-native windows, close/root guards and complete lists; a single truncated snapshot would change supported behavior. Use named payloads for new manager-only operator methods now. Examine existing wrappers with the shared-read cleanup, without freezing provisional v1 merely because WebSockets carry it.
- 2026-11-16, root: Persist only the backend fact needed to reproduce Claude's existing reopen behavior. Keep Saved/ClaudeFresh/Unconfirmed typed inside Core, use one checked boolean only for Claude in the catalogue, and reuse the existing metadata resolver. Do not invent a new conversation or resend uncertain edits. Register launch cancellation before metadata/SDK startup, rather than after startup completes.
- 2026-11-16, root: Honor the owner's new request for one mid-EP bloat review despite the earlier end-only review cadence. Keep its advice distinct from final approval. Examine repeated panel drivers and command wrappers before expanding the protocol; reject any simplification that silently drops supported panel rows, weakens stale-reader guards or sends private outcomes to another device. No new reviewer is needed to examine those suggestions directly.
- 2026-11-15, root: Reuse one small private OwnedChild for Grok and Claude process-family ownership. This handle owns every reap, observes Unix child exit with WNOWAIT, kills the owned group before releasing the leader ID, and caches the reaped status so repeated close cannot signal a reused ID. Keep attached Codex sockets outside this path. Keep existing bounded detached-reader handling for a deliberately unowned process that retains a pipe.

- 2026-11-15, root: Make CLI session actions greet and submit exactly one retained operation through the existing pure client, with a bounded readiness wait instead of a terminal redraw timer. Preserve the managed launcher's build/credential checks. Keep lifecycle results separate from prompt text so pending/final acknowledgements never replace a newer composer.
- 2026-11-15, root: Keep owner replacement's stale-task guard. The existing reconnect test must explicitly choose a fresh task before Enter, rather than treating a retired opaque ID as authority for the replacement.

- 2026-11-15, root: Keep stopped owner joins off the manager, retain Live until both prior/final reports are applied, then free its native slot. Stop acknowledgement waits for matching durable metadata. Stop/reopen commands use the existing exact operation store; an old identical stop cannot reach a replacement. Reserve 16 ingress messages/64 KiB and 64 retained lifecycle results/512 KiB independently of ordinary and urgent native work. Preserve FIFO counter order.
- 2026-11-15, root: Return a full final snapshot for explicit stop, including retirement of all cards. Cancel old native/body reads and watches before replacement, retain session interests for closure/reopen facts, and join outstanding stop workers even when the daemon itself shuts down.

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

The stop/reopen milestone now passes all161 actual-daemon cases, including owned descendants, attached-server isolation, exact fresh-Claude restore targets, unconfirmed Grok startup,16-slot reuse, private drafts, durable save ordering and old-operation retries. Both Rust feature/lint matrices pass. The terminal/CLI cases passed their focused checks; the final complete release/terminal gate remains required after the rest of EP020. The owner-requested bloat review supports the shared architecture but identifies repeated panel drivers and command wrappers to examine before more clients consume them. Device credentials and the network listener remain unimplemented.


The accepted EP019b pair and measurements remain frozen for matched comparisons. The mid-EP review is recorded below and does not replace the required final whole-roadmap review.

The full lifecycle release gate now exits0, including all310 terminal workflows and both installed native contract suites. The earlier debug failure remains preserved and its corrected task-selection workflow now passes in release. Lifecycle work is ready for the next milestone: durable device credentials, then the optional listener. The review decisions preserve bounded native pages while requiring shared read drivers before the phone/web consumers.

The retry store now has the device boundary and separate local identity reserve, with direct checked privacy/revocation cases and a green default workspace/lint. No network peer or token record is active yet. Next add durable device records and typed local operator commands on the existing disk worker; then exercise these lease boundaries through real authenticated sockets.

The protocol and pure client now describe device access without platform I/O or extra command mirrors. All1642 default workspace tests and lint pass after the typed greeting change. These are data/API foundations; durable device records, manager operator dispatch and the listener still remain.

The catalogue now saves checked device verifiers and preserves all previous session metadata through schema version4. All1646 default workspace tests, strict lint and architecture/dependency checks pass. The next slice must connect those writes to the existing worker and retained local operator results, then create exclusive private token files through the CLI. The manager does not authenticate network devices yet.

The local RPC slice and its four actual-daemon cases pass. A held device write leaves a native prompt and private reads responsive; failed writes remain uncertain, and old provision retries do not regrant access. All1650 default workspace tests and strict lint pass after the final typed-credential/reserve changes. Next build the user-facing operator CLI on the shared pure client; then enable the optional authenticated WebSocket path.

## Owner-requested mid-EP bloat review

Canonical claude-fable-5-2, medium effort, read-only session6426dd2a-bdc0-44f8-b1d2-d3b087fe019c, reviewed HEAD6746fe8 and the pending backend-target fix. The complete output and tool metadata are .agents/reports/exec020-fable-bloat-review.md and .json. Its verdict is some avoidable bloat, medium confidence: about45 production files read, no direct main diff and no access to the temporary counts. It considers owner threads/mailboxes, numbered facts/Replica, private retries, bounded daemon I/O, stop/reopen and source cursors justified. Its principal suggestions are simpler panel paging, shared client read drivers and named payloads to remove command/reply rewrapping; dense terminal body retention and module fragmentation can wait. Directly verify each proposed simplification against existing list fidelity, stale-reader and private-result guarantees. This requested advisory review is not a final PASS or delegated approval.

### Full advisory reviewer output

> # Size and design review: `iphone-spike` at 6746fe8 plus the uncommitted resume-target fix
> 
> Model: `claude-fable-5-2`. I cannot see the configured effort level from inside the session. Read-only; no builds, tests or edits.
> 
> ## 1. Verdict
> 
> **Some avoidable bloat, concentrated in the native panel families (tasks, queue, settings, goals); the ownership architecture itself is proportionate.** Confidence: medium.
> 
> Evidence limits:
> - I read about 45 production files end to end across proto, client, daemon, Core engine and terminal, plus the plans and prior Fable reviews.
> - I could not diff against `main` (no git access) or open the `/tmp/carlos-ep020` count artifact (permission denied), so the scale figures below are sums of files I read, not measured deltas.
> - Most of Core `session/claude`, `session/codex` and the backend adapters is unread.
> 
> ## 2. Additions justified by the phone/web goal
> 
> - **Owner model in Core:** one thread per session, three mailboxes, and captured command handles so interrupt and approve bypass a waiting send (`engine/dispatch.rs:27-102`).
> - **Numbered events and replica:** stable entry ids, sequence ring, snapshot fallback, and `Replica`/codec/`Operations` in `carlos-client`.
> - **Private retry results:** `retry/mod.rs` is about 325 lines with three reserved classes; it is small for what it guarantees.
> - **Daemon plumbing:** bounded outbox and ingress, explicit `limits.rs`, and the SQLite catalogue.
> - **Stop/reopen:** `manager/stop.rs` (241 lines) and `stop_worker.rs` (62) are tight for the cases they cover; owned process families likewise.
> - **Source and output cursors:** needed because clients render at their own width.
> 
> ## 3. Biggest questionable areas, ranked by payoff
> 
> 1. **Panels use a paged, chunked, revision-hashed, per-reader-capability protocol where a capped snapshot would do.**
>    - A task label goes through a 2 KiB preview, then 64 KiB offset reads (`session/task_views.rs:103-163`), with reply validators in `carlos-client/src/tasks.rs:108-200`.
>    - The terminal then keeps cursor arrays and a "relative movement" resolver to keep a selection across 32-row pages (`remote/tasks_pages.rs:73-226`).
>    - Task windows add server-side captured windows (8 slots, 8 MiB) guarded by a per-socket `Arc<AtomicBool>` (`manager/task_windows.rs:14-64`, `session/task_windows.rs:37-92`). That guard only fences off the same user's other devices.
>    - Task lists and queues are human-scale. The existing 4,352-row ceiling at 1 KiB labels fits one 8 MiB frame.
>    - Simplification: one capped page per panel revision with a `truncated` flag, and a single shared `text/read {handle, offset}` for the rare long field. Boundedness is preserved.
>    - Scale: roughly 60 production files across five crates serve tasks and queue; the 13 I read total about 2.9k lines. Low thousands recoverable; I would not defend a tighter number.
> 
> 2. **Panel read drivers live in the terminal, duplicated per family, on top of a second request-id correlation.**
>    - `remote/queue_reads.rs:135-250` and `remote/tasks_reads.rs:129-315` are the same state machine: request, future, failure, revision reset, wait until `replica.sequence() >= page.seq`.
>    - `queue_pages.rs` and `tasks_pages.rs` are the same pager; `tasks_overview.rs:86-267` is a third variant.
>    - The client already maps id to `Kind` and validates (`pending.rs:179-341`, `client_replies.rs:61-120`). It then hands back `ReadResult{id}`, and the terminal matches the id again, with a 128-entry `retired` deque (`tasks_reads.rs:33-57`) and take-and-push-back routing (`queue_reads.rs:76-107`, `events.rs:55-60`).
>    - Simplification: one generic pager in `carlos-client` that owns outstanding reads; UIs read settled rows. This removes the duplicates now.
> 
> 3. **`Command`, `EngineCommand`, `EngineReply` and `ReadReply` are hand-copied mirrors.**
>    - `manager/mutation_work.rs` is 361 lines, about 90% field-for-field rewrap plus five size checks.
>    - The same pattern repeats in `manager/tasks.rs:131-259`, `read_commands.rs:34-164`, and 21 identical `self.reply` arms in `reads.rs:82-108`.
>    - Limit defaults and clamps are restated in three crates (`pending.rs:270`, `manager/tasks.rs:182`, `task_views.rs:48`).
>    - Core already depends on proto types throughout, so make each variant payload a named proto struct that `EngineCommand` carries, move size checks to one proto `validate()`, and let the owner reply with one serialisable value.
>    - Scale: about 500-700 lines, and a new command drops from roughly eight edit sites to three.
> 
> 4. **Bounded body conduit feeding an unbounded store (terminal only; lower confidence).**
>    - Client windows and pause/resume backpressure (`bodies.rs`, `remote/source_formatter.rs:65-78`, `transcript_formatter.rs:267-313`) bound transit.
>    - The formatter worker then keeps the whole original (`transcript_ansi_pages.rs:54-55`), which matches the recorded 238 MiB retention.
>    - That file's own doc comment still says "Prototype" (lines 1 and 37) while being production.
>    - The owner explicitly rejected a body cap, so I list this as a cost, not a cut.
> 
> 5. **Fragmentation driven by the caps.**
>    - `Device` is one 18-field object (`remote.rs:93-112`) whose `impl` is spread over about 70 files.
>    - Several files sit just under 400 lines (`command.rs` 399, `transcript_formatter.rs` 393).
>    - EP020's decision log moves `handle.rs` out "to preserve file size caps" (plan line 96).
>    - This costs navigation and per-file import and doc overhead more than logic. Do not fix it on its own; items 1-3 dissolve much of it.
> 
> ## 4. Sequencing
> 
> - **Before the WebSocket listener and Swift consumer make v1 sticky:** do item 3, ahead of EP020 milestone 3's device and operator commands. Decide item 1's wire shape, or mark the task-window, task-activity and overview families terminal-only. Move item 2's pager into `carlos-client`, or commit that the phone will not consume those panels.
> - **Safe to defer:** item 4, item 5, and the 8 ms poll.
> - **Do not rewrite:** owner threads and mailboxes, the retry store, outbox bounds, `Replica`, source cursors, stop/reopen, and process-family teardown.
> 
> ## 5. Will each new UI multiply code?
> 
> - **Transcript, approvals, send, interrupt, operations: no.** `Replica`, `Operations` and `Bodies` are genuinely shared.
> - **Panels: yes, as written.** The driver logic in item 2 sits in `src/app/remote`, not in `carlos-client`, which contradicts the "one copy" rule in `docs/engine-architecture.md`. A Swift client wanting queue or tasks would reimplement about 1.3k lines of subtle revision and sequence handling per panel pair.
> - Fixing items 1 and 2 before EP022 is what keeps the per-UI cost down to layout only.

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

Revision 2026-11-15: connect retained stop/reopen commands through Core, the pure client, manager workers and durable saves. Record both Rust matrices, strict lint,150 full daemon cases and two additional attached-server cases. Keep the lifecycle milestone open for helper/descendant, lost-reply and terminal/CLI proof.

Revision 2026-11-15: add CLI/terminal lifecycle controls and record full/focused checks without claiming a failed gate passed. Preserve changed-owner task guards and reproduce surviving owned descendants before repairing process-family teardown.

Revision 2026-11-15: prove and repair owned descendant cleanup without changing attached-server ownership or inherited unowned-pipe latency. Reproduce the fresh-Claude saved-versus-new target defect before changing its durable restore descriptor.
