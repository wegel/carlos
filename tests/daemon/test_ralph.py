"""Preserve deferred Ralph toggles and prompt choices through the actual daemon."""

import argparse
import asyncio
import errno
import os
from pathlib import Path
import unittest

import support
from support import wait_for
from ralph_support import RalphFixture


class RalphTests(RalphFixture):
    def test_explicit_blocked_and_waiting_marker_choices_stop_the_automatic_driver(self):
        for marker, notice in (("BLOCK_SPIKE", "Ralph blocked"), ("WAIT_SPIKE", "Ralph waiting for human")):
            with self.subTest(marker=marker):
                fixture, first, other, session = self.ralph_session("ralph-marker-" + marker)
                fixture.ralph_marker = marker
                (fixture.root / "seed.md").write_text("automatic")
                operation = first.operation(2)
                first.call("session/setRalph", session=session, operation=operation,
                           on=True, options=self.ralph_options())
                self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
                wait_for(lambda:any(notice in entry["text"] for entry in self.ralph_snapshot(first, session)["entries"]), "explicit custom marker stops the automatic driver")
                self.assertFalse(self.ralph_snapshot(other, session)["info"]["ralph"])
                self.assertEqual(fixture.ralph_texts, ["automatic"])

    def test_deferred_toggles_cancel_without_reading_a_missing_prompt_or_sending_automatic_input(self):
        fixture, first, other, session = self.ralph_session("ralph-toggle-cancel")
        self.start_ralph_working(fixture, first, session)
        on = first.operation(3)
        first.call("session/toggleRalph", session=session, operation=on, options=self.ralph_options())
        self.assertIn("pending", first.outcome(on)["state"]["notice"])
        self.assertFalse(self.ralph_snapshot(other, session)["info"]["ralph"])
        cancel = other.operation(1)
        other.call("session/toggleRalph", session=session, operation=cancel)
        self.assertIn("canceled", other.outcome(cancel)["state"]["notice"])
        self.finish_ralph_native(fixture)
        wait_for(lambda:self.ralph_snapshot(first, session)["info"]["current_turn"] is None, "original turn completes")
        fence = first.operation(4)
        first.call("session/send", session=session, operation=fence, text="fence")
        first.outcome(fence)
        page = wait_for(lambda:(page if any("Answer: fence" in entry["text"] for entry in page["entries"]) else None)
                        if (page := self.ralph_snapshot(first, session)) else None, "ordinary fence after canceled toggle")
        self.assertFalse(page["info"]["ralph"])
        self.assertEqual(fixture.ralph_texts, ["working", "fence"])
        self.assertFalse(any("Ralph could not start" in entry["text"] for entry in page["entries"]))
        self.assertFalse((fixture.root / "seed.md").exists())

    def test_deferred_enable_reads_original_options_after_turn_end_and_old_retry_never_toggles_again(self):
        fixture, first, other, session = self.ralph_session("ralph-toggle-enable")
        self.start_ralph_working(fixture, first, session)
        operation = first.operation(3)
        params = dict(session=session, operation=operation, options=self.ralph_options())
        first.call("session/toggleRalph", **params)
        accepted = first.outcome(operation)
        self.assertEqual(accepted["state"]["kind"], "accepted")
        self.assertFalse((fixture.root / "seed.md").exists())
        (fixture.root / "seed.md").write_text("automatic")
        self.finish_ralph_native(fixture)
        wait_for(lambda:any("Ralph complete" in entry["text"] for entry in self.ralph_snapshot(first, session)["entries"]), "custom done marker ends the single automatic turn")
        self.assertEqual(fixture.ralph_texts, ["working", "automatic"])
        self.assertFalse(self.ralph_snapshot(other, session)["info"]["ralph"])
        (fixture.root / "seed.md").write_text("must not run after old retry")
        self.assertEqual(first.call("session/toggleRalph", **params), accepted)
        response = other.response(other.send("operation/read", operation=operation))
        self.assertEqual(response["error"]["code"], -32003)
        self.assertEqual(fixture.ralph_texts, ["working", "automatic"])
        self.assertFalse((Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json").exists())

    def test_deferred_off_keeps_a_queued_human_prompt_and_discards_automatic_continuation(self):
        fixture, first, other, session = self.ralph_session("ralph-toggle-off")
        (fixture.root / "seed.md").write_text("working")
        enable = first.operation(2)
        first.call("session/setRalph", session=session, operation=enable, on=True, options=self.ralph_options())
        self.assertEqual(first.outcome(enable)["state"]["kind"], "accepted")
        wait_for(lambda:fixture.ralph_texts == ["working"], "initial automatic prompt running")
        stop = first.operation(3)
        first.call("session/toggleRalph", session=session, operation=stop)
        self.assertIn("pending", first.outcome(stop)["state"]["notice"])
        human = other.operation(1)
        other.call("session/send", session=session, operation=human, text="human after off", policy="queue")
        self.finish_ralph_native(fixture)
        self.assertEqual(other.outcome(human)["state"]["kind"], "accepted")
        wait_for(lambda:any("Answer: human after off" in entry["text"] for entry in self.ralph_snapshot(other, session)["entries"]), "accepted human prompt survives turning off Ralph")
        self.assertEqual(fixture.ralph_texts, ["working", "human after off"])
        self.assertFalse(self.ralph_snapshot(first, session)["info"]["ralph"])

    def test_deferred_prompt_file_wait_keeps_reads_and_native_approval_controls_responsive(self):
        fixture, first, other, session = self.ralph_session("ralph-toggle-file")
        path = fixture.root / "seed.fifo"
        os.mkfifo(path)
        self.addCleanup(path.unlink, missing_ok=True)
        self.start_ralph_working(fixture, first, session)
        toggle = first.operation(3)
        first.call("session/toggleRalph", session=session, operation=toggle, options=self.ralph_options(path.name))
        self.assertEqual(first.outcome(toggle)["state"]["kind"], "accepted")
        self.finish_ralph_native(fixture)
        def writer():
            try:
                return os.open(path, os.O_WRONLY | os.O_NONBLOCK)
            except OSError as error:
                if error.errno != errno.ENXIO:
                    raise
        descriptor = wait_for(writer, "deferred config opens its FIFO on a worker")
        with os.fdopen(descriptor, "wb", buffering=0) as stream:
            self.assertFalse(self.ralph_snapshot(other, session)["info"]["ralph"])
            async def approval():
                for link in list(fixture.native.connections):
                    await fixture.native.notify(link, "turn/started", turn={"id":"urgent"})
                    await fixture.native.send(link, {"id":"card", "method":"item/commandExecution/requestApproval", "params":{
                        "threadId":"native-root", "turnId":"urgent", "itemId":"urgent-tool", "command":"true"}})
            asyncio.run_coroutine_threadsafe(approval(), fixture.native.loop).result(timeout=3)
            card = wait_for(lambda:(cards[0] if cards else None)
                            if (cards := self.ralph_snapshot(other, session)["approvals"]) is not None else None, "native card during config file wait")
            answer = other.operation(1)
            other.call("session/approve", session=session, operation=answer, approval=card["id"],
                       answer={"kind":"decision", "decision":"once"})
            self.assertEqual(other.outcome(answer)["state"]["kind"], "accepted")
            self.assertEqual(fixture.native.answers, 1)
            stream.write(b"automatic")
        wait_for(lambda:self.ralph_snapshot(first, session)["info"]["ralph"], "same config read finishes while urgent native turn stays active")
        self.assertEqual(fixture.ralph_texts, ["working"])
        self.finish_ralph_native(fixture, "urgent")
        wait_for(lambda:any("Ralph complete" in entry["text"] for entry in self.ralph_snapshot(first, session)["entries"]), "single deferred seed runs after the native control completes")
        self.assertEqual(fixture.ralph_texts, ["working", "automatic"])


if __name__ == "__main__":
    class StandaloneRalphTests(RalphTests, unittest.TestCase):
        pass

    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY, support.ARTIFACTS = args.binary.resolve(), args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
