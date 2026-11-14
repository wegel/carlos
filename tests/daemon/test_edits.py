"""Check operation-owned rename and fork/send chains against real native adapters."""

import json
from pathlib import Path

from support import Client, Fixture, wait_for


class EditTests:
    def test_claude_fork_waits_for_its_own_native_acknowledgement_and_preserves_original_history(self):
        fixture = Fixture("claude-remote-edit")
        self.addCleanup(fixture.close)
        first = Client(fixture, "editor")
        observer = Client(fixture, "observer")
        self.addCleanup(first.close)
        self.addCleanup(observer.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="claude", cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        for counter, text in ((2, "first original"), (3, "second original")):
            operation = first.operation(counter)
            first.call("session/send", session=session, operation=operation, text=text)
            self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
            wait_for(lambda: any(entry["text"] == "Answer: " + text for entry in observer.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]), "native answer complete")
        wait_for(lambda: observer.call("sessions/watch")["sessions"][0]["phase"] == "idle", "native turn idle")
        snapshot = observer.call("session/subscribe", session=session, since=None)["snapshot"]
        selected = next(entry for entry in snapshot["entries"] if entry["text"] == "second original")
        native_id = fixture.saved()[0][2]
        path = next(Path(fixture.env["CLAUDE_CONFIG_DIR"]).rglob(native_id+".jsonl"))
        original = path.read_bytes()
        operation = first.operation(4)
        params = dict(session=session, operation=operation, entry=selected["id"], text="edited branch")
        first.call("session/rewind", **params)
        self.assertEqual(first.outcome(operation)["state"]["kind"], "accepted")
        wait_for(lambda: any(entry["text"] == "Answer: edited branch" for entry in observer.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]), "edited native answer")
        self.assertEqual(first.call("session/rewind", **params)["state"]["kind"], "accepted")
        self.assertEqual(path.read_bytes(), original)
        requests = [row["message"] for line in (fixture.root / "claude-controller.jsonl").read_text().splitlines() if (row := json.loads(line))["direction"] == "client"]
        self.assertEqual(sum(request.get("method") == "sessions.forkBefore" for request in requests), 1)
        self.assertEqual(sum(request.get("method") == "prompt" for request in requests), 3)
        self.assertFalse(any(message.get("method") == "operation/updated" for message in observer.messages))

    def test_names_and_edits_return_private_results_and_a_fork_sends_once(self):
        fixture = Fixture("remote-edits", codex=True)
        self.addCleanup(fixture.close)
        fixture.native.history_user = True
        first = Client(fixture, "editor")
        second = Client(fixture, "observer")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex", cwd=str(fixture.root), name=None)["session"]
        first.outcome(create)
        snapshot = second.call("session/subscribe", session=session, since=None)["snapshot"]
        older = second.call("session/history", session=session, before=snapshot["entries"][0]["id"], limit=100)
        prompt = next(row for row in older["entries"] if row["kind"] == "user")
        rename = first.operation(2)
        params = dict(session=session, operation=rename, name="Shared name")
        first.call("session/rename", **params)
        self.assertEqual(first.outcome(rename)["state"]["kind"], "accepted")
        self.assertEqual(first.call("session/rename", **params)["state"]["kind"], "accepted")
        self.assertEqual(sum(r.get("method") == "thread/name/set" for r in fixture.native.requests), 1)
        wait_for(lambda: second.call("sessions/watch")["sessions"][0]["name"] == "Shared name", "accepted shared name")
        invalid = first.operation(3)
        first.call("session/rewind", session=session, operation=invalid, entry=snapshot["entries"][-1]["id"], text="kept invalid draft")
        rejected = first.outcome(invalid)["state"]
        self.assertEqual(rejected["kind"], "rejected")
        self.assertEqual(rejected["recovery"]["text"], "kept invalid draft")
        self.assertFalse(any(r.get("method") == "thread/fork" for r in fixture.native.requests))
        edit = first.operation(4)
        params = dict(session=session, operation=edit, entry=prompt["id"], text="edited prompt")
        first.call("session/rewind", **params)
        self.assertEqual(first.outcome(edit)["state"]["kind"], "accepted")
        self.assertEqual(first.call("session/rewind", **params)["state"]["kind"], "accepted")
        self.assertEqual(sum(r.get("method") == "thread/fork" for r in fixture.native.requests), 1)
        prompts = [r for r in fixture.native.requests if r.get("method") == "turn/start"]
        self.assertEqual(len(prompts), 1)
        self.assertEqual(prompts[0]["params"]["threadId"], "native-fork")
        self.assertEqual(prompts[0]["params"]["input"][0]["text"], "edited prompt")
        second.call("sessions/watch")
        self.assertFalse(any(m.get("method") == "operation/updated" for m in second.messages))
        late = second.operation(1)
        second.call("session/rewind", session=session, operation=late, entry=prompt["id"], text="retired target")
        self.assertEqual(second.outcome(late)["state"]["kind"], "rejected")

    def test_unsupported_rename_rejects_before_native_dispatch(self):
        fixture = Fixture("unsupported-name")
        self.addCleanup(fixture.close)
        client = Client(fixture, "sender")
        self.addCleanup(client.close)
        create = client.operation(1)
        session = client.call("session/create", operation=create, backend="grok", cwd=str(fixture.root), name=None)["session"]
        client.outcome(create)
        rename = client.operation(2)
        client.call("session/rename", session=session, operation=rename, name="unsupported")
        self.assertEqual(client.outcome(rename)["state"]["kind"], "rejected")
        self.assertFalse(fixture.native_requests("thread/name/set"))
