"""Keep current native file-change and MCP data through the actual daemon."""

import copy
import json

from native_history_windows import NativeHistoryWindows
from native_items import MCP, PATCH, NativeItems
from support import Client, Fixture, wait_for


class NativeItemTests:
    def native_item_session(self, name, history=False, items=None, hold=False):
        fixture = Fixture("native-items-" + name, codex=True)
        self.addCleanup(fixture.close)
        peer = NativeItems(fixture.native, items, hold)
        if history:
            windows = NativeHistoryWindows(fixture.native, turns=1)
            windows.turns[0]["items"] = peer.items
        client = Client(fixture, "native-items-reader")
        self.addCleanup(client.close)
        opened = client.operation(1)
        if history:
            session = client.call("session/open", operation=opened, backend="codex", cwd=str(fixture.root),
                                  target={"kind":"resume", "address":fixture.native.thread_id})["session"]
        else:
            session = client.call("session/create", operation=opened, backend="codex", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(opened)["state"]["kind"], "accepted")
        if not history:
            sent = client.operation(2)
            client.call("session/send", session=session, operation=sent, text="native items")
            self.assertEqual(client.outcome(sent)["state"]["kind"], "accepted")
        return fixture, peer, client, session

    def native_item_rows(self, client, session):
        return client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]

    def native_item_source(self, client, session, row):
        return client.call("entry/source", session=session, entry=row["id"], cursor=None, limit=65536)["text"]

    def check_native_item_data(self, client, session, rows):
        patches = [row for row in rows if row.get("file_path") in ("src/one.rs", "src/two.rs")]
        self.assertEqual(len(patches), 2, "native FileChange must retain both declared diffs")
        for row, change, counts in zip(patches, PATCH["changes"], [(1, 1), (1, 0)]):
            self.assertEqual((row["kind"], row["format"]), ("tool", "diff"))
            self.assertEqual((row["tool"]["kind"], row["tool"]["status"]), ("edit", "ok"))
            self.assertEqual((row["tool"]["added"], row["tool"]["removed"]), counts)
            self.assertEqual(self.native_item_source(client, session, row), change["diff"])
        mcp = next(row for row in rows if "Lookup" in (row.get("tool") or {}).get("label", ""))
        self.assertEqual((mcp["kind"], mcp.get("format"), mcp["tool"]["status"]), ("tool", None, "ok"))
        self.assertEqual(json.loads(self.native_item_source(client, session, mcp)), MCP,
                         "MCP nested diff/resource/arguments must not replace the complete native record")

    def test_native_file_changes_and_mcp_results_keep_complete_live_source(self):
        fixture, _, client, session = self.native_item_session("live")
        def received():
            rows = self.native_item_rows(client, session)
            return rows if any(row.get("tool") for row in rows) else None
        rows = wait_for(received, "native tools arrive")
        self.check_native_item_data(client, session, rows)
        self.assertEqual(fixture.native.prompts, 1)

    def test_native_file_changes_and_mcp_results_keep_complete_history_source(self):
        fixture, _, client, session = self.native_item_session("history", history=True)
        self.check_native_item_data(client, session, self.native_item_rows(client, session))
        self.assertEqual(fixture.native.prompts, 0)

    def test_running_native_mcp_keeps_arguments_and_updates_the_same_entry_with_its_result(self):
        running = {**MCP, "status":"inProgress", "result":None}
        _, peer, client, session = self.native_item_session("running", items=[running], hold=True)
        def pending():
            return next((row for row in self.native_item_rows(client, session)
                         if (row.get("tool") or {}).get("status") == "running"), None)
        before = wait_for(pending, "native MCP request stays expandable while running")
        self.assertEqual(json.loads(self.native_item_source(client, session, before)), running)
        peer.finish()
        after = wait_for(lambda: next((row for row in self.native_item_rows(client, session)
                         if row["id"] == before["id"] and row["done"]), None), "native MCP result settled")
        self.assertEqual(after["tool"]["status"], "ok")
        self.assertEqual(json.loads(self.native_item_source(client, session, after)), MCP)

    def test_native_failed_empty_patch_and_mcp_error_remain_visible(self):
        patch = {**PATCH, "status":"failed", "changes":[]}
        mcp = copy.deepcopy(MCP)
        mcp.update(status="failed", result=None, error={"message":"native lookup failed 🌍"})
        _, _, client, session = self.native_item_session("failed", history=True, items=[patch, mcp])
        rows = [row for row in self.native_item_rows(client, session) if row.get("tool")]
        self.assertEqual(len(rows), 2, "a failed patch with no diff is still a native result")
        for row, expected in zip(rows, [patch, mcp]):
            self.assertEqual(row["tool"]["status"], "failed")
            self.assertEqual(json.loads(self.native_item_source(client, session, row)), expected)
