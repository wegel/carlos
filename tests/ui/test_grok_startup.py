"""Keep a saved Grok transcript readable while native history and an early send wait."""


import json
import sys

from test_daemon_client import daemon_support


class GrokStartup:
    def test_grok_complete_native_replay_keeps_oldest_and_newest_rows_reachable(self):
        from test_startup_grok import StartupGrokTests
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture, client = StartupGrokTests._grok_saved_fixture(self, "terminal-complete-replay")
        (fixture.root / "grok-saved.json").write_text(json.dumps({"replay_turns":180}))
        operation = client.operation(1)
        session = client.call("session/open", operation=operation, backend="grok", cwd=str(fixture.root),
            target={"kind":"resume", "address":"terminal-native-replay"})["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        terminal = self.attach_daemon(fixture, session, "complete-grok-replay")
        terminal.contains("Grok answer 179 🌍")
        for count in range(1,4):
            terminal.send(b"\x1b[H")
            terminal.wait(lambda: len(terminal.requests("session/history")) == count, "one stored older history page")
            request = terminal.requests("session/history")[-1]["id"]
            terminal.wait(lambda: any(message.get("id") == request for message in terminal.messages("server")), "older history page replied")
        terminal.send(b"\x1b[H")
        terminal.contains("Grok user 0 🌍")
        terminal.snapshot("complete-grok-oldest-human")
        terminal.send(b"\x1b[F")
        terminal.contains("Grok answer 179 🌍")
        self.assertEqual(len(fixture.native_requests("session/load")),1)
        self.assertEqual(fixture.native_requests("session/prompt"), [])
        self.assertEqual(terminal.requests("session/send"), [])

    def test_grok_offered_permission_choices_work_through_the_terminal_without_a_prompt(self):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture("terminal-grok-permission", mode="grok-permission-number")
        self.addCleanup(fixture.close)
        client = daemon_support.Client(fixture, "permission-owner")
        self.addCleanup(client.close)
        operation = client.operation(1)
        session = client.call("session/create", operation=operation, backend="grok", cwd=str(fixture.root), name=None)["session"]
        client.outcome(operation)
        terminal = self.attach_daemon(fixture, session, "grok-permission")
        terminal.contains("Idle")
        terminal.paste("private composer stays")
        terminal.wait(lambda: terminal.input_is("private composer stays"), "private composer before permission")
        operation = client.operation(2)
        client.call("session/send", session=session, operation=operation, text="permission fixture")
        client.outcome(operation)
        terminal.contains("Choose Grok's permission")
        terminal.contains("Allow this command once")
        terminal.contains("Allow this command in future sessions")
        self.assertFalse((fixture.root / "permission-answers.jsonl").exists())
        terminal.snapshot("grok-original-offered-options")
        terminal.send(b"\x1b[B \t")
        terminal.contains("Review answers")
        terminal.send(b"\x13")
        terminal.contains("Permission fixture finished")
        terminal.wait(lambda: terminal.input_is("private composer stays"), "permission preserves private composer")
        answers = [json.loads(line) for line in (fixture.root / "permission-answers.jsonl").read_text().splitlines()]
        self.assertEqual(answers, [{"jsonrpc":"2.0", "id":91001, "result":{"outcome":{"outcome":"selected", "optionId":"native:always:β"}}}])
        self.assertEqual(len(terminal.requests("session/approve")), 1)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(len(fixture.native_requests("session/prompt")), 1)
        terminal.snapshot("grok-native-permission-chosen-without-an-extra-prompt")

    def test_grok_instant_answers_leave_each_follow_up_prompt_sendable(self):
        terminal = self.launch(backend="grok", resume=None)
        terminal.wait_ready_composer()
        texts = ["instant first", "instant second", "instant third"]
        for text in texts:
            terminal.paste(text)
            terminal.send(b"\r")
            terminal.contains("Answer: " + text)
        prompts = terminal.requests("session/prompt")
        self.assertEqual([p["params"]["prompt"][0]["text"] for p in prompts], texts)
        self.assertEqual(len(terminal.requests("session/new")), 1)
        self.assertNotIn("Grok closed", terminal.text)
        terminal.snapshot("three-instant-answers-and-no-stranded-follow-up")

    def test_grok_restores_more_than_one_native_queue_window_before_sending_queued_text(self):
        terminal = self.launch(backend="grok", resume="grok-saved")
        terminal.wait(lambda: terminal.requests("session/load"), "native restore waits at its gate")
        terminal.wait(lambda: terminal.input_is(""), "terminal accepts input while native restore waits")
        terminal.paste("after restore")
        terminal.wait(lambda: terminal.input_is("after restore"), "human can type while native restore waits")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.input_is(""), "human text is accepted into the pending queue")
        self.assertEqual(terminal.requests("session/prompt"), [])
        terminal.snapshot("human-input-queued-during-native-restore")
        (terminal.root / "release-history").write_text("go")
        terminal.contains("Answer: after restore")
        self.assertEqual(len(terminal.requests("session/load")), 1)
        prompts = terminal.requests("session/prompt")
        self.assertEqual(len(prompts), 1)
        self.assertEqual(prompts[0]["params"]["sessionId"], "grok-saved")
        self.assertEqual(prompts[0]["params"]["prompt"][0]["text"], "after restore")
        self.assertNotIn("Grok closed", terminal.text)
        terminal.snapshot("restored-history-and-one-human-turn")
