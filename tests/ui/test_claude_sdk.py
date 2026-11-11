"""Check the actual Rust UI, compiled SDK helper, and deterministic native CLI together."""

import json
import time
import uuid


class ClaudeSdk:
    def test_claude_context_keeps_known_busy_counts_and_recovers_from_optional_failure(self):
        terminal = self.launch(mode="claude-context-retry",backend="claude",resume=None,width=100)
        self.claude_ready(terminal)
        terminal.contains("32k/123k")
        terminal.paste("hold this turn")
        terminal.send(b"\r")
        terminal.contains("Answer: hold this turn")
        terminal.contains("~32k/123k")
        terminal.paste("unsent 日本語")
        terminal.wait(lambda: terminal.input_is("unsent 日本語"),"typing while optional context failed")
        terminal.resize(36,12)
        terminal.contains("~32k/123k")
        terminal.contains("42k/123k")
        terminal.wait(lambda: "~42k" not in terminal.text,"successful retry renders fresh counts")
        self.assertEqual(len(self.helper_requests(terminal,"prompt")),1)
        self.assertEqual(len([frame for frame in terminal.messages() if frame.get("event") == "launch"]),1)
        terminal.snapshot("context-busy-retry-phone")

    def helper_messages(self, terminal, direction="client"):
        path = terminal.root / "claude-controller.jsonl"
        if not path.exists():
            return []
        return [record["message"] for line in path.read_bytes().splitlines(keepends=True) if line.endswith(b"\n")
                if (record := json.loads(line))["direction"] == direction]

    def helper_requests(self, terminal, method):
        return [message for message in self.helper_messages(terminal) if message.get("method") == method]

    def native_controls(self, terminal, subtype):
        return [message for message in terminal.messages() if message.get("request", {}).get("subtype") == subtype]

    def claude_ready(self, terminal):
        terminal.contains("32k/123k")
        terminal.contains("26%")

    def test_claude_startup_permission_can_be_answered_before_initialize_reply(self):
        terminal = self.launch(mode="claude-startup-question", backend="claude", resume=None)
        terminal.contains("Allow startup hook?")
        replies = self.helper_messages(terminal, "server")
        self.assertFalse(any(frame.get("type") == "reply" and frame.get("ok") is True for frame in replies))
        terminal.snapshot("startup-question")
        terminal.send(b"n")
        self.claude_ready(terminal)
        answers = self.helper_requests(terminal, "callback.respond")
        self.assertEqual(answers[0]["params"]["callbackId"], "startup-question")
        self.assertEqual(answers[0]["params"]["answer"]["behavior"], "deny")
        native = [frame for frame in terminal.messages() if frame.get("type") == "control_response"]
        self.assertEqual(native[0]["response"]["request_id"], "startup-question")
        terminal.paste("after startup")
        terminal.send(b"\r")
        terminal.contains("Answer: after startup")

    def test_claude_native_default_to_no_requires_two_explicit_keys(self):
        terminal = self.launch(mode="claude-startup-confirm", backend="claude", resume=None)
        terminal.contains("Press y twice to allow")
        terminal.send(b"y")
        terminal.contains("Press y again to allow")
        self.assertEqual(self.helper_requests(terminal, "callback.respond"), [])
        terminal.send(b"y")
        self.claude_ready(terminal)
        answer = self.helper_requests(terminal, "callback.respond")[0]
        self.assertEqual(answer["params"]["answer"]["behavior"], "allow")

    def test_claude_uuid_origin_priorities_and_both_result_idle_orders(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        for text in ["hold this turn", "steer this", "after steer"]:
            terminal.paste(text)
            terminal.send(b"\r")
            terminal.contains("Answer: " + text)
            if text == "steer this":
                terminal.wait(lambda: len([m for m in terminal.messages("server") if m.get("type") == "result"]) == 2,
                              "two native result cycles")
                for _ in range(4):
                    terminal.pump(.05)
        prompts = self.helper_requests(terminal, "prompt")
        self.assertEqual([prompt["params"]["priority"] for prompt in prompts], ["later", "now", "later"])
        ids = [str(uuid.UUID(prompt["params"]["uuid"])) for prompt in prompts]
        self.assertEqual(len(set(ids)), 3)
        self.assertTrue(all(prompt["params"]["origin"] == {"kind": "human"} for prompt in prompts))
        native_prompts = [frame for frame in terminal.messages() if frame.get("type") == "user"]
        self.assertEqual([prompt["uuid"] for prompt in native_prompts], ids)
        self.assertEqual(len([frame for frame in terminal.messages() if frame.get("event") == "launch"]), 1)
        terminal.snapshot("steered-and-completed")

    def test_claude_discovers_models_context_and_changes_live_settings_without_restart(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("hold this turn")
        terminal.send(b"\r")
        terminal.contains("Answer: hold this turn")
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.contains("sdk-sonnet")
        terminal.send(b"\x1b[C\x1b[B\x1b[C\r")
        terminal.wait(lambda: bool(self.native_controls(terminal, "set_model")), "live native model change")
        terminal.wait(lambda: bool(self.native_controls(terminal, "apply_flag_settings")), "live native effort change")
        self.assertEqual(self.helper_requests(terminal, "model.set")[-1]["params"]["model"], "sdk-opus")
        self.assertEqual(self.helper_requests(terminal, "effort.set")[-1]["params"]["effort"], "high")
        self.assertEqual(self.native_controls(terminal, "set_model")[-1]["request"]["model"], "sdk-opus")
        self.assertEqual(len([frame for frame in terminal.messages() if frame.get("event") == "launch"]), 1)
        self.assertEqual(len([frame for frame in terminal.messages() if frame.get("type") == "user"]), 1)
        terminal.snapshot("live-settings")

    def test_claude_keeps_saved_model_and_effort_over_native_startup_defaults(self):
        root = self.summary_root()
        settings = root / "config/carlos/runtime-defaults.json"
        settings.parent.mkdir(parents=True, exist_ok=True)
        settings.write_text(json.dumps({"model": "sdk-opus", "effort": "max"}))
        terminal = self.launch(backend="claude", resume=None, directory=root)
        self.claude_ready(terminal)
        terminal.contains("sdk-opus/max")
        terminal.paste("use saved model")
        terminal.send(b"\r")
        terminal.contains("Answer: use saved model")
        controls = self.native_controls(terminal, "set_model")
        self.assertEqual([control["request"]["model"] for control in controls], ["sdk-opus"])
        self.assertEqual(self.native_controls(terminal, "apply_flag_settings")[0]["request"]["settings"]["effortLevel"], "max")
        native = terminal.messages()
        model_index = next(i for i, message in enumerate(native) if message.get("request", {}).get("subtype") == "set_model")
        prompt_index = next(i for i, message in enumerate(native) if message.get("type") == "user")
        self.assertLess(model_index, prompt_index)
        self.assertEqual(len([message for message in native if message.get("event") == "launch"]), 1)
        self.assertEqual(self.helper_requests(terminal, "model.set")[0]["params"]["model"], "sdk-opus")
        terminal.snapshot("saved-model-preserved")

    def test_claude_discards_a_saved_other_backend_model_and_uses_native_default(self):
        root = self.summary_root()
        settings = root / "config/carlos/runtime-defaults.json"
        settings.parent.mkdir(parents=True, exist_ok=True)
        settings.write_text(json.dumps({"model": "gpt-5.5", "effort": "xhigh"}))
        terminal = self.launch(backend="claude", resume=None, directory=root)
        self.claude_ready(terminal)
        terminal.paste("use native default")
        terminal.send(b"\r")
        terminal.contains("Answer: use native default")
        self.assertNotIn("gpt-5.5", terminal.text)
        self.assertTrue(all(control["request"]["model"] != "gpt-5.5" for control in self.native_controls(terminal, "set_model")))
        self.assertEqual(self.helper_requests(terminal, "effort.set"), [])
        self.assertEqual(self.helper_requests(terminal, "model.set"), [])
        self.assertEqual(len([message for message in terminal.messages() if message.get("event") == "launch"]), 1)
        terminal.snapshot("other-backend-default-discarded")

    def test_claude_appearance_save_does_not_pin_inherited_native_model_or_effort(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.send(b"\x1b[Z\x1b[C\r")
        terminal.wait(lambda: "Settings" not in terminal.text, "appearance saved")
        defaults = json.loads((terminal.root / "config/carlos/runtime-defaults.json").read_text())
        self.assertIsNone(defaults["model"])
        self.assertIsNone(defaults["effort"])
        terminal.paste("keep native defaults")
        terminal.send(b"\r")
        terminal.contains("Answer: keep native defaults")
        self.assertEqual(self.helper_requests(terminal, "model.set"), [])
        self.assertEqual(self.helper_requests(terminal, "effort.set"), [])
        terminal.snapshot("native-defaults-inherited")

    def test_claude_failed_ralph_turn_stops_without_an_automatic_retry(self):
        terminal = self.launch(mode="claude-ralph-failure", backend="claude", resume=None)
        self.claude_ready(terminal)
        prompt = terminal.root / ".agents/ralph-prompt.md"
        prompt.parent.mkdir(exist_ok=True)
        prompt.write_text("Check the deterministic fixture.")
        terminal.send(b"\x12")
        terminal.contains("Fixture model failure")
        deadline = time.monotonic() + 1.3
        while time.monotonic() < deadline:
            terminal.pump(.05)
        prompts = self.helper_requests(terminal, "prompt")
        self.assertEqual(len(prompts), 1)
        self.assertEqual(prompts[0]["params"]["origin"], {"kind": "auto-continuation"})
        terminal.paste("human retry")
        terminal.send(b"\r")
        terminal.wait(lambda: len(self.helper_requests(terminal, "prompt")) == 2, "human retry after failed turn")
        self.assertEqual(self.helper_requests(terminal, "prompt")[-1]["params"]["priority"], "later")
        self.assertEqual(self.helper_requests(terminal, "prompt")[-1]["params"]["origin"], {"kind": "human"})
        terminal.snapshot("ralph-failed")

    def test_claude_ralph_followup_keeps_automatic_origin(self):
        terminal = self.launch(mode="claude-ralph-followup", backend="claude", resume=None)
        self.claude_ready(terminal)
        prompt = terminal.root / ".agents/ralph-prompt.md"
        prompt.parent.mkdir(exist_ok=True)
        prompt.write_text("Start the deterministic fixture.")
        terminal.send(b"\x12")
        terminal.wait(lambda: len(self.helper_requests(terminal, "prompt")) >= 2, "Ralph follow-up")
        terminal.contains("@@AWAITING_HUMAN@@")
        prompts = self.helper_requests(terminal, "prompt")
        self.assertEqual(len(prompts), 2)
        self.assertTrue(all(p["params"]["origin"] == {"kind": "auto-continuation"} for p in prompts))
        self.assertTrue(all(p["params"]["priority"] == "later" for p in prompts))

    def test_claude_coalesced_result_finishes_both_prompt_uuids(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        for text in ["hold this turn", "merge this"]:
            terminal.paste(text)
            terminal.send(b"\r")
            terminal.contains("Answer: " + text)
        terminal.wait(lambda: any(m.get("type") == "result" for m in terminal.messages("server")), "coalesced result")
        prompts = self.helper_requests(terminal, "prompt")
        result = next(m for m in terminal.messages("server") if m.get("type") == "result")
        self.assertEqual(result["user_message_uuids"], [prompt["params"]["uuid"] for prompt in prompts])
        for _ in range(3):
            terminal.pump(.05)
        terminal.paste("after merged turn")
        terminal.send(b"\r")
        terminal.contains("Answer: after merged turn")
        self.assertEqual(self.helper_requests(terminal, "prompt")[-1]["params"]["priority"], "later")
        terminal.snapshot("coalesced-result")

    def test_claude_interrupt_stops_ralph_for_either_result_ack_order(self):
        for mode in ["claude-interrupt-before-ack", "claude-interrupt-after-ack"]:
            terminal = self.launch(mode=mode, backend="claude", resume=None, directory=self.summary_root() / mode)
            self.claude_ready(terminal)
            prompt = terminal.root / ".agents/ralph-prompt.md"
            prompt.parent.mkdir(exist_ok=True)
            prompt.write_text("hold this turn")
            terminal.send(b"\x12")
            terminal.contains("Answer: hold this turn")
            terminal.send(b"\x1b")
            terminal.contains("Turn interrupted")
            deadline = time.monotonic() + 1
            while time.monotonic() < deadline:
                terminal.pump(.05)
            self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
            frames = self.helper_messages(terminal, "server")
            request = self.helper_requests(terminal, "interrupt")[-1]
            ack_index = next(i for i, frame in enumerate(frames) if frame.get("id") == request["id"])
            result_index = next(i for i, frame in enumerate(frames) if frame.get("message", {}).get("type") == "result")
            reason = frames[result_index]["message"]["terminal_reason"]
            self.assertIn(reason, ["aborted_streaming", "aborted_tools"])
            if mode.endswith("before-ack"):
                self.assertLess(result_index, ack_index)
            else:
                self.assertLess(ack_index, result_index)
            terminal.snapshot(mode)
            terminal.close()

    def test_claude_streamed_text_can_contain_protocol_method_names(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        text = "thread/tokenUsage/updated claude/exitPlan/requestApproval"
        terminal.paste(text)
        terminal.send(b"\r")
        terminal.contains("Answer: " + text)
        self.assertEqual(self.helper_requests(terminal, "prompt")[-1]["params"]["text"], text)
        terminal.snapshot("method-names-in-text")

    def test_claude_failed_interrupt_rolls_back_intent_without_forging_a_stop(self):
        terminal = self.launch(mode="claude-interrupt-error", backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("hold this turn")
        terminal.send(b"\r")
        terminal.contains("Answer: hold this turn")
        terminal.send(b"\x1b")
        terminal.wait(lambda: any(frame.get("response", {}).get("subtype") == "error" for frame in terminal.messages("server")), "failed native interrupt")
        for _ in range(4):
            terminal.pump(.05)
        self.assertNotIn("Turn interrupted", terminal.text)
        terminal.paste("after failed interrupt")
        terminal.send(b"\r")
        terminal.contains("Answer: after failed interrupt")
        self.assertEqual(self.helper_requests(terminal, "prompt")[-1]["params"]["priority"], "later")
