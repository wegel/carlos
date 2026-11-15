"""Drive independent child item readers through the actual F2 terminal and daemon."""

from test_daemon_tasks import F2
from test_daemon_client import daemon_support
from native_task_activity import child_item


class DaemonTaskActivity:
    def child_terminal(self, name, count=45, width=120):
        fixture, client, session, terminal = self.daemon_tasks(name, width=width)
        fixture.native.task_items["native-child-0"] = [child_item(index) for index in range(count)]
        return fixture, client, session, terminal

    def open_child(self, terminal):
        self.open_daemon_tasks(terminal)
        terminal.send(b"\r")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        terminal.wait(lambda:"Reading accepted child display pages" not in terminal.text, "accepted child activity")

    def test_daemon_f2_child_readers_keep_history_cursors_and_composers_private(self):
        fixture, client, session, first = self.child_terminal("child-two-ui")
        second = self.attach_daemon(fixture, session, "child-second", width=120)
        second.contains("history-output 9999")
        first.paste("first child draft 🌍")
        second.paste("second child draft 日本語")
        first.wait(lambda:first.input_is("first child draft 🌍"), "first composer")
        second.wait(lambda:second.input_is("second child draft 日本語"), "second composer")
        self.open_child(first)
        self.open_child(second)
        first.send(b"l")
        first.contains("CHILD_DETAIL_ONLY_39")
        second.contains("CHILD_DETAIL_ONLY_19")
        self.assertNotIn("CHILD_DETAIL_ONLY_39", second.text)
        a = first.requests("task/activity/open")[-1]["params"]
        b = second.requests("task/activity/open")[-1]["params"]
        self.assertEqual(a["task"], b["task"])
        ap = first.requests("task/activity/page")[-1]["params"]
        bp = second.requests("task/activity/page")[-1]["params"]
        self.assertNotEqual(ap["activity"], bp["activity"])
        first.send(b"\x1b")
        first.contains("Thread: native-child-0")
        fixture.native.child_delta("native-child-0", "CHILD_LIVE_ONLY🌍")
        second.send(b"\x1b[6~" * 20)
        second.contains("CHILD_LIVE_ONLY🌍")
        self.assertNotIn("CHILD_LIVE_ONLY", first.text)
        first.send(F2)
        second.send(F2)
        first.wait(lambda:first.input_is("first child draft 🌍"), "first draft after child close")
        second.wait(lambda:second.input_is("second child draft 日本語"), "second draft after child close")
        self.assertNotIn("CHILD_LIVE_ONLY", first.text)
        self.assertNotIn("CHILD_DETAIL_ONLY", second.text)
        self.assertEqual(first.requests("session/send") + second.requests("session/send"), [])
        self.assertEqual(first.requests("task/stop") + second.requests("task/stop"), [])
        native = [row for row in fixture.native.requests if row.get("method") == "thread/items/list"]
        self.assertTrue(native)
        self.assertTrue(all(row["params"]["threadId"] == "native-child-0" and row["params"]["limit"] == 1 for row in native))
        first.snapshot("child-first-composer")
        second.snapshot("child-second-composer")

    def test_daemon_f2_child_reload_failure_keeps_accepted_body_and_retries_explicitly(self):
        fixture, client, session, terminal = self.child_terminal("child-error-ui")
        terminal.paste("child error draft")
        self.open_child(terminal)
        fixture.native.task_item_errors.add("native-child-0")
        terminal.send(b"r")
        terminal.contains("PRIVATE_CHILD_READ_FAILED")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        terminal.wait(lambda:len(terminal.requests("task/activity/advance")) == 1, "one explicit reload")
        fixture.native.task_item_errors.clear()
        terminal.send(b"r")
        terminal.wait(lambda:len(terminal.requests("task/activity/advance")) == 2, "explicit retry")
        terminal.wait(lambda:"PRIVATE_CHILD_READ_FAILED" not in terminal.text, "native child retry accepted")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("child error draft"), "failure retains composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("child-error-explicit-retry")

    def test_daemon_f2_held_child_close_releases_only_that_read_and_preserves_selection(self):
        fixture, client, session, terminal = self.child_terminal("child-held-ui")
        fixture.native.task_item_hold.add("native-child-0")
        terminal.paste("held child draft")
        self.open_daemon_tasks(terminal)
        terminal.send(b"\r")
        terminal.contains("Reading child items")
        daemon_support.wait_for(lambda:fixture.native.task_item_pending, "native child read held")
        terminal.send(b"\x1b")
        terminal.contains("Thread: native-child-0")
        terminal.wait(lambda:terminal.requests("task/activity/close"), "original reader closed")
        fixture.native.release_child_items()
        terminal.contains("Thread: native-child-0")
        self.assertNotIn("CHILD_DETAIL_ONLY", terminal.text)
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("held child draft"), "held child close keeps composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("held-child-closed")

    def test_daemon_f2_older_child_activity_uses_captured_window_and_live_thread(self):
        fixture, client, session, terminal, _ = self.window_terminals("child-older-ui", count=257)
        fixture.native.task_items["native-child-256"] = [child_item(0, "OLDER_CHILD_ONLY")]
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"\r")
        terminal.contains("OLDER_CHILD_ONLY_0")
        opened = terminal.requests("task/activity/open")[-1]["params"]
        detail = terminal.requests("task/detail")[-1]["params"]
        self.assertEqual((opened["task"], opened["window"]), (detail["task"], detail["window"]))
        fixture.native.child_delta("native-child-256", "OLDER_CHILD_LIVE")
        terminal.contains("OLDER_CHILD_LIVE")
        native = [row["params"] for row in fixture.native.requests if row.get("method") == "thread/items/list"]
        self.assertEqual(native, [{"threadId":"native-child-256", "sortDirection":"desc", "limit":1, "cursor":None}])
        terminal.send(b"\x1b")
        terminal.contains("Thread: native-child-256")
        self.assertNotIn("OLDER_CHILD_LIVE", terminal.text)
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("older-child-captured-window")

    def test_daemon_f2_child_reconnect_keeps_display_until_explicit_reopen(self):
        fixture, client, session, terminal = self.child_terminal("child-reconnect-ui")
        terminal.paste("child reconnect draft")
        self.open_child(terminal)
        fixture.stop()
        terminal.contains("F5 reconnects")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        fixture.start()
        terminal.send(b"\x1b[15~")
        terminal.wait(lambda:len(terminal.requests("hello")) == 2, "replacement hello")
        terminal.contains("Child activity reader retired")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        def replacement_ready():
            messages = terminal.messages("server")
            hello = terminal.requests("hello")[-1]["id"]
            start = next((index + 1 for index, row in enumerate(messages) if row.get("id") == hello), len(messages))
            for row in messages[start:]:
                event = row.get("params", {}).get("event", {})
                result = row.get("result")
                snapshot = result.get("snapshot", {}) if isinstance(result, dict) else {}
                info = snapshot.get("info") or event.get("info") or event.get("snapshot", {}).get("info")
                if info and info["id"] == session and info["availability"] == "ready":
                    return True
            return False
        terminal.wait(replacement_ready, "fresh ready facts from replacement daemon")
        self.assertEqual(len(terminal.requests("task/activity/open")), 1)
        terminal.contains("Thread: native-child-0")
        terminal.send(b"\r")
        terminal.wait(lambda:len(terminal.requests("task/activity/open")) == 2, "explicit child reopen")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        terminal.send(F2)
        terminal.wait(lambda:terminal.input_is("child reconnect draft"), "reconnect retains composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("child-reconnect-explicit-reopen")

    def test_daemon_f2_child_close_then_open_keeps_original_rpc_order_on_a_phone(self):
        fixture, client, session, terminal, _ = self.window_terminals("child-close-open-ui", count=257)
        fixture.native.task_items["native-child-256"] = [child_item(index) for index in range(45)]
        terminal.paste("phone child draft 日本語")
        self.open_daemon_tasks(terminal)
        terminal.send(b"l")
        terminal.contains("Thread: native-child-256")
        terminal.send(b"\r")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        original = terminal.requests("task/activity/page")[-1]["params"]["activity"]
        terminal.send(b"\x1b[B\x1b[A\r")
        terminal.wait(lambda:len(terminal.requests("task/activity/open")) == 2, "immediate explicit reopen")
        terminal.wait(lambda:len(terminal.requests("task/activity/page")) >= 2, "new reader display accepted")
        terminal.contains("CHILD_DETAIL_ONLY_19")
        fresh = terminal.requests("task/activity/page")[-1]["params"]["activity"]
        self.assertNotEqual(original, fresh)
        methods = [row.get("method") for row in terminal.messages("client")]
        opens = [index for index, method in enumerate(methods) if method == "task/activity/open"]
        self.assertIn("task/activity/close", methods[opens[0]+1:opens[1]])
        terminal.resize(18, 10)
        terminal.contains("F2 back")
        terminal.send(F2)
        terminal.resize(100, 28)
        terminal.wait(lambda:terminal.input_is("phone child draft 日本語"), "phone child composer")
        self.assertEqual(terminal.requests("session/send"), [])
        terminal.snapshot("child-close-before-reopen-phone")
