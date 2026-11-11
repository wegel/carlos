"""Exercise actual native socket lifetime, running-turn recovery and pending-request replay."""

import json
from pathlib import Path
import sys

from mock_responses import answer, outputs, tool
from native_shared_codex import drop_connection


class NativeCodexReconnect:
    def test_native_shared_socket_reconnect_restores_the_same_running_turn_without_resending_input(self):
        def reply(body):
            if "shared-sleep" in outputs(body): return [answer("SHARED_NATIVE_AFTER_STEER")]
            return [{**answer("SHARED_NATIVE_BEFORE_SLEEP"),"phase":"commentary"},
                tool("shared-sleep","clock","sleep",{"duration_ms":120000})]
        terminal,model,server,socket=self.launch_shared(reply)
        terminal.paste("Start the isolated shared clock")
        terminal.send(b"\r")
        terminal.contains("Wait up to 120 s")
        start=terminal.requests("turn/start")[0]
        root=start["params"]["threadId"]
        started=next(row for row in terminal.messages("server") if row.get("method")=="turn/started")
        active=started["params"]["turn"]["id"]
        drop_connection(terminal)
        self.assertIsNone(server.poll(),"socket loss must not stop the shared server")
        terminal.paste("Steer the retained shared clock")
        terminal.send(b"\x1b[15~")
        terminal.contains("Codex reconnected")
        self.assertEqual(terminal.requests("thread/resume")[-1]["params"]["threadId"],root)
        terminal.wait(lambda:terminal.input_is("Steer the retained shared clock"),"composer retained across shared rebuild")
        terminal.send(b"\r")
        terminal.contains("SHARED_NATIVE_AFTER_STEER")
        steer=terminal.requests("turn/steer")[0]
        self.assertEqual(steer["params"]["expectedTurnId"],active)
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(len(terminal.requests("turn/steer")),1)
        self.assertEqual(len(model.requests),2)
        self.assertEqual(terminal.text.count("SHARED_NATIVE_AFTER_STEER"),1)
        records=[json.loads(line) for line in (terminal.root/"rpc.jsonl").read_text().splitlines()]
        self.assertFalse(any(row["connection"]==2 and row["direction"]=="client" and row["message"].get("method")=="thread/settings/update" for row in records))
        terminal.snapshot("native-shared-reconnected-running-turn")
        terminal.close()
        self.assertIsNone(server.poll(),"normal Carlos exit only closes its local socket")
        self.assertIn("--codex-socket",(terminal.root/"terminal.ansi").read_text(errors="replace"))
        self.assertIn(str(socket),(terminal.root/"terminal.ansi").read_text(errors="replace"))

    def test_native_shared_root_question_replays_with_its_exact_id_and_no_old_answer(self):
        def reply(body):
            if "shared-question" in outputs(body): return [answer("SHARED_QUESTION_ANSWERED")]
            return [{"type":"function_call","call_id":"shared-question","name":"request_user_input",
                "arguments":json.dumps({"questions":[{"id":"shared-choice","header":"Shared choice",
                    "question":"Choose after reconnect","options":[{"label":"Alpha","description":"A"},{"label":"Beta","description":"B"}]}]})}]
        terminal,model,server,_socket=self.launch_shared(reply,"default_mode_request_user_input = true\n")
        terminal.paste("Ask the isolated shared question")
        terminal.send(b"\r")
        terminal.contains("F4: Codex request waiting")
        first=next(row for row in terminal.messages("server") if row.get("method")=="item/tool/requestUserInput")
        terminal.paste("keep main shared draft")
        drop_connection(terminal)
        terminal.send(b"\x1b[15~")
        terminal.contains("Codex reconnected")
        callbacks=[row for row in terminal.messages("server") if row.get("method")=="item/tool/requestUserInput"]
        self.assertEqual(len(callbacks),2)
        self.assertEqual(callbacks[0]["id"],callbacks[1]["id"])
        terminal.send(b"\x1bOS")
        terminal.contains("Shared choice")
        terminal.send(b"\x13")
        terminal.contains("choose an answer")
        terminal.send(b"\x1b[B \x13")
        terminal.contains("SHARED_QUESTION_ANSWERED")
        replies=[row for row in terminal.messages() if row.get("id")==first["id"] and "method" not in row]
        self.assertEqual([row["result"] for row in replies],[{"answers":{"shared-choice":{"answers":["Beta"]}}}])
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),2)
        self.assertIsNone(server.poll())
        terminal.wait(lambda:terminal.input_is("keep main shared draft"),"main draft after replayed callback")
        terminal.snapshot("native-root-pending-request-replay")

    def test_native_initial_socket_resume_finds_existing_active_turn_without_a_fresh_start(self):
        def reply(body):
            if "attach-sleep" in outputs(body): return [answer("INITIAL_ATTACH_STEERED")]
            return [tool("attach-sleep","clock","sleep",{"duration_ms":120000})]
        terminal,model,server,socket=self.launch_shared(reply)
        terminal.paste("Keep this native turn running across a new Carlos process")
        terminal.send(b"\r")
        terminal.contains("Wait up to 120 s")
        root=terminal.requests("turn/start")[0]["params"]["threadId"]
        started=next(row for row in terminal.messages("server") if row.get("method")=="turn/started")
        active=started["params"]["turn"]["id"]
        terminal.close()
        self.assertIsNone(server.poll())
        resume_root=terminal.root/"initial-resume"
        resume_root.mkdir()
        (resume_root/"codex-home").symlink_to(terminal.root/"codex-home",target_is_directory=True)
        # The recorder writes one ordered log across clients, with explicit connection IDs.
        resumed=self.resume_shared(resume_root,root,socket)
        resumed.contains("Keep this native turn running")
        def rebuilt():
            rows=[json.loads(line) for line in (terminal.root/"rpc.jsonl").read_text().splitlines()]
            return sum(row["connection"]==2 and row["direction"]=="client" and row["message"].get("method")=="thread/read" for row in rows)>=2
        resumed.wait(rebuilt,"initial snapshot installed before normal metadata reconciliation")
        # Native does not persist a display item for clock.sleep; the retained user row is authoritative.
        resumed.paste("Wake the already running native turn")
        resumed.send(b"\r")
        resumed.contains("INITIAL_ATTACH_STEERED")
        rows=[json.loads(line) for line in (terminal.root/"rpc.jsonl").read_text().splitlines()]
        writes=[row["message"] for row in rows if row["connection"]==2 and row["direction"]=="client"]
        self.assertEqual([row for row in writes if row.get("method")=="turn/start"],[])
        steer=next(row for row in writes if row.get("method")=="turn/steer")
        self.assertEqual(steer["params"]["expectedTurnId"],active)
        self.assertEqual(steer["params"]["threadId"],root)
        self.assertEqual([row for row in writes if row.get("method")=="thread/settings/update"],[])
        self.assertEqual(len(model.requests),2)
        resumed.snapshot("initial-attachment-restores-current-turn")

    def test_native_root_reattach_does_not_replay_a_child_owned_question(self):
        def reply(body):
            text=json.dumps(body.get("input",[]))
            if "shared-child-prompt" in text and "shared-child-spawn" not in text:
                if any(item.get("call_id")=="shared-child-mcp" and item.get("type") in ("function_call_output","custom_tool_call_output") for item in body.get("input",[])):
                    return [answer("CHILD_MCP_FINISHED")]
                code="const entry=ALL_TOOLS.find(tool=>tool.name.includes('mcp__forms') && tool.name.endsWith('ask')); if(!entry) throw new Error('MCP fixture tool missing'); text(await tools[entry.name]({}));"
                return [{"type":"custom_tool_call","call_id":"shared-child-mcp","namespace":"functions","name":"exec","input":code}]
            if "shared-child-spawn" in outputs(body): return [answer("SHARED_PARENT_FINISHED")]
            return [tool("shared-child-spawn","collaboration","spawn_agent",{
                "task_name":"shared_worker","message":"shared-child-prompt","fork_turns":"none"})]
        def config(root):
            path=root/"codex-home/config.toml"
            path.write_text(path.read_text().replace('approval_policy = "never"','approval_policy = "on-request"'))
            return f'\n[mcp_servers.forms]\ncommand = {json.dumps(sys.executable)}\nargs = [{json.dumps(str(Path(__file__).with_name("native_mcp_fixture.py").resolve()))}]\nenv = {{ CARLOS_TEST_ROOT = {json.dumps(str(root))} }}\n'
        terminal,model,server,_socket=self.launch_shared(reply,config)
        terminal.paste("Start the isolated child question")
        terminal.send(b"\r")
        terminal.wait(lambda:any(row.get("method")=="item/completed" and row.get("params",{}).get("item",{}).get("text")=="SHARED_PARENT_FINISHED" for row in terminal.messages("server")),"parent report received while child may ask")
        root=terminal.requests("turn/start")[0]["params"]["threadId"]
        def child_requests():
            return [row for row in terminal.messages("server") if "id" in row and "method" in row and row.get("params",{}).get("threadId") not in (None,root)]
        terminal.wait(lambda:bool(child_requests()),"original child callback delivered")
        first=child_requests()[0]
        child=first["params"]["threadId"]
        self.assertNotEqual(root,child)
        drop_connection(terminal)
        terminal.send(b"\x1b[15~")
        terminal.contains("Codex reconnected")
        terminal.contains("unanswered child")
        terminal.contains("requests may need")
        terminal.contains("F2 shows its state")
        terminal.send(b"\x1bOQ")
        terminal.contains("Thread: "+child)
        self.assertEqual(len(child_requests()),1,"native root resume only replays requests owned by that root")
        self.assertTrue(all(row["params"]["threadId"]==root for row in terminal.requests("thread/resume")))
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertLessEqual(len(model.requests),4)
        self.assertIsNone(server.poll())
        terminal.snapshot("native-child-replay-scope-limit")

    def test_native_owned_resume_does_not_steer_an_abandoned_persisted_turn(self):
        terminal,model=self.owned_stale_fixture(lambda _body:[answer("OWNED_FRESH_TURN")])
        terminal.contains("HISTORY_REPLY:1")
        terminal.wait(lambda:len(terminal.requests("thread/read"))>=2,"owned rebuild installed and reconciled")
        self.assertEqual(model.requests,[])
        terminal.paste("Start fresh after the previous owned process ended")
        terminal.send(b"\r")
        terminal.contains("OWNED_FRESH_TURN")
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),1)
        terminal.snapshot("owned-resume-has-no-ghost-active-turn")

    def test_native_shared_dense_history_read_stays_within_the_original_deadline(self):
        from codex_history_fixture import write_history
        import time
        generated={}
        def config(root):
            generated["thread"],_,_=write_history(root,count=50,compact=True,provider="fixture",commands_per_turn=60)
            return ""
        terminal,model,_server,socket=self.launch_shared(lambda _body:self.fail("History reads must not call the model"),config)
        terminal.close()
        root=terminal.root/"dense-resumed"
        root.mkdir()
        (root/"codex-home").symlink_to(terminal.root/"codex-home",target_is_directory=True)
        started=time.monotonic()
        resumed=self.resume_shared(root,generated["thread"],socket)
        resumed.contains("HISTORY_REPLY:49")
        elapsed=time.monotonic()-started
        rows=[json.loads(line) for line in (terminal.root/"rpc.jsonl").read_text().splitlines()]
        requests=[row["message"] for row in rows if row["connection"]==2 and row["direction"]=="client" and "method" in row["message"]]
        items=sum(row["method"]=="thread/items/list" for row in requests)
        self.assertEqual(items,0)
        self.assertEqual(sum(row["method"] == "thread/turns/list" for row in requests),1)
        self.assertLess(elapsed,10,"dense complete history must fit the original20s budget")
        self.assertEqual(model.requests,[])
        (terminal.root/"socket-history-timing.json").write_text(json.dumps({"readySeconds":elapsed,"requests":len(requests),"itemPages":items},indent=2))
        resumed.snapshot("native-shared-dense-history-ready")
