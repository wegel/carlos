"""Keep input and native callback identities safe while reconnect reads are pending or fail."""

import json
import os
import time


class CodexReconnect:
    def disconnect_codex_fixture(self,terminal):
        terminal.paste("DISCONNECT")
        terminal.send(b"\r")
        terminal.contains("Codex stopped")
        terminal.wait(lambda:terminal.input_is("DISCONNECT"),"uncertain prompt draft recovered")

    def answer_reconnect_hook(self,terminal):
        terminal.send(b"\x1b[15~")
        terminal.contains("Reconnect question")
        terminal.paste("7")
        terminal.send(b"\x13")
        terminal.wait(lambda:any(row.get("id")==9 and row.get("result")=={"action":"accept","content":{"count":7}} for row in terminal.messages()),"callback answered before initialize reply")
        rows=terminal.messages()
        initialize=max(index for index,row in enumerate(rows) if row.get("method")=="initialize")
        answered=next(index for index,row in enumerate(rows) if index>initialize and row.get("id")==9 and "result" in row)
        self.assertTrue(all("method" not in row for row in rows[initialize+1:answered]),"no quota/read is sent before initialize settles")
        methods=[row.get("method") for row in rows]
        self.assertEqual(methods.count("turn/steer"),0)

    def test_codex_reconnect_keeps_drafts_uses_current_fork_and_replays_full_items_without_duplicates(self):
        terminal=self.launch(mode="codex-reconnect",width=100)
        terminal.contains("Answer: second")
        self.edit(terminal,"second","edited fork first")
        fork=terminal.requests("turn/start")[-1]["params"]["threadId"]
        self.disconnect_codex_fixture(terminal)
        terminal.replace_input("keep main 日本語")
        self.answer_reconnect_hook(terminal)
        terminal.wait(lambda:len(terminal.requests("thread/turns/list"))>=3,"asynchronous history read started")
        terminal.paste(" typed while reading")
        terminal.wait(lambda:terminal.input_is("keep main 日本語 typed while reading"),"typing remains usable while history waits")
        sends=len(terminal.requests("turn/start"))
        terminal.send(b"\r")
        self.assertEqual(len(terminal.requests("turn/start")),sends)
        (terminal.root/"release-history").write_text("ready")
        terminal.contains("Codex reconnected")
        terminal.contains("prefix suffix FINAL")
        self.assertEqual(terminal.text.count("prefix suffix FINAL"),1)
        self.assertNotIn("prefix suffix suffix",terminal.text)
        self.assertEqual(terminal.requests("thread/resume")[-1]["params"]["threadId"],fork)
        terminal.wait(lambda:terminal.input_is("keep main 日本語 typed while reading"),"draft survived atomic rebuild")
        terminal.send(b"\x1bq")
        terminal.contains("PERSISTED_QUEUE")
        terminal.send(b"\x1b")
        terminal.wait(lambda:"Codex queue" not in terminal.text,"queue closed before composing")
        terminal.replace_input("after reconnect")
        terminal.send(b"\r")
        terminal.contains("Answer: after reconnect")
        request=terminal.requests("turn/start")[-1]
        self.assertEqual(request["params"]["threadId"],fork)
        self.assertNotIn("model",request["params"])
        self.assertNotIn("effort",request["params"])
        self.assertEqual(sum(any(part.get("text")=="DISCONNECT" for part in row["params"].get("input",[])) for row in terminal.requests("turn/start")),1)
        terminal.snapshot("reconnected-current-fork-and-one-final-item")

    def test_codex_failed_reconnect_keeps_old_history_and_phone_draft_until_explicit_retry(self):
        terminal=self.launch(mode="codex-reconnect",width=54)
        terminal.contains("Answer: second")
        self.disconnect_codex_fixture(terminal)
        terminal.replace_input("recover phone draft 日本語")
        (terminal.root/"fail-history").write_text("fail")
        (terminal.root/"release-history").write_text("ready")
        self.answer_reconnect_hook(terminal)
        terminal.contains("Read Codex")
        terminal.contains("history failure")
        terminal.contains("Answer: second")
        terminal.wait(lambda:terminal.input_is("recover phone draft 日本語"),"failed reconnect preserves main draft")
        self.assertNotIn("prefix suffix FINAL",terminal.text)
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        terminal.snapshot("failed-reconnect-atomic-history-and-phone-draft")

    def test_codex_same_native_ids_in_different_turns_keep_both_rows_and_raw_rewind_anchor(self):
        terminal=self.launch(mode="codex-reused-items",width=100)
        terminal.contains("Answer: second")
        terminal.contains("Answer: first")
        self.edit(terminal,"second","edit exact second turn")
        self.assertEqual(terminal.requests("thread/fork")[0]["params"]["beforeTurnId"],"turn-second")
        reads=terminal.requests("thread/items/list")
        self.assertTrue(any(row["params"].get("turnId")=="turn-second" for row in reads))
        self.assertTrue(any(entry.get("item",{}).get("id")=="same-user" for row in terminal.messages("server") for entry in row.get("result",{}).get("data",[]) if isinstance(entry,dict)))
        self.assertNotIn("codex:[",json.dumps(terminal.messages()))
        self.assertIn("Answer: first",terminal.text)
        self.assertNotIn("Answer: second",terminal.text)
        terminal.snapshot("same-native-ids-exact-turn-rewind")

    def test_codex_owned_reconnect_reuses_cached_flags_and_quit_ends_its_only_child(self):
        terminal=self.launch(mode="codex-reconnect",width=100)
        terminal.contains("Answer: second")
        self.disconnect_codex_fixture(terminal)
        terminal.send(b"\x1b[15~")
        terminal.contains("Reconnect question")
        child=int((terminal.root/"connection-pid-2").read_text())
        started=time.monotonic()
        terminal.close()
        self.assertLess(time.monotonic()-started,2)
        self.assertEqual((terminal.root/"feature-probes").read_text(),"1")
        self.assertFalse((terminal.root/"probe-stalled-pid").exists())
        with self.assertRaises(ProcessLookupError): os.kill(child,0)
