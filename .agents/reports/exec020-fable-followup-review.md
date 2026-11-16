# EP020 follow-up review: the two final corrections

**Verdict: PASS WITH ISSUES.** Both corrections are sound and sufficient to close EP020 and proceed to the specified EP021 cleanup. There are no BLOCKER or MAJOR findings; the three MINORs below do not gate closure.

## Scope and limits

- **Reviewed:** `target/ep020-review/corrections.patch` (b851d73..272fae3), plus the current `transport/websocket.rs`, `transport/websocket_poll.rs`, `network/handshake.rs`, `websocket_write_tests.rs`, `grok_backend/process_writer.rs`, `trace_frame` in `claude_backend/transport.rs`, and the daemon slow-reader test in `tests/daemon/test_network_source.py`.
- **Not run:** I executed nothing. Test results, gate 04 and the measurements are taken from your report as stated, not reproduced.
- **Not read:** tungstenite's source. My reasoning assumes its `write` and `read` can themselves push buffered bytes to the stream; the code handles that case correctly either way.
- **Not evaluated:** any minimum-rate or total-duration cap, as instructed. The documented consequence is that a reader draining at a trickle keeps its socket and its charge for as long as it trickles. Memory stays bounded by the per-socket outbox limit.

## Correction 1 (3c83655): WebSocket no-progress deadline

Deadline semantics and accounting are correct.

- **Progress counter:** `written` advances only on a successful `TcpStream::write`. Any re-arm therefore reflects real bytes accepted by the kernel, including pong or close bytes, which are queued in order behind the frame.
- **Baseline:** `flushing()` captures the baseline when it arms, so handshake bytes never count as progress.
- **Check order:** `write()` checks progress before testing the deadline and again after `flush()`. Bytes pushed during `socket.write` or `socket.read` are credited before expiry is judged.
- **Stalled peer:** the counter stays unchanged, poll wakes at the deadline plus 1 ms, and the next `write()` returns `TimedOut`. I see no spin and no periodic timer.
- **Charge:** `pending.frame` is released only on `Ok(())` from flush or in `Drop`. Re-arming cannot release the charge early or twice.
- **Login deadline:** the absolute check in `run()` is untouched and still enters the poll timeout as the minimum of the two deadlines. Write progress cannot extend an unauthenticated socket.
- **Wrap:** the `!=` comparison on a wrapping counter is safe.
- **Paced test:** 64 reads of at most 16 KiB at 40 ms each force about 2.5 s of delivery. The zero-charge assertion is not racy in practice, because the server's flush completes when the tail enters kernel buffers, well before the client's last read.
- **Docs:** both protocol documents match the behaviour.

## Correction 2 (272fae3): writer stage timing

The timing record and both writers are correct.

- **Record consistency:** one writer thread stores marks in order, and `Display` loads them latest-first. A snapshot can never show a later stage without its predecessors.
- **Pending encoding:** the `micros + 1` encoding with a saturating clamp is right.
- **No payloads:** the output is numeric only.
- **Grok snapshot:** the string is built before `close()`, so the kill and resulting EPIPE cannot rewrite the observed stage.
- **Claude snapshot:** it is taken immediately after the timeout, with no intervening cleanup.
- **Deadlines and resend:** `WRITE_TIMEOUT` is 500 ms in both writers, and neither path resends.
- **Trace parse:** making the Claude re-parse conditional on `trace.is_some()` preserves behaviour, since `trace_frame` already did nothing for `None`.
- **Queue wait:** the snapshot shows `dequeued=pending` when the 500 ms was spent behind earlier jobs. That is the distinction the old intermittent event lacked, and the change claims no more than that.
- **Grok blocked-pipe test:** it asserts ready, written-pending, redaction and bounded reap against a real pipe.

## Findings

1. **MINOR: no test for a stall after partial progress.** No test covers a WebSocket reader that drains some bytes and then stops, which is the one new state the re-arm introduces.
   - The Rust blocked-writer test tears down by `drop(peer)`, not by deadline expiry.
   - The daemon slow-reader test proves slot release within 3 s for a reader that never drains.
   - The logic is correct by inspection.
   - Guidance: in EP021, extend the paced fixture to stop reading after a few chunks and assert the worker closes about 500 ms after the last progress, with the charge released.

2. **MINOR: `Stage::Written` is marked on error as well as success.** It records write-return, not success.
   - It only surfaces in the narrow race where the writer returns just as the waiter times out.
   - The snapshot could then read `written+…us` beside "did not finish", or follow a failed write.
   - Guidance: rename the printed label to `returned`, or note it in the message as the Claude path does with "ready is after trace".

3. **MINOR: the new Grok test depends on `python3` and a wall-clock bound.** Its `< 4 s` bound and the paced test's 40 ms sleeps are timing-sensitive under heavy load.
   - The margins are wide (about 3.5 s, and roughly 300 ms or more of slack per re-arm).
   - Guidance: no action unless it flakes.
