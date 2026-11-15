"""Read one child's activity independently through actual daemon sockets and native pages."""

import json

from native_task_activity import child_item
from support import wait_for


class TaskActivityTests:
    def activity_clients(self, name, count=45):
        fixture, first, second, session = self.task_clients(name, count=2)
        fixture.native.task_items["native-child-0"] = [child_item(index) for index in range(count)]
        page = self.known_tasks(first, session)
        return fixture, first, second, session, page

    def open_activity(self, client, session, page):
        return client.call("task/activity/open", session=session, task=page["tasks"][0]["id"], expected=page["revision"])

    def ready_activity(self, client, session, header):
        while header["reading"]:
            header = client.call("task/activity/read", session=session, activity=header["activity"], since=header["change"])
        return header

    def test_child_activity_readers_on_same_child_keep_history_and_live_output_separate_from_root(self):
        fixture, first, second, session, page = self.activity_clients("child-independent")
        a = self.ready_activity(first, session, self.open_activity(first, session, page))
        b = self.ready_activity(second, session, self.open_activity(second, session, page))
        self.assertNotEqual(a["activity"], b["activity"])
        self.assertEqual((a["items"], b["items"]), (20,20))
        ap = first.call("task/activity/page", session=session, activity=a["activity"], expected=a["revision"])
        bp = second.call("task/activity/page", session=session, activity=b["activity"], expected=b["revision"])
        self.assertEqual([row["text"] for row in ap["records"]], [row["text"] for row in bp["records"]])
        self.assertIn("CHILD_DETAIL_ONLY_19", ap["records"][0]["text"])
        advanced = first.call("task/activity/advance", session=session, activity=a["activity"], older=True)
        a = self.ready_activity(first, session, advanced)
        self.assertEqual(a["items"], 40)
        self.assertEqual(second.call("task/activity/read", session=session, activity=b["activity"], since=None), b)
        foreign = second.response(second.send("task/activity/read", session=session, activity=a["activity"], since=None))
        self.assertIn("foreign", foreign["error"]["message"])
        fixture.native.child_delta("native-child-0", "PRIVATE_CHILD_LIVE🌍")
        live = second.call("task/activity/read", session=session, activity=b["activity"], since=b["change"])
        self.assertEqual(live["revision"], b["revision"])
        self.assertEqual(live["tail"], "PRIVATE_CHILD_LIVE🌍")
        root = first.call("session/subscribe", session=session, since=None)["snapshot"]
        self.assertNotIn("PRIVATE_CHILD_LIVE", json.dumps(root))
        self.assertNotIn("CHILD_DETAIL_ONLY", json.dumps(root))
        first.call("task/activity/close", session=session, activity=a["activity"])
        self.assertTrue(second.call("task/activity/read", session=session, activity=b["activity"], since=None)["known"])
        methods = [row.get("method") for row in fixture.native.requests]
        self.assertFalse(any(method in methods for method in ("turn/start", "turn/steer", "turn/interrupt")))
        self.assertTrue(all(row["params"]["limit"] == 1 for row in fixture.native.requests if row.get("method") == "thread/items/list"))

    def test_held_same_child_reads_settle_out_of_order_and_close_only_the_original_reader(self):
        fixture, first, second, session, page = self.activity_clients("child-held", count=21)
        fixture.native.task_item_hold.add("native-child-0")
        a, b = self.open_activity(first, session, page), self.open_activity(second, session, page)
        waiting = first.send("task/activity/read", session=session, activity=a["activity"], since=a["change"])
        wait_for(lambda:len(fixture.native.task_item_pending) == 2, "two native reads for the same child held")
        self.assertTrue(second.call("sessions/watch")["sessions"])
        fixture.native.release_child_items(last=True)
        b = self.ready_activity(second, session, b)
        self.assertEqual(b["items"],20)
        self.assertEqual(len(fixture.native.task_item_pending),1)
        first.call("task/activity/close", session=session, activity=a["activity"])
        self.assertIn("error", first.response(waiting))
        fixture.native.release_child_items()
        self.assertEqual(second.call("task/activity/read", session=session, activity=b["activity"], since=None), b)
        self.assertEqual([row for row in fixture.native.requests if row.get("method") == "turn/start"], [])

    def test_child_scalar_pages_keep_utf8_and_stop_before_the_wire_byte_bound(self):
        fixture, first, second, session, page = self.activity_clients("child-wire-bound", count=20)
        for index, row in enumerate(fixture.native.task_items["native-child-0"]):
            row["item"]["text"] = f"CHILD_SCALAR_{index}:" + "日本語🌍" * 4000
        header = self.ready_activity(first, session, self.open_activity(first, session, page))
        records, after, sizes = [], None, []
        while True:
            result = first.call("task/activity/page", session=session, activity=header["activity"],
                                expected=header["revision"], after=after, limit=128)
            sizes.append(len(json.dumps(result, ensure_ascii=False, separators=(",", ":")).encode()))
            self.assertTrue(result["records"])
            records.extend(result["records"])
            if not result["has_more"]:
                break
            after = result["records"][-1]["id"]
        self.assertGreater(len(sizes), 1)
        self.assertTrue(all(size < 384 * 1024 + 2048 for size in sizes))
        self.assertEqual(len(records), 20)
        self.assertEqual(len({record["id"] for record in records}), 20)
        self.assertTrue(all(not record["complete"] and len(record["text"].encode()) <= 32768 for record in records))
        self.assertIn("CHILD_SCALAR_19:", records[0]["text"])
        self.assertIn("CHILD_SCALAR_0:", records[-1]["text"])

    def test_native_connection_loss_retires_the_original_waiting_child_reader(self):
        fixture, first, second, session, page = self.activity_clients("child-native-eof", count=1)
        header = self.ready_activity(first, session, self.open_activity(first, session, page))
        waiting = first.send("task/activity/read", session=session, activity=header["activity"], since=header["change"])
        fixture.native.disconnect()
        settled = first.response(waiting)
        self.assertTrue("error" in settled or settled["result"]["closed"])
        self.assertEqual([row for row in fixture.native.requests if row.get("method") in ("turn/start", "turn/interrupt")], [])
