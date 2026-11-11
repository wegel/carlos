"""Run native Codex monitor and session-name checks with a local, fixed-reply model peer."""

import argparse
import json
from pathlib import Path
import shutil
import tempfile
import time
import unittest

from mock_responses import Model, answer, outputs, tool
from codex_history_fixture import write_history, write_legacy
from terminal import Terminal
from test_native_codex_controls import NativeCodexControls
from test_native_codex_panel import NativeCodexPanel
from test_native_codex_goals import NativeCodexGoals
from test_native_codex_reconnect import NativeCodexReconnect
from native_shared_codex import launch_shared


class NativeMonitor(NativeCodexControls,NativeCodexPanel,NativeCodexGoals,NativeCodexReconnect,unittest.TestCase):
    def launch_shared(self,reply,feature_config=""):
        return launch_shared(self,BINARY,CODEX,ARTIFACTS/self._testMethodName,reply,feature_config)

    def resume_shared(self,root,thread,socket):
        terminal=Terminal(BINARY,root,resume=thread,codex=CODEX,extra_args=["--codex-socket",str(socket)])
        self.addCleanup(terminal.close)
        return terminal

    def owned_stale_fixture(self,reply):
        root=ARTIFACTS/self._testMethodName
        root.mkdir(parents=True)
        model=Model(root,reply)
        self.addCleanup(model.close)
        thread,turns,path=write_history(root,count=2,provider="fixture")
        rows=[json.loads(line) for line in path.read_text().splitlines()]
        rows=[row for row in rows if not (row.get("payload",{}).get("type")=="task_complete" and row["payload"].get("turn_id")==turns[-1])]
        for index,row in enumerate(rows): row["ordinal"]=index
        path.write_text("".join(json.dumps(row)+"\n" for row in rows))
        terminal=Terminal(BINARY,root,resume=thread,codex=CODEX)
        self.addCleanup(terminal.close)
        return terminal,model

    def resume_native_goal(self,root,thread):
        terminal=Terminal(BINARY,root,resume=thread,codex=CODEX)
        self.addCleanup(terminal.close)
        return terminal

    def test_rename_uses_native_codex_metadata_and_survives_reload(self):
        terminal, model = self.launch(lambda _body: [answer("Saved conversation")])
        terminal.paste("/rename Before the first prompt")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.placeholder_is("Before the first prompt"), "new session name")
        self.assertEqual(model.requests, [], "naming a new session must not invoke the model")
        terminal.paste("hello")
        terminal.send(b"\r")
        terminal.contains("Saved conversation")
        terminal.paste("/rename Native session name")
        terminal.send(b"\r")
        terminal.wait(lambda: terminal.placeholder_is("Native session name"), "saved name")
        terminal.send(b"\r")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(model.requests), 1, "rename must not invoke the model")
        self.assertEqual(len(terminal.requests("thread/name/set")), 2)
        thread_id = terminal.requests("turn/start")[0]["params"]["threadId"]
        terminal.close()
        resume_root = terminal.root / "resumed"
        resume_root.mkdir()
        (resume_root / "codex-home").symlink_to(terminal.root / "codex-home", target_is_directory=True)
        resumed = Terminal(BINARY, resume_root, resume=thread_id, codex=CODEX)
        self.addCleanup(resumed.close)
        resumed.contains("Saved conversation")
        resumed.wait(lambda: resumed.placeholder_is("Native session name"), "restored name")

    def test_native_codex_steer_edit_is_refused_before_fork_and_first_prompt_stays_editable(self):
        def reply(body):
            texts = [part.get("text", "") for item in body.get("input", []) if item.get("role") == "user"
                     for part in item.get("content", [])]
            if "edited initial native" in texts:
                return [answer("NATIVE_EDITED_INITIAL")]
            if "native-shared-sleep" in outputs(body):
                return [answer("NATIVE_AFTER_STEER")]
            return [{**answer("NATIVE_BEFORE_STEER"), "phase":"commentary"},
                    tool("native-shared-sleep", "clock", "sleep", {"duration_ms":120000})]

        terminal, model = self.launch(reply)
        for expected in ["Wait up to 120 s", "NATIVE_AFTER_STEER"]:
            terminal.paste("same native prompt")
            terminal.send(b"\r")
            terminal.contains(expected)
        terminal.wait(lambda: any(row.get("method") == "turn/completed" for row in terminal.messages("server")), "shared turn complete")
        terminal.contains("you (steer)")
        thread = terminal.requests("turn/start")[0]["params"]["threadId"]
        source = next((terminal.root / "codex-home/sessions").rglob("*" + thread + ".jsonl"))
        terminal.wait(lambda: any(json.loads(row).get("payload", {}).get("type") == "task_complete"
                      for row in source.read_text().splitlines()), "source turn persisted")
        original = source.read_bytes()
        terminal.rewind("same native prompt")
        terminal.replace_input("keep this rejected draft")
        terminal.send(b"\r")
        terminal.contains("This message is a steer")
        self.assertTrue(terminal.input_is("keep this rejected draft"))
        self.assertEqual(terminal.requests("thread/fork"), [])
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(source.read_bytes(), original)
        terminal.snapshot("native-steer-rejected")
        terminal.close()
        root = terminal.root / "resumed"
        root.mkdir()
        (root / "codex-home").symlink_to(terminal.root / "codex-home", target_is_directory=True)
        resumed = Terminal(BINARY, root, resume=thread, codex=CODEX)
        self.addCleanup(resumed.close)
        resumed.contains("NATIVE_AFTER_STEER")
        resumed.contains("you (steer)")
        resumed.rewind("same native prompt")
        resumed.send(b"\x1b[A")  # Same text, but the earlier exact user item.
        resumed.replace_input("edited initial native")
        resumed.send(b"\r")
        resumed.contains("NATIVE_EDITED_INITIAL")
        self.assertEqual(len(resumed.requests("thread/fork")), 1)
        self.assertEqual(source.read_bytes(), original)
        self.assertNotIn("NATIVE_BEFORE_STEER", resumed.text)
        self.assertNotIn("(steer)", json.dumps(resumed.requests("turn/start")[-1]["params"]["input"]))
        self.assertEqual(len(model.requests), 3)
        resumed.snapshot("native-initial-edit-after-resume")

    def test_native_large_forked_history_batches_full_turns_without_losing_ancestor_cut(self):
        from native_history_regression import check_history
        check_history(self,BINARY,CODEX,ARTIFACTS/self._testMethodName)

    def test_native_socket_large_forked_history_keeps_the_exact_ancestor_cut(self):
        from native_history_regression import check_history
        check_history(self,BINARY,CODEX,ARTIFACTS/self._testMethodName,socket_mode=True)

    def test_native_legacy_recovery_and_compacted_paged_history_need_no_model_call(self):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True)
        for mode in ["legacy", "paged", "dense"]:
            case = root / mode
            case.mkdir()
            model = Model(case, lambda _body: self.fail("History reads must not call the model"))
            self.addCleanup(model.close)
            if mode == "legacy":
                thread, _ = write_legacy(case,provider="fixture")
                expected = "Committed EP118:"
            else:
                thread, _, _ = write_history(case,count=50 if mode == "dense" else 105,compact=True,
                    provider="fixture",commands_per_turn=60 if mode == "dense" else 0)
                expected = "HISTORY_REPLY:49" if mode == "dense" else "HISTORY_REPLY:104"
            started = time.monotonic()
            terminal = Terminal(BINARY,case,resume=thread,codex=CODEX,width=140)
            self.addCleanup(terminal.close)
            terminal.contains(expected)
            elapsed = time.monotonic() - started
            (case / "history-timing.json").write_text(json.dumps({"readySeconds":elapsed,
                "requests":len([message for message in terminal.messages() if "id" in message]),
                "turnPageRequests":len(terminal.requests("thread/turns/list")),
                "itemPageRequests":len(terminal.requests("thread/items/list"))},indent=2))
            self.assertTrue(all(row["params"]["includeTurns"] is False for row in terminal.requests("thread/read")),"native metadata reads must not fetch deprecated full history")
            self.assertTrue(terminal.requests("thread/resume")[0]["params"]["excludeTurns"])
            self.assertEqual(model.requests,[])
            if mode == "dense":
                self.assertEqual(len(terminal.requests("thread/items/list")),0)
                self.assertEqual(len(terminal.requests("thread/turns/list")),1)
                self.assertLess(elapsed,10,"dense history should fit comfortably within the20s read deadline")
            if mode == "paged":
                self.assertGreaterEqual(len(terminal.requests("thread/turns/list")),2)
                self.assertEqual(len(terminal.requests("thread/items/list")),0)
                terminal.rewind("history human 104")
                terminal.send(b"\x1b[A")
                terminal.wait(lambda: terminal.input_is("history human 103"), "exact adjacent paged human")
                terminal.send(b"\x1b")
            terminal.snapshot(mode + "-complete-history")
            terminal.close()

    def test_native_codex_question_keeps_composer_focus_and_returns_structured_answer(self):
        def reply(body):
            if "native-user-question" in outputs(body):
                self.assertIn("native-choice",json.dumps(outputs(body)))
                return [answer("NATIVE_QUESTION_ANSWERED")]
            return [{"type":"function_call","call_id":"native-user-question","name":"request_user_input",
                "arguments":json.dumps({"questions":[{"id":"native-choice","header":"Native choice",
                    "question":"Choose the native route","isOther":True,
                    "options":[{"label":"Alpha","description":"Use Alpha"},{"label":"Beta","description":"Use Beta"}]}]})}]

        root=ARTIFACTS/self._testMethodName
        root.mkdir(parents=True)
        model=Model(root,reply)
        self.addCleanup(model.close)
        config=root/"codex-home/config.toml"
        config.write_text(config.read_text()+"default_mode_request_user_input = true\n")
        terminal=Terminal(BINARY,root,resume=None,codex=CODEX)
        self.addCleanup(terminal.close)
        terminal.wait(lambda:terminal.input_is(""),"native composer")
        terminal.paste("Ask the isolated native question")
        terminal.send(b"\r")
        terminal.contains("F4: Codex request waiting")
        terminal.paste("keep this main draft")
        terminal.wait(lambda:terminal.input_is("keep this main draft"),"main composer while native asks")
        terminal.send(b"\x1bOS")
        terminal.contains("Native choice")
        terminal.send(b"\x13")
        terminal.contains("choose an answer")
        terminal.send(b"\x1b[B \x13")
        terminal.contains("NATIVE_QUESTION_ANSWERED")
        terminal.wait(lambda:terminal.input_is("keep this main draft"),"main draft after answer")
        callbacks=[message for message in terminal.messages("server") if message.get("method")=="item/tool/requestUserInput"]
        self.assertEqual(len(callbacks),1)
        self.assertFalse(callbacks[0]["params"]["isBlocking"])
        replies=[message for message in terminal.messages() if message.get("id")==callbacks[0]["id"] and "method" not in message]
        self.assertEqual(replies[0]["result"],{"answers":{"native-choice":{"answers":["Beta"]}}})
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),2)
        terminal.snapshot("native-question-answered-with-main-draft")

    def test_native_codex_question_skip_and_interrupt_follow_native_callback_lifetime(self):
        for action in ["skip","interrupt"]:
            root=ARTIFACTS/self._testMethodName/action
            root.mkdir(parents=True)
            def reply(body):
                if "native-cancel-question" in outputs(body):
                    return [answer("NATIVE_EXPLICIT_SKIP_DONE")]
                return [{"type":"function_call","call_id":"native-cancel-question","name":"request_user_input",
                    "arguments":json.dumps({"questions":[{"id":"native-q","header":"Native pending",
                        "question":"Answer or explicitly skip","options":[{"label":"Yes","description":"Continue"},{"label":"No","description":"Stop"}]}]})}]
            model=Model(root,reply)
            self.addCleanup(model.close)
            config=root/"codex-home/config.toml"
            config.write_text(config.read_text()+"default_mode_request_user_input = true\n")
            terminal=Terminal(BINARY,root,resume=None,codex=CODEX)
            self.addCleanup(terminal.close)
            terminal.wait(lambda:terminal.input_is(""),"native composer")
            terminal.paste("Ask the isolated pending question")
            terminal.send(b"\r")
            terminal.contains("F4: Codex request waiting")
            terminal.send(b"\x1bOS")
            terminal.contains("Native pending")
            callback=next(message for message in terminal.messages("server") if message.get("method")=="item/tool/requestUserInput")
            terminal.send(b"\x1bn" if action=="skip" else b"\x1bc")
            terminal.contains("NATIVE_EXPLICIT_SKIP_DONE" if action=="skip" else "Turn interrupted")
            terminal.wait(lambda:any(message.get("method")=="serverRequest/resolved" and message["params"]["requestId"]==callback["id"] for message in terminal.messages("server")),"exact native callback resolved")
            replies=[message for message in terminal.messages() if message.get("id")==callback["id"] and "method" not in message]
            if action=="skip":
                self.assertEqual(replies[0]["result"],{"answers":{}})
            else:
                self.assertEqual(replies,[])
                self.assertEqual(terminal.requests("turn/interrupt")[0]["params"],{
                    "threadId":callback["params"]["threadId"],"turnId":callback["params"]["turnId"]})
            self.assertEqual(len(terminal.requests("turn/start")),1)
            self.assertEqual(terminal.requests("turn/steer"),[])
            terminal.snapshot(action+"-native-question-settled")
            terminal.close()

    def test_native_codex_mcp_form_returns_typed_values_to_the_actual_local_server(self):
        def reply(body):
            result=next((item.get("output") for item in body.get("input",[])
                if item.get("type") in ("function_call_output","custom_tool_call_output") and item.get("call_id")=="native-mcp-ask"),None)
            if result is not None:
                return [answer("NATIVE_MCP_FORM_DONE" if "MCP_FORM_ACCEPTED" in json.dumps(result) else "MCP_FIXTURE_FAILED")]
            if "mcp__forms" not in json.dumps(body.get("input",[])):
                return [answer("MCP_NAMESPACE_NOT_ADVERTISED")]
            code="const entry=ALL_TOOLS.find(tool=>tool.name.includes('mcp__forms') && tool.name.endsWith('ask')); if(!entry) throw new Error('MCP fixture tool missing'); text(await tools[entry.name]({}));"
            return [{"type":"custom_tool_call","call_id":"native-mcp-ask","namespace":"functions","name":"exec","input":code}]

        root=ARTIFACTS/self._testMethodName
        root.mkdir(parents=True)
        model=Model(root,reply)
        self.addCleanup(model.close)
        config=root/"codex-home/config.toml"
        server=Path(__file__).with_name("native_mcp_fixture.py").resolve()
        config.write_text(config.read_text().replace('approval_policy = "never"','approval_policy = "on-request"')+f'\n[mcp_servers.forms]\ncommand = "{shutil.which("python3")}"\nargs = [{json.dumps(str(server))}]\nenv = {{ CARLOS_TEST_ROOT = {json.dumps(str(root))} }}\n')
        terminal=Terminal(BINARY,root,resume=None,codex=CODEX)
        self.addCleanup(terminal.close)
        terminal.wait(lambda:terminal.input_is(""),"MCP native composer")
        terminal.paste("Ask the isolated local MCP form")
        terminal.send(b"\r")
        terminal.wait(lambda:"Native MCP form" in terminal.text or "One-time human approval" in terminal.text,"native MCP form or managed approval")
        if "One-time human approval" in terminal.text:
            self.assertNotIn("s accept session",terminal.text)
            terminal.send(b"y")
        terminal.contains("Native MCP form")
        terminal.contains("age")
        terminal.replace_input("17")
        terminal.send(b"\x13")
        terminal.contains("enter at least 18")
        terminal.replace_input("25")
        terminal.send(b"\x13")
        terminal.contains("NATIVE_MCP_FORM_DONE")
        records=[json.loads(line) for line in (root/"mcp-fixture.jsonl").read_text().splitlines()]
        response=next(record["message"] for record in records if record["direction"]=="client" and record["message"].get("id")=="fixture-form")
        self.assertEqual(response["result"],{"action":"accept","content":{"age":25,"choice":"jp","enabled":False}})
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),2)
        terminal.snapshot("actual-native-mcp-form-answered")

    def launch(self, reply, feature_config=""):
        root = ARTIFACTS / self._testMethodName
        root.mkdir(parents=True)
        model = Model(root, reply)
        self.addCleanup(model.close)
        if feature_config:
            config=root/"codex-home/config.toml"
            config.write_text(config.read_text()+feature_config)
        terminal = Terminal(BINARY, root, resume=None, codex=CODEX)
        self.addCleanup(terminal.close)
        terminal.wait(lambda: terminal.input_is(""), "composer ready")
        return terminal, model

    def test_native_sleep_wakes_on_user_input_without_an_extra_turn(self):
        def reply(body):
            if "monitor-sleep" in outputs(body):
                return [answer("Checked immediately after your message")]
            return [tool("monitor-sleep", "clock", "sleep", {"duration_ms": 120000})]

        terminal, model = self.launch(reply)
        terminal.paste("Monitor cleanup")
        terminal.send(b"\r")
        terminal.contains("Wait up to 120 s")
        terminal.snapshot("native-wait")
        terminal.paste("Check now")
        terminal.send(b"\r")
        terminal.contains("Checked immediately after your message")
        self.assertEqual(len(terminal.requests("turn/start")), 1)
        self.assertEqual(len(terminal.requests("turn/steer")), 1)
        self.assertEqual(terminal.text.count("Wait up to 120 s"), 1)
        self.assertIn("Sleep interrupted by new input", json.dumps(model.requests))

    def test_native_child_finishes_after_parent_answer_and_survives_reload(self):
        def reply(body):
            text = json.dumps(body.get("input", []))
            if "monitor-spawn" not in text and "native-monitor-child" in text:
                self.assertTrue(model.release.wait(15), "test must release the child")
                return [answer("Cleanup finished")]
            if "Cleanup finished" in text:
                return [answer("Monitor report: cleanup finished")]
            if "Read the monitor result" in text:
                return []  # Let Codex drain the queued child message at its next safe point.
            if "monitor-spawn" in outputs(body):
                return [answer("Monitor running")]
            return [tool("monitor-spawn", "collaboration", "spawn_agent", {
                "task_name": "monitor", "message": "native-monitor-child", "fork_turns": "none"})]

        terminal, model = self.launch(reply)
        terminal.paste("Start a background monitor")
        terminal.send(b"\r")
        terminal.contains("Monitor running")
        terminal.contains("Agent /root/monitor started")
        terminal.wait(lambda: any(message.get("method") == "turn/completed"
                      for message in terminal.messages("server")), "parent finished")
        model.release.set()
        terminal.contains("Agent /root/monitor completed")
        terminal.snapshot("native-child-completed")
        self.assertEqual(terminal.text.count("Agent /root/monitor completed"), 1)
        self.assertNotIn("Cleanup finished", terminal.text,
                         "child messages must not appear as the parent's answer")
        self.assertEqual(len(terminal.requests("turn/start")), 1,
                         "Carlos must not invent a new turn on child completion")
        terminal.paste("Read the monitor result")
        terminal.send(b"\r")
        terminal.contains("Monitor report: cleanup finished")
        thread_id = terminal.requests("turn/start")[0]["params"]["threadId"]
        terminal.close()
        resume_root = terminal.root / "resumed"
        resume_root.mkdir()
        (resume_root / "codex-home").symlink_to(terminal.root / "codex-home", target_is_directory=True)
        resumed = Terminal(BINARY, resume_root, resume=thread_id, codex=CODEX)
        self.addCleanup(resumed.close)
        resumed.contains("Monitor report: cleanup finished")
        resumed.contains("Agent /root/monitor completed")
        self.assertEqual(resumed.text.count("Agent /root/monitor completed"), 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--codex", default="codex")
    parser.add_argument("--artifacts", type=Path)
    args, remaining = parser.parse_known_args()
    BINARY = args.binary.resolve()
    CODEX = Path(shutil.which(args.codex) or args.codex).resolve()
    ARTIFACTS = (args.artifacts or Path(tempfile.mkdtemp(prefix="carlos-native-monitor-"))).resolve()
    unittest.main(argv=[__file__, *remaining])
