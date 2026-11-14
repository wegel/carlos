"""Check invalid frames, lost native work and congested peers at the actual process boundary."""

import select
import socket

from support import Client, Fixture, alive, wait_for


class FailureTests:
    def test_explicit_shutdown_stops_owned_native_children_after_clients_detach(self):
        fixture = Fixture("owned-stop", mode="codex-current")
        self.addCleanup(fixture.close)
        client = Client(fixture, "launcher")
        self.addCleanup(client.close)
        for counter, backend in enumerate(("codex", "grok", "claude"), 1):
            operation = client.operation(counter)
            client.call("session/create", operation=operation, backend=backend, cwd=str(fixture.root), name=None)
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        children = [row for row in fixture.pids() if alive(row["pid"])]
        self.assertEqual({row["backend"] for row in children}, {"codex", "grok", "claude"})
        client.close()
        self.assertTrue(all(alive(row["pid"]) for row in children))
        fixture.stop()
        wait_for(lambda: all(not alive(row["pid"]) for row in children), "all owned native children stopped and were reaped")

    def test_shutdown_cancels_a_native_initialize_before_the_launch_factory_returns(self):
        fixture = Fixture("cancel-launch", mode="grok-held-initialize")
        self.addCleanup(fixture.close)
        client = Client(fixture, "launcher")
        self.addCleanup(client.close)
        operation = client.operation(1)
        client.call("session/create", operation=operation, backend="grok", cwd=str(fixture.root), name=None)
        wait_for(lambda: (fixture.root / "initialize-held").exists(), "real Grok adapter waits for native initialize")
        self.assertEqual(client.call("operation/read", operation=operation)["state"]["kind"], "pending")
        child = fixture.pids()[0]["pid"]
        fixture.stop()
        self.assertFalse(alive(child))

    def test_invalid_envelopes_credentials_and_foreign_targets_do_not_reach_a_backend(self):
        fixture = Fixture("invalid-frames")
        self.addCleanup(fixture.close)
        first = Client(fixture, "not-greeted", greet=False)
        self.addCleanup(first.close)
        self.assertEqual(first.response(first.send("sessions/watch"))["error"]["code"], -32002)
        first.socket.sendall(b"{\n")
        self.assertEqual(first.receive()["error"]["code"], -32700)
        self.assertEqual(first.response(first.send("native/call", methodName="turn/start"))["error"]["code"], -32601)
        self.assertEqual(first.response(first.send("hello", protocol=999, client="bad-version"))["error"]["code"], -32001)
        first.hello = first.call("hello", protocol=1, client="fixed-version")
        self.assertEqual(first.response(first.send("hello", protocol=1, client="twice"))["error"]["code"], -32002)
        foreign = "f"*32
        self.assertEqual(first.response(first.send("session/subscribe", session=foreign, since=None))["error"]["code"], -32004)
        second = Client(fixture, "bad-secret", greet=False)
        self.addCleanup(second.close)
        reply = second.response(second.send("hello", protocol=1, client="bad-secret", resume=first.hello["client"], resume_token="0"*64, instance=first.hello["instance"]))
        self.assertEqual(reply["error"]["code"], -32003)
        self.assertEqual(fixture.native_requests("initialize"), [])
        oversized = Client(fixture, "oversized", greet=False)
        self.addCleanup(oversized.close)
        try:
            oversized.socket.sendall(b"x"*(4*1024*1024+1))
        except (BrokenPipeError, ConnectionResetError):
            pass
        self.assertTrue(select.select([oversized.socket], [], [], 2)[0])
        try:
            self.assertEqual(oversized.socket.recv(1), b"")
        except ConnectionResetError:
            pass
        self.assertEqual(first.call("sessions/watch")["sessions"], [])

    def test_native_eof_returns_pending_text_privately_and_keeps_another_session_live(self):
        fixture = Fixture("native-eof", codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "first")
        second = Client(fixture, "second")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        pending = first.operation(2)
        first.call("session/send", session=session, operation=pending, text="held")
        wait_for(lambda: any(request.get("method") == "turn/start" for request in fixture.native.requests), "native send is in flight")
        fixture.native.disconnect()
        outcome = first.outcome(pending)
        self.assertEqual(outcome["state"]["kind"], "uncertain")
        self.assertEqual(outcome["state"]["recovery"]["text"], "held")
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(snapshot["info"]["availability"], "closed")
        another = second.operation(1)
        other_session = second.call("session/create", operation=another, backend="grok", cwd=str(fixture.root), name=None)["session"]
        self.assertNotEqual(session, other_session)
        self.assertEqual(second.outcome(another)["state"]["kind"], "accepted")
        self.assertEqual(sum(request.get("method") == "thread/start" for request in fixture.native.requests), 1)

    def test_a_slow_expanded_peer_cannot_stop_the_owner_or_a_collapsed_peer(self):
        fixture = Fixture("slow-peer", codex=True)
        self.addCleanup(fixture.close)
        first = Client(fixture, "slow")
        second = Client(fixture, "collapsed")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        first.call("session/subscribe", session=session, since=None)
        second.call("session/subscribe", session=session, since=None)
        send = second.operation(1)
        second.call("session/send", session=session, operation=send, text="flood-tool")
        tool = first.message(lambda m: m.get("method") == "session/event" and m["params"]["event"]["kind"] == "entry_added" and m["params"]["event"]["entry"].get("tool"))["params"]["event"]["entry"]["id"]
        first.call("entry/output/subscribe", session=session, entry=tool, cursor=None)
        # This socket stops reading while its peer continues to collect the native result.
        self.assertEqual(second.outcome(send)["state"]["kind"], "accepted")
        state = second.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertEqual(state["info"]["availability"], "ready")
        self.assertFalse(any(m.get("method") == "entry/output" for m in second.messages))
        deadline = select.poll()
        deadline.register(first.socket, select.POLLHUP | select.POLLRDHUP)
        deadline.poll(1500)
        # Drain what was already written before the deadline, then observe explicit reset or EOF.
        reset = False
        try:
            while True:
                message = first.receive(timeout=3)
                if message.get("method") == "entry/output/reset":
                    reset = True
                    break
        except (AssertionError, ConnectionResetError):
            self.assertTrue(select.select([first.socket], [], [], 2)[0], "slow peer must reset its watch or disconnect")
        self.assertTrue(reset or first.socket.recv(1) == b"")
