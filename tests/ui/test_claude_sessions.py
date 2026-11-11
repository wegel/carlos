"""Assert SDK session behavior on the screen and across both helper/native pipes."""

import json
import time

from claude_session_fixtures import session_file, write_private_children, write_session


class ClaudeSessions:
    def edit_claude(self, terminal, original, edited):
        last = self.helper_requests(terminal, "prompt")[-1]["params"]["uuid"]
        terminal.wait(lambda: any(frame.get("message", {}).get("type") == "result"
                      and last in frame["message"].get("user_message_uuids", [])
                      for frame in self.helper_messages(terminal, "server")), "native edit source turn finished")
        terminal.wait(lambda: any(frame.get("message", {}).get("subtype") == "session_state_changed"
                      and frame["message"].get("state") == "idle"
                      for frame in self.helper_messages(terminal, "server")), "native edit source idle")
        for _ in range(3): terminal.pump(.05)
        self.edit(terminal, original, edited)

    def test_claude_live_repeated_edits_keep_original_and_use_new_generations(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        for text in ["first human", "second human"]:
            terminal.paste(text)
            terminal.send(b"\r")
            terminal.contains("Answer: " + text)
        start = self.helper_requests(terminal, "start")[0]["params"]["sessionId"]
        path = session_file(terminal.root, start)
        original = path.read_bytes()
        self.edit_claude(terminal, "second human", "second edited")
        self.edit_claude(terminal, "second edited", "second edited again")
        self.assertEqual(path.read_bytes(), original)
        forks = self.helper_requests(terminal, "sessions.forkBefore")
        self.assertEqual(len(forks), 2)
        self.assertNotEqual(forks[0]["params"]["sessionId"], forks[1]["params"]["sessionId"])
        prompts = self.helper_requests(terminal, "prompt")
        self.assertEqual(forks[0]["params"]["beforeMessageId"], prompts[1]["params"]["uuid"])
        self.assertEqual(forks[1]["params"]["beforeMessageId"], prompts[2]["params"]["uuid"])
        self.assertTrue(all(message["params"]["origin"] == {"kind": "human"} for message in prompts))
        self.assertEqual(len([message for message in terminal.messages() if message.get("event") == "launch"]), 3)
        terminal.snapshot("repeated-native-edits")

    def test_claude_resumed_hook_history_edits_twice_with_exact_uuids_and_original_intact(self):
        terminal = self.launch(backend="claude", resume=None, mode="claude-hook-history", width=110)
        self.claude_ready(terminal)
        texts = ["First hooked human", "SSH retained context", "SSH selected fixture human"]
        for index,text in enumerate(texts,1):
            terminal.paste(text)
            terminal.send(b"\r")
            self.fence_native(terminal,index)
        prompts = self.helper_requests(terminal,"prompt")
        selected = prompts[-1]["params"]["uuid"]
        session = self.helper_requests(terminal,"start")[0]["params"]["sessionId"]
        path = session_file(terminal.root,session)
        before = path.read_bytes()
        rows = [json.loads(line) for line in before.splitlines()]
        human = next(row for row in rows if row.get("uuid") == selected)
        self.assertTrue(any(row.get("uuid") == human["parentUuid"] and row.get("subtype") == "stop_hook_summary" for row in rows))
        terminal.close()
        resumed = self.launch(backend="claude",resume=session,directory=terminal.root,mode="claude-hook-history",width=110)
        self.claude_ready(resumed)
        resumed.contains("Answer: SSH selected fixture human")
        self.edit_claude(resumed,texts[-1],"Edited hooked human 日本語")
        first_fork = self.helper_requests(resumed,"sessions.forkBefore")[-1]
        self.assertEqual(first_fork["params"]["beforeMessageId"],selected)
        branch = self.helper_requests(resumed,"start")[-1]["params"]["resume"]
        branch_rows = [json.loads(line) for line in session_file(resumed.root,branch).read_text().splitlines()]
        retained = [row for row in branch_rows if row.get("subtype") == "stop_hook_summary" and row.get("forkedFrom",{}).get("sessionId") == session]
        self.assertEqual(len(retained),2)
        resumed.contains("Answer: SSH retained context")
        self.assertNotIn("SSH selected fixture human",resumed.text)
        new_id = self.helper_requests(resumed,"prompt")[-1]["params"]["uuid"]
        self.edit_claude(resumed,"Edited hooked human 日本語","Edited again after hooks")
        self.assertEqual(self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"],new_id)
        self.assertEqual(path.read_bytes(),before)
        self.assertEqual(len(self.helper_requests(resumed,"prompt")),5)
        self.assertEqual(resumed.text.count("Answer: Edited again after hooks"),1)
        resumed.resize(36,12)
        resumed.contains("Edited again after hooks")
        self.assertNotIn("Edit failed",resumed.text)
        resumed.snapshot("resumed-hook-fork-phone")

    def test_claude_edit_first_prompt_starts_a_genuinely_empty_session(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("original first")
        terminal.send(b"\r")
        terminal.contains("Answer: original first")
        self.edit_claude(terminal, "original first", "new first")
        launches = self.helper_requests(terminal, "start")
        self.assertEqual(len(launches), 2)
        self.assertNotIn("resume", launches[1]["params"])
        self.assertNotEqual(launches[0]["params"]["sessionId"], launches[1]["params"]["sessionId"])
        self.assertNotIn("original first", terminal.text)
        terminal.rewind("new first")
        terminal.snapshot("fresh-first-prompt")

    def test_claude_unicode_rename_persists_after_next_turn_and_restart(self):
        root = self.summary_root()
        terminal = self.launch(backend="claude", resume=None, directory=root)
        self.claude_ready(terminal)
        terminal.paste("rename source")
        terminal.send(b"\r")
        terminal.contains("Answer: rename source")
        session = self.helper_requests(terminal, "start")[0]["params"]["sessionId"]
        terminal.paste("/rename Renommé 你好 🦊")
        terminal.send(b"\r")
        terminal.contains("Renommé 你好 🦊")
        terminal.paste("after rename")
        terminal.send(b"\r")
        terminal.contains("Answer: after rename")
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 2)
        self.assertEqual(self.helper_requests(terminal, "sessions.rename")[0]["params"]["title"], "Renommé 你好 🦊")
        terminal.close()
        resumed = self.launch(backend="claude", resume=session, directory=root)
        self.claude_ready(resumed)
        resumed.contains("Renommé 你好 🦊")
        resumed.contains("Answer: after rename")
        resumed.rewind("after rename")
        resumed.snapshot("renamed-resumed-history")

    def test_claude_picker_and_continue_use_the_same_sdk_session_as_history(self):
        for mode in ["__picker__", "__continue__"]:
            root = self.summary_root() / mode
            session, _, _ = write_session(root, ["programmatic saved prompt"], title="SDK saved title")
            terminal = self.launch(backend="claude", resume=mode, directory=root)
            if mode == "__picker__":
                terminal.contains("SDK saved title")
                terminal.send(b"\r")
            self.claude_ready(terminal)
            terminal.contains("Answer: programmatic saved prompt")
            start = self.helper_requests(terminal, "start")[-1]
            self.assertEqual(start["params"]["resume"], session)
            terminal.paste("resume followup")
            terminal.send(b"\r")
            terminal.contains("Answer: resume followup")
            terminal.snapshot("sdk-" + mode.strip("_"))
            terminal.close()

    def test_claude_unknown_source_requires_explicit_branch_choice(self):
        root = self.summary_root()
        session, path, _ = write_session(root, ["older unknown"], ["unknown"])
        original = path.read_bytes()
        terminal = self.launch(backend="claude", resume=session, directory=root, width=54)
        self.claude_ready(terminal)
        terminal.contains("[Source unknown] older unknown")
        terminal.send(b"\x1b")
        for _ in range(3): terminal.pump(.05)
        terminal.send(b"\x1b")
        terminal.contains("F4: older unknown prompts")
        self.assertTrue(terminal.input_is(""))
        terminal.send(b"\x1bOS")  # F4 selects records without asserting their author.
        terminal.wait(lambda: terminal.input_is("older unknown"), "explicit unknown-source edit")
        terminal.send(b"\x1bOS")
        terminal.wait(lambda: terminal.input_is(""), "no known human record is selected")
        terminal.send(b"\x1bOS")
        terminal.wait(lambda: terminal.input_is("older unknown"), "unknown record selected again")
        terminal.replace_input("new human branch")
        terminal.send(b"\r")
        terminal.contains("Answer: new human branch")
        self.assertEqual(path.read_bytes(), original)
        self.assertEqual(self.helper_requests(terminal, "prompt")[0]["params"]["origin"], {"kind": "human"})
        terminal.snapshot("explicit-unknown-branch")

    def test_claude_compaction_retains_earlier_humans_and_excludes_automatic_summary(self):
        root = self.summary_root()
        session, path, rows = write_session(root, ["before compact", "automatic continuation", "later human"],
                                             ["human", "auto-continuation", "human"], compact=True)
        original = path.read_bytes()
        terminal = self.launch(backend="claude", resume=session, directory=root)
        self.claude_ready(terminal)
        terminal.contains("Conversation compacted")
        terminal.rewind("later human")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("before compact"), "only human prompts enter ordinary rewind")
        terminal.replace_input("branch before compaction")
        terminal.send(b"\r")
        terminal.contains("Answer: branch before compaction")
        self.assertEqual(path.read_bytes(), original)
        self.assertEqual(self.helper_requests(terminal, "sessions.forkBefore")[0]["params"]["beforeMessageId"], rows[0]["uuid"])
        terminal.snapshot("precompact-branch")

    def test_claude_closed_helper_preserves_draft_instead_of_queueing_forever(self):
        terminal = self.launch(backend="claude", resume=None)
        self.claude_ready(terminal)
        terminal.paste("close helper")
        terminal.send(b"\r")
        terminal.contains("Claude closed")
        terminal.paste("recover this draft 🦊")
        terminal.send(b"\r")
        terminal.contains("Claude is closed")
        self.assertTrue(terminal.input_is("recover this draft 🦊"))
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("closed-draft-preserved")

    def test_claude_fork_startup_hook_remains_answerable_after_old_helper_closes(self):
        terminal = self.launch(backend="claude", resume=None, mode="claude-startup-question")
        terminal.contains("Allow startup hook?")
        terminal.send(b"n")
        self.claude_ready(terminal)
        terminal.paste("original hook prompt")
        terminal.send(b"\r")
        terminal.contains("Answer: original hook prompt")
        terminal.rewind("original hook prompt")
        terminal.replace_input("edited hook prompt")
        terminal.send(b"\r")
        terminal.contains("Allow startup hook?")
        for _ in range(8): terminal.pump(.05)
        terminal.contains("Allow startup hook?")
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.send(b"n")
        terminal.contains("Answer: edited hook prompt")
        self.assertEqual(len(self.helper_requests(terminal, "callback.respond")), 2)
        terminal.snapshot("fork-startup-question")

    def test_claude_replacement_start_failure_keeps_edited_text_as_a_closed_draft(self):
        terminal = self.launch(backend="claude", resume=None, mode="claude-fork-start-failure")
        self.claude_ready(terminal)
        terminal.paste("before failed fork startup")
        terminal.send(b"\r")
        terminal.contains("Answer: before failed fork startup")
        terminal.rewind("before failed fork startup")
        terminal.replace_input("recover edited 🦊")
        terminal.send(b"\r")
        terminal.contains("Claude closed")
        self.assertTrue(terminal.input_is("recover edited 🦊"))
        terminal.send(b"\r")
        terminal.contains("Claude is closed")
        self.assertTrue(terminal.input_is("recover edited 🦊"))
        self.assertEqual(len(self.helper_requests(terminal, "prompt")), 1)
        terminal.snapshot("fork-start-failed-draft")

    def test_claude_prefork_failure_keeps_original_session_and_rewind_draft(self):
        root = self.summary_root()
        session, path, _ = write_session(root, ["original ready prompt"])
        terminal = self.launch(backend="claude", resume=session, directory=root)
        self.claude_ready(terminal)
        terminal.rewind("original ready prompt")
        terminal.replace_input("retain failed edit")
        path.unlink()
        terminal.send(b"\r")
        terminal.wait(lambda: any(frame.get("ok") is False for frame in self.helper_messages(terminal, "server")), "failed public fork")
        terminal.contains("Edit failed")
        self.assertTrue(terminal.input_is("retain failed edit"))
        terminal.send(b"\x1b")
        terminal.wait(lambda: terminal.input_is(""), "cancel failed edit")
        terminal.paste("same helper still usable")
        terminal.send(b"\r")
        terminal.contains("Answer: same helper still usable")
        self.assertEqual(len([message for message in terminal.messages() if message.get("event") == "launch"]), 1)
        terminal.snapshot("prefork-failure")

    def test_claude_large_history_pages_match_native_resume_and_keep_old_human_anchors(self):
        root = self.summary_root()
        session, _, rows = write_session(root, [f"saved prompt {index}" for index in range(130)])
        terminal = self.launch(backend="claude", resume=session, directory=root, width=54)
        self.claude_ready(terminal)
        terminal.contains("Answer: saved prompt 129")
        self.assertGreaterEqual(len(self.helper_requests(terminal, "sessions.read")), 5)
        terminal.rewind("saved prompt 129")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("saved prompt 128"), "paged native prompt selection")
        terminal.replace_input("edited paged prompt")
        terminal.send(b"\r")
        terminal.contains("Answer: edited paged prompt")
        self.assertEqual(self.helper_requests(terminal, "sessions.forkBefore")[0]["params"]["beforeMessageId"], rows[256]["uuid"])
        terminal.snapshot("paged-history-edited")

    def test_claude_resume_keeps_consumed_humans_editable_and_automatic_prompts_out_of_rewind(self):
        root = self.summary_root()
        terminal = self.launch(backend="claude", resume=None, directory=root, mode="claude-ralph-followup")
        self.claude_ready(terminal)
        prompt = root / ".agents/ralph-prompt.md"
        prompt.parent.mkdir(exist_ok=True)
        prompt.write_text("automatic saved source")
        terminal.send(b"\x12")
        terminal.contains("@@AWAITING_HUMAN@@")
        terminal.paste("consumed human prompt")
        terminal.send(b"\r")
        terminal.wait(lambda: len(self.helper_requests(terminal, "prompt")) == 3, "human after automatic prompts")
        terminal.wait(lambda: len([frame for frame in terminal.messages("server") if frame.get("type") == "result"]) == 3, "all prompts consumed")
        session = self.helper_requests(terminal, "start")[0]["params"]["sessionId"]
        self.assertEqual([request["params"]["origin"]["kind"] for request in self.helper_requests(terminal, "prompt")],
                         ["auto-continuation", "auto-continuation", "human"])
        terminal.close()
        resumed = self.launch(backend="claude", resume=session, directory=root)
        self.claude_ready(resumed)
        resumed.contains("Automatic prompt:")
        resumed.rewind("consumed human prompt")
        resumed.send(b"\x1b[A")
        for _ in range(4): resumed.pump(.05)
        self.assertTrue(resumed.input_is("consumed human prompt"))
        self.assertNotIn("F4: older unknown prompts", resumed.text)
        resumed.snapshot("saved-source-rewind")

    def test_claude_failed_startup_recovers_all_queued_humans_without_replacing_new_composer(self):
        root = self.summary_root()
        prompt = root / ".agents/ralph-prompt.md"
        prompt.parent.mkdir(parents=True)
        prompt.write_text("automatic work must not become a draft")
        terminal = self.launch(backend="claude", resume=None, directory=root, mode="claude-startup-closed")
        terminal.contains("___k/___k")
        terminal.send(b"\x12")
        for text in ["first queued human", "second queued 你好 🦊"]:
            terminal.paste(text)
            terminal.send(b"\r")
        terminal.paste("newer composer stays")
        terminal.contains("Claude closed")
        self.assertTrue(terminal.input_is("newer composer stays"))
        self.assertEqual(self.helper_requests(terminal, "prompt"), [])
        terminal.send(b"\x1b[15~")  # F5 cannot replace the newer composer.
        terminal.contains("Copy or clear the current draft")
        self.assertTrue(terminal.input_is("newer composer stays"))
        for expected in ["first queued human", "second queued 你好 🦊"]:
            terminal.replace_input("")
            terminal.send(b"\x1b[15~")
            terminal.wait(lambda: terminal.input_is(expected), "recover queued human draft")
        terminal.replace_input("")
        terminal.send(b"\x1b[15~")
        for _ in range(3): terminal.pump(.05)
        self.assertTrue(terminal.input_is(""))
        terminal.snapshot("startup-queued-drafts-recovered")

    def test_claude_resumed_child_workstreams_stay_private_and_main_report_tools_remain(self):
        root = self.summary_root()
        session = write_private_children(root)
        terminal = self.launch(backend="claude",resume=session,directory=root,width=140)
        self.claude_ready(terminal)
        terminal.contains("MAIN_HISTORY_REPORT")
        terminal.contains("MAIN_HISTORY_COMMAND")
        terminal.contains("MAIN_HISTORY_OUTPUT")
        terminal.contains("Visible cross-session peer")
        terminal.contains("Human <task-notification> literal")
        self.assertNotIn("PRIVATE_HISTORY",terminal.text)
        pages = [frame for frame in self.helper_messages(terminal,"server") if frame.get("result",{}).get("messages")]
        self.assertTrue(any(row["message"].get("agent_id") == "child" for page in pages for row in page["result"]["messages"]))
        terminal.rewind("Human <task-notification> literal")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("saved main human"),"hidden child records excluded from human rewind")
        terminal.snapshot("resumed-private-child-workstream")

    def test_claude_waiting_and_failed_edit_stay_visible_and_f5_recovers_without_replacing_newer_text(self):
        root=self.summary_root()
        session,path,rows=write_session(root,["older first human","middle human","last human"])
        terminal=self.launch(backend="claude",resume=session,directory=root,width=36)
        self.claude_ready(terminal)
        terminal.rewind("last human")
        terminal.send(b"\x1b[A\x1b[A")
        terminal.wait(lambda:terminal.input_is("older first human"),"older selected human")
        terminal.replace_input("saved edit 日本語")
        selected=rows[0]["uuid"]
        rows[0]["uuid"]="00000000-0000-4000-8000-000000000001"
        path.write_text("".join(json.dumps(row,ensure_ascii=False)+"\n" for row in rows))
        before=path.read_bytes()
        terminal.send(b"\r")
        terminal.contains("Preparing edited branch")
        terminal.replace_input("newer composer")
        terminal.send(b"\r")
        terminal.contains("Edit failed")
        self.assertTrue(terminal.input_is("newer composer"))
        self.assertEqual(self.helper_requests(terminal,"sessions.forkBefore")[-1]["params"]["beforeMessageId"],selected)
        self.assertEqual(len(self.helper_requests(terminal,"sessions.forkBefore")),1)
        self.assertEqual(self.helper_requests(terminal,"prompt"),[])
        self.assertEqual(path.read_bytes(),before)
        terminal.resize(18,10)
        terminal.contains("Edit failed")
        terminal.resize(36,12)
        terminal.contains("Edit failed · draft kept")
        terminal.replace_input("")
        terminal.wait(lambda:terminal.input_is(""),"composer cleared before explicit F5 recovery")
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda:terminal.input_is("saved edit 日本語"),"F5 recovers while original helper remains ready")
        self.assertEqual(self.helper_requests(terminal,"prompt"),[])
        terminal.snapshot("failed-edit-recovered-ready-phone")
