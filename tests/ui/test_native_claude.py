"""Full packaged Rust UI -> standalone SDK helper -> installed native CLI -> localhost contracts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest

from claude_native_api import NativeClaudeAPI
from terminal import Terminal
from test_claude_sdk import ClaudeSdk
from test_native_claude_rewind import NativeClaudeRewind
from test_native_claude_labels import NativeClaudeLabels


class NativeClaude(NativeClaudeRewind,NativeClaudeLabels,unittest.TestCase):
    helper_messages = ClaudeSdk.helper_messages
    helper_requests = ClaudeSdk.helper_requests
    def setUp(self):
        self.root = ARTIFACTS / self._testMethodName
        self.root.mkdir(parents=True,exist_ok=True)
        self.api = None

    def start(self,scenario="text",resume=None):
        self.api = self.api or NativeClaudeAPI(self.root,scenario)
        trace = self.root/"claude-controller.jsonl"
        previous = len([line for line in trace.read_bytes().splitlines(keepends=True) if line.endswith(b"\n") and json.loads(line)["direction"] == "server"]) if trace.exists() else 0
        terminal = Terminal(BINARY,self.root,"native-claude",resume,140,"claude",claude=NATIVE,
            extra_env={"ANTHROPIC_API_KEY":"fake-local-native-contract","ANTHROPIC_BASE_URL":self.api.url,
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":"1","DISABLE_AUTOUPDATER":"1","DISABLE_TELEMETRY":"1"})
        self.addCleanup(terminal.close)
        if resume == "__picker__":
            terminal.contains("Native original café 日本語")
            selected = lambda: next(((index,line) for index,line in enumerate(terminal.screen.display) if "●" in line),None)
            for _ in range(20):
                if any("●" in line and "Native original café 日本語" in line for line in terminal.screen.display): break
                before = selected()
                terminal.send(b"\x1b[B")
                terminal.wait(lambda: selected() != before,"picker selection moved after Down")
            assert any("●" in line and "Native original café 日本語" in line for line in terminal.screen.display),terminal.text
            terminal.snapshot("native-picker-selected-original")
            terminal.send(b"\r")
        terminal.wait(lambda:terminal.input_is(""),"native composer rendered")
        terminal.wait(lambda: any(frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict) and "models" in frame["result"]
                      for frame in self.helper_messages(terminal,"server")[previous:]),"new native SDK startup reply",timeout=20)
        self.wait_native_idle(terminal,previous)
        return terminal

    def wait_native_idle(self,terminal,offset):
        def is_idle():
            states = [frame["message"].get("state") for frame in self.helper_messages(terminal,"server")[offset:]
                if frame.get("event") == "sdk" and frame["message"].get("subtype") == "session_state_changed"]
            status = terminal.screen.display[-2]
            busy = ("Running","Requesting","Compacting","Thinking ~","Requires action")
            return (not states or states[-1] == "idle") and not any(label in status for label in busy)
        # Resuming can start native task notices before initialization replies.
        # Wait for that work and its visible badge to settle before sending keys.
        terminal.wait(is_idle,"native startup work and visible activity are idle",timeout=30)

    def tearDown(self):
        if self.api:
            self.api.close()

    def submit(self,terminal,text):
        terminal.paste(text)
        terminal.send(b"\r")

    def results(self,terminal):
        return [frame["message"] for frame in self.helper_messages(terminal,"server") if frame.get("event") == "sdk" and frame["message"].get("type") == "result"]

    def ended(self,terminal,count):
        terminal.wait(lambda: len(self.results(terminal)) >= count,"native result",timeout=20)
        def idle_after_result():
            frames = self.helper_messages(terminal,"server")
            last = max((index for index,frame in enumerate(frames) if frame.get("event") == "sdk" and frame["message"].get("type") == "result"),default=-1)
            return any(frame.get("event") == "sdk" and frame["message"].get("subtype") == "session_state_changed" and frame["message"].get("state") == "idle" for frame in frames[last+1:])
        terminal.wait(idle_after_result,"native idle after result",timeout=15)
        for _ in range(4): terminal.pump(.05)
        assert self.results(terminal)[-1]["is_error"] is False, self.results(terminal)[-1]

    def allow_tool(self,terminal,tool,answer):
        def pending():
            answered = {request["params"]["callbackId"] for request in self.helper_requests(terminal,"callback.respond")}
            return next((frame for frame in self.helper_messages(terminal,"server") if frame.get("type") == "callback"
                and frame.get("payload",{}).get("toolName") == tool and frame["callbackId"] not in answered),None)
        terminal.wait(lambda: bool(pending()) or answer in terminal.text,"native tool policy or completed work",timeout=20)
        if not pending(): return
        for _ in range(3): terminal.pump(.05)
        terminal.send(b"y")
        for _ in range(3): terminal.pump(.05)
        if "Press y again" in terminal.text: terminal.send(b"y")

    def test_native_context_remains_visible_while_the_next_model_step_is_paused(self):
        terminal = self.start("context")
        self.submit(terminal,"Run the isolated context fixture.")
        terminal.wait(lambda: len(self.api.requests) == 2,"second native request paused",timeout=20)
        def contexts():
            return [frame["result"] for frame in self.helper_messages(terminal,"server")
                    if frame.get("type") == "reply" and frame.get("ok") and
                    isinstance(frame.get("result"),dict) and (frame["result"].get("apiUsage") or {}).get("cache_read_input_tokens") == 32000]
        terminal.wait(lambda: bool(contexts()),"native live context summary includes cached input",timeout=10)
        used, maximum = contexts()[-1]["totalTokens"], contexts()[-1]["rawMaxTokens"]
        assert used >= 40128 and used < 45000,contexts()[-1]
        compact = lambda value: f"{value//1000000}m" if value >= 1000000 else f"{value//1000}k" if value >= 1000 else str(value)
        label = f"{compact(used)}/{compact(maximum)}"
        terminal.contains(label)
        assert len(self.results(terminal)) == 0,"Context must show while native work is still running"
        terminal.paste("unsent context draft 日本語")
        terminal.wait(lambda: terminal.input_is("unsent context draft 日本語"),"typing remains responsive")
        terminal.resize(36,12)
        terminal.contains(label)
        terminal.snapshot("native-context-paused-phone")
        self.api.context_release.set()
        terminal.resize(140,32)
        terminal.contains("NATIVE_CONTEXT_ANSWER_1")
        self.ended(terminal,1)
        terminal.wait(lambda: any((context.get("apiUsage") or {}).get("cache_read_input_tokens") == 33000
            for context in [frame.get("result",{}) for frame in self.helper_messages(terminal,"server")
                if frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict)]),
            "new native context replaces the previous model step",timeout=10)
        assert terminal.input_is("unsent context draft 日本語")
        assert len(self.api.requests) == 2
        assert self.api.count_token_requests == 0,"Summary reads must not call the token-count API"
        latest = [frame["result"] for frame in self.helper_messages(terminal,"server")
                  if frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict)
                  and (frame["result"].get("apiUsage") or {}).get("cache_read_input_tokens") == 33000][-1]
        assert latest["totalTokens"] < used*1.5,"Use the latest native snapshot, not summed model steps"
        terminal.contains(f"{compact(latest['totalTokens'])}/{compact(latest['rawMaxTokens'])}")
        assert len(self.helper_requests(terminal,"prompt")) == 1

    def test_native_session_stream_rename_resume_and_first_later_edits(self):
        terminal = self.start()
        self.submit(terminal,"first native prompt")
        terminal.contains("NATIVE_TEXT_ANSWER_0")
        self.ended(terminal,1)
        first_id = self.results(terminal)[-1]["session_id"]
        self.submit(terminal,"second native prompt")
        terminal.contains("NATIVE_TEXT_ANSWER_1")
        self.ended(terminal,2)
        original = next((self.root/"claude-home/projects").rglob(first_id+".jsonl"))
        before = hashlib.sha256(original.read_bytes()).hexdigest()
        self.submit(terminal,"/rename Native café 日本語")
        terminal.wait(lambda: bool(self.helper_requests(terminal,"sessions.rename")),"native rename")
        terminal.rewind("second native prompt")
        terminal.replace_input("edited native second")
        terminal.send(b"\r")
        terminal.contains("NATIVE_TEXT_ANSWER_2")
        self.ended(terminal,3)
        assert hashlib.sha256(original.read_bytes()).hexdigest() != before, "Rename must persist the title"
        renamed = hashlib.sha256(original.read_bytes()).hexdigest()
        terminal.rewind("edited native second")
        terminal.send(b"\x1b[A")
        terminal.wait(lambda: terminal.input_is("first native prompt"),"native first prompt")
        terminal.replace_input("new native first")
        terminal.send(b"\r")
        terminal.contains("NATIVE_TEXT_ANSWER_3")
        self.ended(terminal,4)
        assert hashlib.sha256(original.read_bytes()).hexdigest() == renamed,"SDK forks changed original source"
        assert "NATIVE_TEXT_ANSWER_0" not in terminal.text
        assert len(self.helper_requests(terminal,"prompt")) == 4
        terminal.close()
        resumed = self.start(resume=first_id)
        resumed.contains("Native café 日本語")
        resumed.contains("NATIVE_TEXT_ANSWER_0")
        resumed.contains("NATIVE_TEXT_ANSWER_1")
        self.submit(resumed,"after native resume")
        resumed.contains("NATIVE_TEXT_ANSWER_4")
        self.ended(resumed,5)
        resumed.snapshot("native-resume-original")
        self.submit(resumed,"/rename Native original café 日本語")
        resumed.contains("Native original café 日本語")
        resumed.close()
        picker = self.start(resume="__picker__")
        assert self.helper_requests(picker,"sessions.info")[-1]["params"]["sessionId"] == first_id
        picker.close()
        continued = self.start(resume="__continue__")
        assert self.helper_requests(continued,"sessions.info")[-1]["params"]["sessionId"] == first_id
        continued.contains("Native original café 日本語")
        assert self.helper_requests(continued,"sessions.list")
        continued.snapshot("native-continue-same-session")

    def test_native_ask_question_returns_real_callback_answers_without_extra_prompt(self):
        terminal = self.start("question")
        self.submit(terminal,"Ask the native fixture question")
        terminal.contains("Which native output?")
        terminal.send(b"\r")
        terminal.contains("Review answers")
        terminal.send(b"\x13")
        terminal.contains("NATIVE_QUESTION_ANSWER_1")
        self.ended(terminal,1)
        answers = self.helper_requests(terminal,"callback.respond")
        assert answers[-1]["params"]["answer"]["updatedInput"]["answers"] == {"Which native output?":"Summary"},answers
        assert len(self.helper_requests(terminal,"prompt")) == 1
        assert "Summary" in json.dumps(self.api.requests[1])
        terminal.snapshot("native-question")

    def test_native_plan_uses_real_file_and_permission_callback(self):
        settings = self.root/"claude-home/settings.json"
        settings.parent.mkdir(parents=True)
        settings.write_text(json.dumps({"permissions":{"defaultMode":"plan"}}))
        terminal = self.start("plan")
        self.submit(terminal,"Write and approve the isolated native plan")
        terminal.contains("Packaged native plan")
        terminal.send(b"y")
        terminal.contains("NATIVE_PLAN_ANSWER_2")
        self.ended(terminal,1)
        callbacks = [frame for frame in self.helper_messages(terminal,"server") if frame.get("type") == "callback"]
        assert any(frame["payload"].get("toolName") == "ExitPlanMode" for frame in callbacks),callbacks
        assert len(self.helper_requests(terminal,"prompt")) == 1
        terminal.snapshot("native-plan-approved")

    def test_native_background_survives_clear_pipeline_then_finishes_and_can_be_stopped(self):
        terminal = self.start("background")
        self.submit(terminal,"Start the isolated background fixture")
        self.allow_tool(terminal,"Bash","NATIVE_BACKGROUND_ANSWER_1")
        terminal.contains("NATIVE_BACKGROUND_ANSWER_1")
        self.ended(terminal,1)
        first = self.results(terminal)[0]
        terminal.send(b"\x1bOR")
        terminal.wait(lambda:"Queue" in terminal.screen.display[-2],"selected Queue badge")
        self.submit(terminal,"/clear")
        self.submit(terminal,"human queued after native clear")
        terminal.contains("NATIVE_BACKGROUND_ANSWER_2")
        self.ended(terminal,3)
        assert self.results(terminal)[-1]["session_id"] != first["session_id"]
        surviving = self.helper_requests(terminal,"prompt")[-1]["params"]["uuid"]
        assert self.results(terminal)[-1]["user_message_uuid"] == surviving
        (self.root/"release-first").touch()
        terminal.contains("NATIVE_BACKGROUND_ANSWER_3")
        self.ended(terminal,4)
        self.submit(terminal,"Start a second isolated background fixture")
        self.allow_tool(terminal,"Bash","NATIVE_BACKGROUND_ANSWER_5")
        terminal.contains("NATIVE_BACKGROUND_ANSWER_5")
        self.ended(terminal,5)
        terminal.send(b"\x1bOQ")
        terminal.contains("Tasks")
        tasks = [frame["message"] for frame in self.helper_messages(terminal,"server") if frame.get("event") == "sdk" and frame["message"].get("subtype") == "task_started"]
        stop_id = tasks[-1]["task_id"]
        for _ in range(2):
            if "Task " + stop_id + " ·" in terminal.text: break
            terminal.send(b"\x1b[B")
            terminal.pump(.05)
        terminal.contains("Task " + stop_id + " ·")
        terminal.send(b"x")
        terminal.wait(lambda: any(frame.get("event") == "sdk" and frame["message"].get("subtype") == "task_notification" and frame["message"].get("status") == "stopped"
                      for frame in self.helper_messages(terminal,"server")),"actual native task stopped",timeout=20)
        assert len(self.helper_requests(terminal,"prompt")) == 4
        assert self.helper_requests(terminal,"task.stop")[-1]["params"]["taskId"] == stop_id
        terminal.snapshot("native-background-stopped")
        session = self.results(terminal)[2]["session_id"]
        terminal.close()
        resumed = self.start(resume=session)
        resumed.rewind("Start a second isolated background fixture")
        resumed.send(b"\x1b[A")
        resumed.wait(lambda: resumed.input_is("human queued after native clear"),"native persisted human UUID/source")
        resumed.send(b"\x1b[A")
        resumed.wait(lambda: resumed.input_is("human queued after native clear"),"clear and automatic wakeups excluded from human history")
        resumed.snapshot("native-reset-resume")


def package(binary,helper,root):
    folder = root/"package/bin"
    folder.mkdir(parents=True)
    packaged = folder/binary.name
    shutil.copy2(binary,packaged)
    shutil.copy2(helper,folder/helper.name)
    return packaged


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary",type=Path,required=True)
    parser.add_argument("--helper",type=Path,required=True)
    parser.add_argument("--claude",type=Path)
    parser.add_argument("--artifacts",type=Path,required=True)
    options,arguments = parser.parse_known_args()
    ARTIFACTS = options.artifacts.resolve()
    ARTIFACTS.mkdir(parents=True,exist_ok=True)
    NATIVE = (options.claude or Path(shutil.which("claude") or "")).resolve()
    if not NATIVE.is_file(): parser.error("Install the public native Claude CLI for --claude-contract")
    version = subprocess.check_output([str(NATIVE),"--version"],text=True).strip()
    (ARTIFACTS/"native-version.txt").write_text(version+"\n")
    BINARY = package(options.binary.resolve(),options.helper.resolve(),ARTIFACTS)
    print("Packaged native Claude contracts:",version,"Artifacts:",ARTIFACTS,flush=True)
    unittest.main(argv=[__file__,*arguments])
