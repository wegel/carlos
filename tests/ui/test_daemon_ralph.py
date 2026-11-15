"""Drive the main composer's Ralph key without bypassing socket readiness or private drafts."""

import asyncio
import socket
import sys

from test_daemon_client import daemon_support
from ralph_support import RalphFixture


class DaemonRalph(RalphFixture):
    def ralph_terminal(self, name):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture, first, other, session = self.ralph_session(name)
        path = fixture.root / "seed.md"
        terminal = self.attach_daemon(fixture, session, name, extra_args=[
            "--ralph-prompt", str(path), "--ralph-done-marker", "DONE_SPIKE"])
        terminal.contains("history-output 9999")
        return fixture, first, other, session, path, terminal

    @staticmethod
    def ralph_request(terminal, count):
        terminal.send(b"\x12")
        terminal.wait(lambda:len(terminal.requests("session/toggleRalph")) == count, "one explicit keyboard toggle")

    def test_daemon_main_ralph_key_defers_cancels_and_uses_cli_markers_without_clearing_input(self):
        fixture, first, other, session, path, terminal = self.ralph_terminal("ralph-main-ui")
        self.start_ralph_working(fixture, first, session)
        terminal.paste("private Ralph composer 日本語")
        terminal.wait(lambda:terminal.input_is("private Ralph composer 日本語"), "private draft before toggles")
        self.ralph_request(terminal, 1)
        terminal.contains("Ralph pending")
        self.assertFalse(self.ralph_snapshot(first, session)["info"]["ralph"])
        self.assertFalse(path.exists())
        self.ralph_request(terminal, 2)
        terminal.wait(lambda:"Ralph pending" not in terminal.text, "second keyboard request cancels the shared pending flag")
        self.ralph_request(terminal, 3)
        terminal.contains("Ralph pending")
        path.write_text("automatic")
        self.finish_ralph_native(fixture)
        terminal.contains("automatic result")
        terminal.contains("Ralph complete")
        terminal.wait(lambda:terminal.input_is("private Ralph composer 日本語"), "automatic work keeps the original composer")
        self.assertNotIn("Ralph pending", terminal.text)
        self.assertNotIn("RALPH MODE", terminal.text)
        self.assertEqual(fixture.ralph_texts, ["working", "automatic"])
        self.assertEqual([request["params"]["options"] for request in terminal.requests("session/toggleRalph")],
                         [{"prompt_path":str(path), "done_marker":"DONE_SPIKE"}] * 3)
        self.assertEqual(terminal.requests("session/setRalph"), [])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_daemon_ralph_key_during_a_held_resubscribe_keeps_the_draft_and_allocates_no_request(self):
        fixture, first, other, session, path, terminal = self.ralph_terminal("ralph-restoring-ui")
        path.write_text("working")
        terminal.paste("private draft through reconnect 🌍")
        terminal.wait(lambda:terminal.input_is("private draft through reconnect 🌍"), "draft before reconnect")
        recorder = terminal.daemon_recorder
        recorder.pause_subscriptions()
        self.addCleanup(recorder.resume_subscriptions)
        for peer in list(recorder.peers):
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        terminal.contains("F5 reconnects")
        terminal.send(b"\x1b[15~")
        terminal.wait(recorder.subscription_held.is_set, "same daemon holds the original fresh subscription")
        terminal.send(b"\x12")
        terminal.contains("Ralph request was not sent")
        self.assertEqual(terminal.requests("session/toggleRalph"), [])
        self.assertEqual(fixture.ralph_texts, [])
        self.assertTrue(terminal.input_is("private draft through reconnect 🌍"))
        recorder.resume_subscriptions()
        async def fence():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "item/agentMessage/delta", itemId="ralph-ready-fence", delta="Ralph ready fence")
        asyncio.run_coroutine_threadsafe(fence(), fixture.native.loop).result(timeout=3)
        terminal.contains("Ralph ready fence")
        self.ralph_request(terminal, 1)
        terminal.contains("RALPH MODE")
        self.assertEqual(fixture.ralph_texts, ["working"])
        self.assertTrue(terminal.input_is("private draft through reconnect 🌍"))
        self.assertEqual(terminal.requests("session/send"), [])
