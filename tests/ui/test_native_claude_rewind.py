"""Fork actual native tool/compaction histories without changing the source conversation."""

import json
import shlex
import sys

from claude_native_api import NativeClaudeAPI


class NativeClaudeRewind:
    def source_bytes(self,terminal,session):
        path=next((terminal.root/"claude-home/projects").rglob(session+".jsonl"))
        return path,path.read_bytes()

    def source_unchanged(self,path,before,edited,branch_answer):
        after=path.read_bytes()
        assert after.startswith(before),"Existing source bytes must remain unchanged"
        appended=[json.loads(line) for line in after[len(before):].splitlines()]
        assert all(row.get("sessionId")==path.stem for row in appended),appended
        assert all(row["type"] in ("last-prompt","cost-state") or
                   row["type"]=="mode" and row.get("mode")=="normal" for row in appended),appended
        conversation=[row for row in map(json.loads,after.splitlines()) if row.get("type") in ("user","assistant")]
        assert edited not in json.dumps(conversation)
        assert branch_answer not in json.dumps(conversation)

    def test_native_edit_of_interrupted_second_prompt_keeps_tools_and_submits_once(self):
        terminal=self.start("rewind")
        self.submit(terminal,"Keep the completed tool-rich first turn")
        terminal.contains("NATIVE_REWIND_ANSWER_1")
        self.ended(terminal,1)
        self.submit(terminal,"Interrupt and replace this second prompt")
        terminal.wait(lambda: len(self.api.requests)==3,"second human request paused",timeout=20)
        terminal.send(b"\x1b")
        terminal.wait(lambda:len(self.results(terminal))==2,"native interrupted result",timeout=15)
        interrupted=self.results(terminal)[-1]
        assert interrupted.get("terminal_reason") in ("aborted_streaming","aborted_tools"),interrupted
        self.wait_native_idle(terminal,0)
        session=interrupted["session_id"]
        selected=self.helper_requests(terminal,"prompt")[-1]["params"]["uuid"]
        path,before=self.source_bytes(terminal,session)
        terminal.rewind("Interrupt and replace this second prompt")
        edited="Edited interrupted second 日本語"
        terminal.replace_input(edited)
        terminal.send(b"\r")
        terminal.contains("NATIVE_REWIND_ANSWER_3")
        self.ended(terminal,3)
        assert self.results(terminal)[-1]["session_id"]!=session
        assert self.results(terminal)[-1]["user_message_uuid"]==self.helper_requests(terminal,"prompt")[-1]["params"]["uuid"]
        assert self.helper_requests(terminal,"sessions.forkBefore")[-1]["params"]["beforeMessageId"]==selected
        messages=json.dumps(self.api.requests[-1]["messages"])
        assert "NATIVE_REWIND_TOOL_DONE" in messages
        assert "Keep the completed tool-rich first turn" in messages
        assert "Interrupt and replace this second prompt" not in messages
        assert len(self.api.requests)==4
        assert len(self.helper_requests(terminal,"prompt"))==3
        self.source_unchanged(path,before,edited,"NATIVE_REWIND_ANSWER_3")
        terminal.resize(36,12)
        terminal.contains("NATIVE_REWIND_ANSWER_3")
        terminal.snapshot("native-interrupted-edit-phone")

    def test_native_resumed_compacted_edit_keeps_the_exact_prefix_and_original(self):
        self.api=NativeClaudeAPI(self.root,"context")
        self.api.context_release.set()
        terminal=self.start("context")
        self.submit(terminal,"Keep this tool-rich precompact human")
        terminal.contains("NATIVE_CONTEXT_ANSWER_1")
        self.ended(terminal,1)
        self.submit(terminal,"/compact Keep the completed tool result")
        terminal.contains("Claude compacted context")
        self.ended(terminal,2)
        self.submit(terminal,"Replace the postcompact human")
        self.ended(terminal,3)
        old_answer=self.results(terminal)[-1]["result"]
        session=self.results(terminal)[-1]["session_id"]
        selected=self.helper_requests(terminal,"prompt")[-1]["params"]["uuid"]
        terminal.close()
        resumed=self.start(resume=session)
        resumed.contains("Conversation compacted")
        path,before=self.source_bytes(resumed,session)
        resumed.rewind("Replace the postcompact human")
        edited="Edited resumed compacted human é🦊"
        resumed.replace_input(edited)
        resumed.send(b"\r")
        self.ended(resumed,4)
        latest=self.results(resumed)[-1]
        resumed.contains(latest["result"])
        assert latest["session_id"]!=session
        assert self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"]==selected
        assert old_answer not in json.dumps(self.api.requests[-1]["messages"])
        assert "Replace the postcompact human" not in json.dumps(self.api.requests[-1]["messages"])
        assert len(self.helper_requests(resumed,"prompt"))==4
        self.source_unchanged(path,before,edited,latest["result"])
        resumed.snapshot("native-resumed-compacted-edit")

    def install_isolated_stop_hook(self):
        settings=self.root/"claude-home/settings.json"
        settings.parent.mkdir(parents=True)
        script=self.root/"isolated-stop-hook.py"
        script.write_text("from pathlib import Path\nPath("+repr(str(self.root/"stop-hook-ran"))+").touch()\nprint('{}')\n")
        settings.write_text(json.dumps({"hooks":{"Stop":[{"hooks":[{"type":"command",
            "command":shlex.join([sys.executable,str(script)])}]}]}}))

    def loaded_native_messages(self,terminal,session):
        return [record["message"] for frame in self.helper_messages(terminal,"server")
            if frame.get("type")=="reply" and frame.get("ok") and isinstance(frame.get("result"),dict)
            for record in frame["result"].get("messages",[]) if record["message"].get("session_id")==session]

    def test_native_stop_hook_suffix_after_compaction_and_resume_remains_exactly_forkable(self):
        self.install_isolated_stop_hook()
        self.api=NativeClaudeAPI(self.root,"context")
        self.api.context_release.set()
        terminal=self.start("context")
        self.submit(terminal,"Keep the hooked precompact fixture")
        self.ended(terminal,1)
        first_answer=self.results(terminal)[-1]["result"]
        self.submit(terminal,"/compact Keep the hooked fixture context")
        self.ended(terminal,2)
        self.submit(terminal,"Keep this ordinary postcompact hooked answer")
        self.ended(terminal,3)
        retained_answer=self.results(terminal)[-1]["result"]
        text="Replace the human after native Stop hooks"
        self.submit(terminal,text)
        self.ended(terminal,4)
        session=self.results(terminal)[-1]["session_id"]
        selected=self.helper_requests(terminal,"prompt")[-1]["params"]["uuid"]
        path,_=self.source_bytes(terminal,session)
        rows=[json.loads(line) for line in path.read_bytes().splitlines()]
        human=next(row for row in rows if row.get("uuid")==selected)
        hook=next(row for row in rows if row.get("uuid")==human["parentUuid"])
        assert hook["type"]=="system" and hook["subtype"]=="stop_hook_summary",hook
        assert hook["hookCount"]==1 and (self.root/"stop-hook-ran").exists()
        terminal.close()
        resumed=self.start(resume=session)
        path,before=self.source_bytes(resumed,session)
        before_requests=len(self.api.requests)
        resumed.rewind(text)
        edited="Edited after the real Stop hook 日本語"
        resumed.replace_input(edited)
        resumed.send(b"\r")
        self.ended(resumed,5)
        result=self.results(resumed)[-1]
        assert result["session_id"]!=session and result["is_error"] is False
        assert result["user_message_uuid"]==self.helper_requests(resumed,"prompt")[-1]["params"]["uuid"]
        assert self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"]==selected
        branch,_=self.source_bytes(resumed,result["session_id"])
        copied=[json.loads(line) for line in branch.read_bytes().splitlines()]
        assert any(row.get("forkedFrom")=={"sessionId":session,"messageUuid":hook["uuid"]} for row in copied)
        history=json.dumps(self.loaded_native_messages(resumed,result["session_id"]))
        assert first_answer in history and retained_answer in history
        assert text not in json.dumps(self.api.requests[-1]["messages"])
        assert len(self.helper_requests(resumed,"prompt"))==5 and len(self.api.requests)==before_requests+1
        assert sum(row["params"]["uuid"]==selected for row in self.helper_requests(resumed,"prompt"))==1
        assert len(self.api.requests)<=8,"bounded native compaction requests plus one edited answer"
        self.source_unchanged(path,before,edited,result["result"])
        resumed.resize(36,12)
        resumed.contains(result["result"])
        resumed.snapshot("native-stop-hook-fork-phone")
