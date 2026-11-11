"""Keep human labels and compact status grounded in captured UUIDs and current native work."""

import json
import time


F2 = b"\x1bOQ"
F3 = b"\x1bOR"


class InputStatus:
    def status_send(self,terminal,text):
        terminal.paste(text)
        terminal.send(b"\r")

    def assert_quiet_status(self,terminal):
        line = terminal.screen.display[-2]
        for hint in ("F2","F3","Enter:","Steer"):
            self.assertNotIn(hint,line)

    def prepare_identical_claude_inputs(self):
        terminal = self.launch(mode="claude-labels",backend="claude",resume=None,width=100,layout="flow")
        self.claude_ready(terminal)
        self.assert_quiet_status(terminal)
        text = "identical 日本語"
        self.status_send(terminal,text)
        terminal.contains("Answer: " + text)
        self.assertNotIn("you (steer)",terminal.text)
        self.status_send(terminal,text)
        terminal.contains("you (steer)")
        terminal.send(F3)
        terminal.wait(lambda:"Queue" in terminal.screen.display[-2],"selected Queue")
        self.status_send(terminal,text)
        terminal.wait(lambda:len(self.helper_requests(terminal,"prompt")) == 3,"three accepted UUIDs")
        terminal.send(F3)
        terminal.wait(lambda:"Steer" in terminal.screen.display[-2],"explicit Steer acknowledgement")
        terminal.send(b"\x1b")
        terminal.contains("Queued once: " + text)
        self.fence_native(terminal,3)
        requests = self.helper_requests(terminal,"prompt")
        self.assertEqual([row["params"]["inputKind"] for row in requests],["prompt","steer","prompt"])
        self.assertEqual([row["params"]["priority"] for row in requests],["later","now","later"])
        ids = [row["params"]["uuid"] for row in requests]
        self.assertEqual(len(set(ids)),3)
        users = [row for row in terminal.messages() if row.get("type") == "user"]
        self.assertEqual([row["uuid"] for row in users],ids)
        self.assertTrue(all("inputKind" not in row for row in users))
        self.assertEqual(terminal.text.count("you (steer)"),1)
        session = next(row["session_id"] for row in terminal.messages("server") if row.get("type") == "result")
        terminal.snapshot("three-identical-uuid-labels")
        return terminal,session,ids

    def test_claude_labels_survive_resume_and_uuid_remapped_forks_and_steers_stay_editable(self):
        terminal,session,ids = self.prepare_identical_claude_inputs()
        terminal.close()
        source = next((terminal.root/"claude-home/projects").rglob(session+".jsonl"))
        original = source.read_bytes()
        for name in ("rpc.jsonl","claude-controller.jsonl","keys.log"):
            (terminal.root/name).rename(terminal.root/("before-resume-"+name))
        resumed = self.launch(backend="claude",resume=session,directory=terminal.root,width=100,layout="flow")
        self.claude_ready(resumed)
        resumed.contains("you (steer)")
        self.assertEqual(resumed.text.count("you (steer)"),1)
        self.edit(resumed,"identical 日本語","branch after queued human")
        self.assertEqual(self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"],ids[2])
        self.assertEqual(resumed.text.count("you (steer)"),1)
        history = [frame["result"] for frame in self.helper_messages(resumed,"server") if frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict) and "messages" in frame["result"]]
        copied_steer = next(row["message"]["uuid"] for page in reversed(history) for row in page["messages"] if row.get("inputKind") == "steer")
        self.assertNotEqual(copied_steer,ids[1])
        resumed.rewind("branch after queued human")
        resumed.send(b"\x1b[A")
        resumed.wait(lambda:resumed.input_is("identical 日本語"),"select retained Claude steer")
        resumed.replace_input("edit the actual steer 日本語")
        resumed.send(b"\r")
        resumed.contains("Answer: edit the actual steer 日本語")
        self.assertEqual(self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"],copied_steer)
        self.assertNotIn("you (steer)",resumed.text)
        self.assertEqual(source.read_bytes(),original)
        resumed.snapshot("steer-remains-uuid-editable")

    def test_claude_unowned_native_running_input_keeps_captured_steer_intent(self):
        terminal = self.launch(mode="claude-labels-running",backend="claude",resume=None,width=100,layout="flow")
        self.claude_ready(terminal)
        terminal.wait(lambda:"Running" in terminal.screen.display[-2],"native automatic work without owned prompt")
        self.status_send(terminal,"human during native follow-up")
        terminal.contains("you (steer)")
        request = self.helper_requests(terminal,"prompt")[0]
        self.assertEqual(request["params"]["inputKind"],"steer")
        self.assertEqual(request["params"]["priority"],"now")
        self.assertEqual(request["params"]["text"],"human during native follow-up")
        self.assertEqual([row["message"]["content"] for row in terminal.messages() if row.get("type") == "user"],["human during native follow-up"])

    def test_quiet_mode_notice_expires_and_help_scrolls_on_a_phone_for_both_backends(self):
        for backend in ("codex","claude"):
            root = self.summary_root()/backend
            terminal = self.launch(backend=backend,resume=None,width=36,directory=root)
            if backend == "claude": self.claude_ready(terminal)
            else: terminal.wait(lambda:terminal.input_is(""),"Codex ready")
            terminal.resize(36,12)
            self.assert_quiet_status(terminal)
            terminal.send(F3)
            terminal.wait(lambda:"Queue" in terminal.screen.display[-2],"Queue changes Enter")
            self.assertNotIn("F3",terminal.screen.display[-2])
            terminal.send(F3)
            terminal.wait(lambda:"Steer" in terminal.screen.display[-2],"explicit Steer notice")
            terminal.wait(lambda:"Steer" not in terminal.screen.display[-2],"notice expires while idle",timeout=4)
            terminal.send(b"?")
            terminal.contains("F2 tasks")
            terminal.contains("F3 switches")
            terminal.send(b"\x1b[F")
            terminal.wait(lambda:"/tools" in terminal.text,"End reaches final help lines")
            bottom = terminal.text
            terminal.send(b"\x1b[A")
            terminal.wait(lambda:terminal.text != bottom,"Up moves after End")
            before_page = terminal.text
            terminal.send(b"\x1b[5~")
            terminal.wait(lambda:terminal.text != before_page,"PageUp moves through help")
            terminal.send(b"\x1b[H")
            terminal.contains("F3 switches")
            terminal.send(b"\x1b")
            terminal.wait(lambda:terminal.input_is(""),"help preserves empty composer")
            self.assertEqual(terminal.requests("turn/start"),[])
            self.assertEqual([row for row in terminal.messages() if row.get("type") == "user"],[])
            terminal.snapshot("quiet-phone-help-"+backend)
            terminal.close()


    def test_selected_queue_keeps_context_and_weekly_labels_at_phone_widths_for_both_backends(self):
        for backend in ("codex","claude"):
            terminal = self.launch(backend=backend,mode="weekly-blue" if backend=="codex" else "modern",
                resume=None,directory=self.summary_root()/backend,width=100)
            if backend=="claude":
                self.claude_ready(terminal)
                self.status_send(terminal,"native quotas")
                terminal.contains("W60%")
            else:
                terminal.contains("91k/100k 91%")
            terminal.send(F3)
            terminal.wait(lambda:"Queue" in terminal.screen.display[-2],"selected Queue before resize")
            for width in (54,36,28):
                terminal.resize(width,14)
                terminal.wait(lambda:"Queue" in terminal.screen.display[-2] and
                    ("91%" if backend=="codex" else "26%") in terminal.screen.display[-2],"Queue and real context fit")
                weekly="W40%/2.1d" if backend=="codex" else "W60%/1.0d"
                self.weekly_row(terminal,weekly)
                self.assertNotIn("F2",terminal.screen.display[-2])
                self.assertNotIn("F3",terminal.screen.display[-2])
                terminal.snapshot("queue-context-weekly-"+backend+"-"+str(width))
            terminal.close()

    def test_task_badge_clears_finished_work_without_idle_polling_and_keeps_child_attention(self):
        terminal=self.launch(mode="codex-panel",width=110)
        terminal.contains("Answer: second")
        terminal.paste("draft while tasks finish")
        self.open_codex_panel(terminal)
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("draft while tasks finish"),"panel closed")
        terminal.wait(lambda:"5 tasks" in terminal.screen.display[-2],"three agents and two actual processes")
        (terminal.root/"panel-step").write_text("finish-all")
        terminal.wait(lambda:(terminal.root/"panel-applied").exists() and
            (terminal.root/"panel-applied").read_text()=="finish-all","native tasks finished")
        terminal.wait(lambda:"task" not in terminal.screen.display[-2].lower(),"terminal observations and empty process reads clear count")
        # The count hides uncertain owners before in-flight targeted reads settle.
        # A completed empty F2 snapshot gives this idle-poll assertion a visible fence.
        terminal.send(F2)
        terminal.wait(lambda:"All verified descendants" in terminal.text and "Reading agents" not in terminal.text,
            "empty process snapshot settled after terminal observations")
        self.assertNotIn("Process:",terminal.text)
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("draft while tasks finish"),"settled panel closed")
        reads=len(terminal.requests("thread/backgroundTerminals/list"))
        panel_reads=len(terminal.requests("thread/list"))
        deadline=time.monotonic()+3.3
        while time.monotonic()<deadline: terminal.pump(.05)
        self.assertEqual(len(terminal.requests("thread/backgroundTerminals/list")),reads,"no idle hidden polling")
        self.assertEqual(len(terminal.requests("thread/list")),panel_reads,"no hidden full-panel reads")
        (terminal.root/"panel-step").write_text("child-question")
        terminal.wait(lambda:"1 task" in terminal.screen.display[-2] and "F4" in terminal.screen.display[-2],"independent child needs attention")
        self.assertTrue(terminal.input_is("draft while tasks finish"))
        self.assertNotIn("LATE_CHILD_HIDDEN",terminal.text)
        terminal.send(b"\x1bOS")
        terminal.contains("Child decision")
        terminal.send(b" \x13")
        terminal.wait(lambda:any(row.get("id")=="child-question-id" and "method" not in row for row in terminal.messages()),"structured child answer")
        terminal.wait(lambda:"task" not in terminal.screen.display[-2].lower(),"answered child no longer needs attention")
        self.assertEqual(terminal.requests("turn/start"),[])
        terminal.snapshot("quiet-finished-tasks-and-child-attention")
