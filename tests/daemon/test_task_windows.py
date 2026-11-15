"""Read older native task windows through actual sockets with private captured contexts."""

from support import wait_for


class TaskWindowTests:
    def test_older_windows_share_the_latest_native_byte_allowance_and_keep_latest_rows(self):
        fixture, first, other, session = self.task_clients("task-window-shared-bytes", count=257)
        fixture.native.task_agents[0]["name"] = "L" * (4 * 1024 * 1024)
        fixture.native.task_agents[256]["name"] = "O" * (4 * 1024 * 1024)
        first.call("task/list", session=session, refresh=True)
        latest = self.known_tasks(first, session)
        self.assertFalse(latest["tasks"][0]["label_complete"])
        header = first.call("task/window/open", session=session, expected=latest["revision"], after=None)
        failed = first.call("task/window/read", session=session, window=header["window"])
        self.assertFalse(failed["known"])
        self.assertIn("shared 8 MiB", failed["read_error"])
        self.assertEqual(other.call("task/list", session=session)["tasks"], latest["tasks"])
        facts = other.call("session/subscribe", session=session, since=None)["snapshot"]["tasks"]
        self.assertEqual(facts["counts"]["agents"], 256)
        self.assertTrue(facts["known"])
        first.call("task/window/close", session=session, window=header["window"])
        fixture.native.task_agents[256]["name"] = "Smaller older task"
        accepted = self.older_window(first, session, latest["revision"])
        self.assertEqual(accepted["rows"], 1)
        self.assertEqual(first.call("task/list", session=session, window=accepted["window"])["tasks"][0]["label"], "Smaller older task")

    def older_window(self, client, session, expected, after=None):
        pending = client.call("task/window/open", session=session, expected=expected, after=after)
        self.assertTrue(pending["reading"])
        ready = client.call("task/window/read", session=session, window=pending["window"])
        self.assertTrue(ready["known"])
        self.assertFalse(ready["reading"])
        return ready

    def test_older_native_task_windows_are_private_and_do_not_replace_latest_facts(self):
        fixture, first, other, session = self.task_clients("task-private-windows", count=600)
        latest = self.known_tasks(first, session)
        a = self.older_window(first, session, latest["revision"])
        b = self.older_window(other, session, latest["revision"])
        self.assertNotEqual(a["window"], b["window"])
        self.assertEqual((a["rows"], b["rows"]), (256, 256))
        ap = first.call("task/list", session=session, window=a["window"], expected=a["revision"])
        bp = other.call("task/list", session=session, window=b["window"], expected=b["revision"])
        self.assertEqual(ap["tasks"], bp["tasks"])
        self.assertEqual(ap["tasks"][0]["label"], "Child 256")
        stolen = other.response(other.send("task/detail", session=session, window=a["window"], expected=a["revision"], task=ap["tasks"][0]["id"]))
        self.assertIn("foreign", stolen["error"]["message"])
        fixture.native.task_agents[0]["name"] = "Latest renamed"
        fixture.native.task_agents[256]["name"] = "Older renamed later"
        first.call("task/list", session=session, refresh=True)
        current = self.known_tasks(other, session)
        self.assertEqual(current["tasks"][0]["label"], "Latest renamed")
        frozen = first.call("task/detail", session=session, window=a["window"], expected=a["revision"], task=ap["tasks"][0]["id"])
        self.assertEqual(frozen["task"]["label"], "Child 256")
        older = self.older_window(first, session, a["revision"], a["window"])
        self.assertEqual(older["rows"], 88)
        last = first.call("task/list", session=session, window=older["window"], expected=older["revision"])
        self.assertEqual(last["tasks"][0]["label"], "Child 512")
        first.call("task/window/close", session=session, window=a["window"])
        self.assertIn("error", first.response(first.send("task/window/read", session=session, window=a["window"])))
        self.assertEqual(other.call("task/list", session=session, window=b["window"])["tasks"], bp["tasks"])
        first.close()
        self.assertTrue(other.call("task/window/read", session=session, window=b["window"])["known"])
        methods = [request.get("method") for request in fixture.native.requests]
        self.assertFalse(any(method in methods for method in ("turn/start", "turn/interrupt", "thread/backgroundTerminals/terminate")))

    def test_held_older_native_task_read_does_not_block_other_clients_and_keeps_original_cursor(self):
        fixture, first, other, session = self.task_clients("task-held-window", count=270)
        latest = self.known_tasks(first, session)
        fixture.native.task_window_hold = True
        header = first.call("task/window/open", session=session, expected=latest["revision"], after=None)
        wait_for(lambda: fixture.native.task_window_pending, "original native older cursor held")
        read = first.send("task/window/read", session=session, window=header["window"])
        self.assertTrue(other.call("sessions/watch")["sessions"])
        self.assertEqual(other.call("task/list", session=session)["tasks"], latest["tasks"])
        fixture.native.release_task_window()
        complete = first.response(read)["result"]
        self.assertEqual(complete["rows"], 14)
        self.assertEqual(complete["window"], header["window"])
        requests = [request for request in fixture.native.requests if request.get("method") == "thread/list"]
        self.assertTrue(any(request["params"].get("cursor") == "task:256" for request in requests))
        self.assertTrue(all(request["params"].get("limit") == 1 for request in requests))

    def test_older_process_pid_replacement_never_sends_a_native_stop(self):
        fixture, first, _, session = self.task_clients("task-window-pid", count=257)
        fixture.native.task_agents[256]["status"] = {"type":"active"}
        command = "é🌍" * 14000 + "OLDER_COMMAND_END"
        process = {"processId":"old-process", "itemId":"original-item", "osPid":123, "command":command}
        fixture.native.task_processes["native-child-256"] = [process]
        latest = self.known_tasks(first, session)
        window = self.older_window(first, session, latest["revision"])
        page = first.call("task/list", session=session, window=window["window"])
        task = next(row for row in page["tasks"] if row["kind"] == "process")
        offset, parts = "0", []
        while True:
            text = first.call("task/text", session=session, window=window["window"], expected=window["revision"], task=task["id"], field="command", offset=offset)
            self.assertEqual(text["offset"], offset)
            self.assertLessEqual(len(text["text"].encode()), 65536)
            parts.append(text["text"])
            offset = text["next_offset"]
            if text["complete"]:
                break
        self.assertGreater(len(parts), 1)
        self.assertEqual("".join(parts), command)
        fixture.native.task_processes["native-child-256"] = [{**process, "osPid":456}]
        original = first.call("task/run", session=session, window=window["window"], task=task["id"], refresh=True)
        def settled():
            run = first.call("task/run", session=session, window=window["window"], task=task["id"], read=original["read"])
            return run if not run["reading"] else None
        refused = wait_for(settled, "original PID proof rejects replacement")
        self.assertIn("not retargeted", refused["error"])
        self.assertIsNone(refused["run"])
        operation = first.operation(2)
        first.call("task/stop", session=session, window=window["window"], operation=operation, task=task["id"], expected_run=task["run"])
        self.assertEqual(first.outcome(operation)["state"]["kind"], "rejected")
        self.assertFalse(any(request.get("method") == "thread/backgroundTerminals/terminate" for request in fixture.native.requests))
