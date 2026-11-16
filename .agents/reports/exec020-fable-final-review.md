# EP020 final engineering review

**Verdict: PASS WITH ISSUES.** I found no blocker. One MAJOR defect needs repair before the phone is tested on a real link, and the unresolved 500 ms stdin event can stay a recorded risk if it gets stage timing.

## 1. What was reviewed

I read the source directly rather than the patch, for range `cc2a5f5..b851d73`:

- **Network path:** `transport/{websocket,websocket_poll,outbox,ingress,mod}.rs`, `network/{mod,handshake}.rs`, `acceptor.rs`, `server.rs`, `limits.rs`.
- **Devices:** `manager/devices.rs`, `registry_devices.rs`, `carlos-proto/src/devices.rs`, `device_cli.rs`, `operator_connection.rs`.
- **Stop/reopen and retry:** `manager/{mod,commands,stop,stop_worker,reopen}.rs`, `retry/{mod,clients,stopped}.rs`, `owned_child.rs`, `engine/stop_native.rs`.
- **Claude stdin writer:** `claude_backend/transport_writer.rs` and its caller in `transport.rs`.
- **Evidence:** the final report, the self-review, `docs/daemon-protocol.md` budgets and `websocket_write_tests.rs`.

The design matches the goal: one manager and owner model, WebSocket as a second socket worker on the same `Ingress`/`Outbox`, and no framework, runtime or broker.

## 2. Findings

### MAJOR 1 — WebSocket write deadline is per whole frame, not per progress

- **Where:** `crates/carlosd/src/transport/websocket.rs:194-198` sets one 500 ms deadline when a frame is taken; `:217-218` aborts unless the entire frame has left tungstenite's buffer by then.
- **Why it matters:** the kernel send buffer is pinned to 64 KiB (`network/handshake.rs:42`), while replies may be 8 MiB (`limits.rs:6`; cards, task windows and panels all allow 8 MiB). A healthy but slow reader is disconnected on any frame larger than roughly 64 KiB plus half a second of link throughput. At 10 Mbit/s that is about 700 KiB.
- **Failure shape:** deterministic. The client reconnects, asks again, gets the same frame and is dropped again, so that item is unreadable from the phone.
- **Test gap:** the evidence covers only stalled readers on loopback (`websocket_write_tests.rs`, `test_network_source.py:98`). No test has a slow reader that keeps making progress.
- **Does it block?** It does not block starting EP021. It does block the phone/web path in practice, so fix it before EP022 device testing.
- **Fix:** make the deadline progress-based. `HandshakeStream::write` (`handshake.rs:124-130`) already wraps every TCP write; count bytes there and re-arm the 500 ms stall deadline in `Worker::write` whenever the count advanced. The outbox charge (128 frames / 16 MiB, held until flush) remains the memory bound, so this needs no body cap, truncation or new machinery. Add one test with a throttled reader draining a multi-MiB frame, and correct the wording at `docs/daemon-protocol.md:123`.

### MAJOR 2 (non-blocking, recorded risk) — unresolved 500 ms Claude stdin timeout

- **Fail-closed holds:** `transport_writer.rs:71-75` reports "may already have been accepted"; `transport.rs:292-299` removes the pending entry and stops the emitter. There is no resend and no false acknowledgement.
- **Recommendation:** it may remain a recorded diagnostic risk; it does not need a speculative repair or a longer deadline. I would rank it above the self-review's MINOR, because each occurrence ends a live owner's transport and the cause is unknown.
- **Why the cause is unknowable today:** the single `recv_timeout` at `:72` covers queue wait behind up to eight requests, the guard check, a full JSON re-parse of the line (`:54`), the trace-file write under a mutex (`:55`), `write_all`, `flush` and the reply hop. The trace line proves only that the writer thread dequeued the frame.
- **Fix:** record monotonic timestamps at enqueue, dequeue, after trace and after write, and include them in the timeout error. Apply the same change to the near-identical `grok_backend/process_writer.rs:65-77`. This does not change the deadline. Do this early in EP021 so the next occurrence is attributable.
- **Related cheap fix:** `transport_writer.rs:54` re-parses every outgoing frame (up to 4 MiB) even when tracing is off. Gate it on `trace.is_some()`.

### MINOR 3 — command classification lives in three hand-kept lists

