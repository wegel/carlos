"""Keep browser consent separate from copied URLs and share clipboard write order."""

import json

from native_task_questions import form
from test_daemon_client import daemon_support
from test_daemon_task_questions import F4


class DaemonQuestionBrowser:
    def held_question_clipboard(self, name):
        root = self.clipboard_root(name)
        program = root / "bin/wl-copy"
        program.write_text("""#!/usr/bin/env python3
import os,sys,time
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
text=sys.stdin.buffer.read()
marker=root/'clipboard-entered'
if not marker.exists():
    marker.touch()
    while not (root/'clipboard-release').exists(): time.sleep(.005)
(root/'copied.txt').write_bytes(text)
with (root/'clipboard-results.jsonl').open('a') as log: log.write(__import__('json').dumps(text.decode())+'\\n')
""")
        return root

    def test_daemon_browser_url_copy_shares_clipboard_epoch_with_held_source_in_both_orders(self):
        for url_first in (False, True):
            name = "question-browser-url-first" if url_first else "question-browser-source-first"
            body = "bounded source line\n" * 5000 + "Original source 日本語 🌍"
            fixture, client, session = self.daemon_session(name, history=lambda:{
                "id":"native-root", "status":{"type":"idle"}, "model":"fixture-model", "reasoningEffort":"low", "turns":[{
                    "id":"old-answer", "status":"completed", "items":[{"type":"agentMessage", "id":"source", "phase":"final_answer", "text":body}]}]})
            root = self.held_question_clipboard(name)
            self.addCleanup(lambda path=root/"clipboard-release":path.touch())
            terminal = self.attach_daemon(fixture, session, name, extra_env={"SSH_TTY":"", "SSH_CONNECTION":"", "SSH_CLIENT":""})
            terminal.contains("Original source 日本語 🌍")
            terminal.paste("composer before browser copy")
            terminal.wait(lambda:terminal.input_is("composer before browser copy"), "composer before held clipboard")
            browser = form("original-browser", mode="url")
            url = browser["params"]["url"]
            def show_browser():
                fixture.native.emit_question(browser)
                original = self.question_cards(client, session)[0]
                terminal.contains("URL host: native-forms.test")
                terminal.send(b"\x19")
                terminal.contains("Copying the original browser URL")
                return original
            if url_first:
                original = show_browser()
                terminal.wait(lambda:(root/"clipboard-entered").exists(), "URL clipboard helper held")
                fixture.native.resolve_question("original-browser")
                terminal.wait(lambda:terminal.input_is("composer before browser copy"), "retired browser reveals original composer")
                terminal.send(b"\x19")
            else:
                terminal.send(b"\x19")
                terminal.wait(lambda:(root/"clipboard-entered").exists(), "source clipboard helper held")
                terminal.paste(" while clipboard waits")
                terminal.wait(lambda:terminal.input_is("composer before browser copy while clipboard waits"), "typing while source clipboard is held")
                original = show_browser()
                terminal.send(b"\x19")
                terminal.contains("URL copy is still pending")
            self.assertEqual(terminal.requests("session/approve"), [])
            (root/"clipboard-release").touch()
            terminal.wait(lambda:(root/"clipboard-results.jsonl").exists() and len((root/"clipboard-results.jsonl").read_text().splitlines()) == 2, "newest clipboard intent finishes after old helper")
            results = [json.loads(line) for line in (root/"clipboard-results.jsonl").read_text().splitlines()]
            expected = [url, body] if url_first else [body, url]
            self.assertEqual(results, expected)
            self.assertEqual((root/"copied.txt").read_text(), expected[-1])
            if not url_first:
                terminal.contains("Browser link copied")
                terminal.send(b"y")
                daemon_support.wait_for(lambda:fixture.native.task_answers, "consent requires a separate explicit key")
                self.assertEqual(fixture.native.task_answers, [{"id":"original-browser", "result":{"action":"accept"}}])
                self.assertEqual(terminal.requests("session/approve")[-1]["params"]["approval"], original["id"])
            else:
                self.assertEqual(fixture.native.task_answers, [])
            self.assertEqual(terminal.requests("session/send"), [])
            terminal.snapshot(name)
            terminal.close()

    def test_daemon_claude_early_browser_completion_keeps_explicit_consent(self):
        fixture, client, session, terminal = self.claude_question_session("claude-browser-early", "browser early")
        terminal.contains("Your consent is still required")
        terminal.contains("URL host: auth.example.com")
        original = self.question_cards(client, session)[0]
        self.assertEqual(self.claude_callback_answers(fixture), {})
        terminal.send(b"y")
        terminal.contains("Native callbacks settled")
        self.assertEqual(self.claude_callback_answers(fixture)["browser-question"], {"action":"accept"})
        self.assertEqual(terminal.requests("session/approve")[-1]["params"]["approval"], original["id"])
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("daemon-browser-early-explicit-consent")