- **Where:** `transport/ingress.rs:44-67`, `manager/commands.rs:31-68`, and retry `Class` in mutation prep.
- **Risk:** the ingress default `_ => 2` silently admits any new mutation under the read allowance. EP021 adds Grok approvals and history commands, so this will be hit.
- **Fix:** one exhaustive `match` (no wildcard) returning the admission class, used by both ingress and routing.

### MINOR 4 — network ingress allowances are shared across devices

- **Where:** `ingress.rs:17,75` keeps one counter set for all network peers; a failed push closes the pushing socket (`websocket.rs:183-189`).
- **Risk:** one authenticated device flooding reads can get another device disconnected. Local operator access is correctly isolated. This is acceptable for one operator's own devices.
- **Fix:** document it, or stop reading from that peer instead of closing it.

### MINOR 5 — device identities share one 128-entry pool

- **Where:** `retry/clients.rs:89-108`.
- **Risk:** one provisioned device that greets fresh repeatedly can evict another device's inactive identity and its retained private results. It cannot read them.
- **Fix:** a small per-device cap (for example 8) in `make_room`.

### MINOR 6 — a failed network greeting keeps its slot for the full 5 s

- **Where:** `manager/commands.rs:113-118` replies but leaves the socket open; there are only four unauthenticated slots.
- **Fix:** close the peer after queuing the refusal.

### MINOR 7 — plain `ws://` on any specific unicast address is accepted silently

- **Where:** `server.rs:77-84` rejects only unspecified, multicast and broadcast addresses.
- **Risk:** the bearer token travels in the first clear-text frame, and the loopback/WireGuard rule exists only in docs.
- **Fix:** print a one-line startup warning for non-loopback binds. No policy engine is needed.

### MINOR 8 — avoidable repetition

- `Manager::accept` and `accept_network` (`manager/mod.rs:157-195`) differ by one predicate and one constructor.
- The Claude and Grok stdin writers are near copies.
- The terminal panel read/paging repetition is already assigned to the start of EP021; I agree it must land before Swift or web consumes panels.

## 3. What I checked and found sound

- **Outbox accounting:** a frame stays charged through real flush or teardown (`websocket.rs:222-224,244-246`), and `close` subtracts only queued frames, so there is no underflow.
- **No idle spin:** the worker blocks on TCP readiness plus a wake socket with deadline-only timeouts (`websocket_poll.rs`).
- **Admission:** counters are released exactly once via `Drop` (`network/mod.rs:73-80`).
- **Pre-auth limits:** the 4 KiB message cap is raised only after the manager authorises, and a second pre-auth frame closes the socket.
- **Origin rule:** it rejects browsers unless the origin matches exactly, which also covers DNS rebinding.
- **Credentials:** token comparison goes through `blake3::Hash` equality (constant-time), and only verifiers are stored. The token file is created `O_EXCL|O_NOFOLLOW` with mode 0600 and synced before the grant.
- **Revocation:** it blocks immediately and stays blocked if the save is uncertain.
- **Stop/reopen:** stop joins off the manager thread and acknowledges only after the durable save. Reopen refuses without a confirmed native address. Unconsumed commands become private Uncertain with their exact text.
- **Process ownership:** `OwnedChild` kills the group before reaping the leader.

## 4. Costs: fixed, necessary, avoidable

- **Measured fixed costs (accept and keep visible):** about +1 ms cold replay, about +0.8 MiB RSS, +2.6 MB daemon binary, about +0.14 ms WebSocket admission median, and about 11% on source transfer.
- **Necessary safety bounds:** the 4/16 admission split, the 5 s login window, 8 KiB HTTP and 4 KiB hello, charged writes, separate local allowances, and the retry windows.
- **Avoidable:** findings 3 and 8, and the unconditional trace re-parse.

## 5. Review limits

- This was read-only; I ran no builds, tests or measurements and took the gate results and numbers from the reports.
- I did not read the full patch, the terminal `src/app/remote/*` changes, the Claude `reopen`/`startup` and Grok changes, the registry schema migration, or the Python tests beyond a targeted grep.
- I did not open the tungstenite source; the buffering reasoning in MAJOR 1 rests on its documented behaviour.
- I inferred the 8 MiB frame producers from constants and did not trace them through each call site.
- The link-speed arithmetic in MAJOR 1 is derived from the code, not measured.

## 6. Verdict

**PASS WITH ISSUES.** EP021 may start. Fix MAJOR 1 before any real-link phone or web testing in EP022. Land the stage timing from MAJOR 2 and the single classification table (finding 3) at the start of EP021, alongside the already-required panel sharing.
