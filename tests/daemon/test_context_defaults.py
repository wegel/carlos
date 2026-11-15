#!/usr/bin/env python3
"""Check host context defaults through the actual daemon without native size mutations."""

import argparse
import json
import os
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class ContextDefaultTests:
    def _context_fixture(self, name, codex=False, mode="modern", shell=None):
        fixture = Fixture("context-" + name, codex=codex, mode=mode)
        self.addCleanup(fixture.close)
        fixture.stop()
        fixture.env.pop("CLAUDE_CODE_AUTO_COMPACT_WINDOW", None)
        fixture.env.pop("CARLOS_REASONING_SUMMARY", None)
        if shell is not None:
            fixture.env["CLAUDE_CODE_AUTO_COMPACT_WINDOW"] = shell
        fixture.start()
        return fixture

    def _context_client(self, fixture, name="first"):
        client = Client(fixture, name)
        self.addCleanup(client.close)
        return client

    def _context_session(self, fixture, client, backend="claude", counter=1):
        operation = client.operation(counter)
        session = client.call("session/create", operation=operation, backend=backend,
                              cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        def ready():
            page = client.call("session/subscribe", session=session, since=None)["snapshot"]
            return page if page["info"]["availability"] == "ready" and not page["settings"]["native_pending"] else None
        wait_for(ready, "native startup settles before context-default checks")
        return session

    def _context_path(self, fixture, backend):
        path = Path(fixture.env["XDG_CONFIG_HOME"]) / f"carlos/{backend}-context.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        return path

    def _context_choice(self, tokens):
        return {"kind":"auto"} if tokens is None else {"kind":"tokens", "tokens":tokens}

    def _context_write(self, client, session, counter, tokens):
        operation = client.operation(counter)
        client.call("settings/context/set", session=session, operation=operation,
                    choice=self._context_choice(tokens))
        return operation, client.outcome(operation)

    def _context_assert_no_live_mutation(self, fixture):
        records = [json.loads(line) for line in (fixture.root / "claude-controller.jsonl").read_text().splitlines()]
        methods = [row["message"].get("method") for row in records if row["direction"] == "client"]
        self.assertEqual(methods.count("start"), 1, "saving a default cannot restart the running helper")
        self.assertFalse(any(method in ("prompt", "model.set", "effort.set") for method in methods))

    def test_two_clients_keep_private_results_and_old_retries_do_not_rewrite_new_defaults(self):
        fixture = self._context_fixture("private")
        first, second = self._context_client(fixture), self._context_client(fixture, "second")
        session = self._context_session(fixture, first)
        self.assertEqual(first.call("settings/context/read", session=session),
                         {"session":session, "saved_tokens":None, "policy":"owned"})
        operation, saved = self._context_write(first, session, 2, 500_000)
        self.assertEqual(saved["state"]["kind"], "accepted")
        _, newer = self._context_write(second, session, 1, 750_000)
        self.assertEqual(newer["state"]["kind"], "accepted")
        retry = first.call("settings/context/set", session=session, operation=operation,
                           choice=self._context_choice(500_000))
        self.assertEqual(retry["state"]["kind"], "accepted")
        self.assertEqual(first.call("settings/context/read", session=session)["saved_tokens"], 750_000)
        path = self._context_path(fixture, "claude")
        self.assertEqual(json.loads(path.read_text()), {"tokens":750_000})
        first.close()
        # Reconnect the same private identity rather than create another retry namespace.
        resumed = Client(fixture, "private-resume", resume=first.hello)
        self.addCleanup(resumed.close)
        self.assertTrue(resumed.hello["resumed"])
        self.assertEqual(resumed.call("operation/read", operation=operation)["state"]["kind"], "accepted")
        resumed.call("settings/context/set", session=session, operation=operation, choice=self._context_choice(500_000))
        self.assertEqual(json.loads(path.read_text()), {"tokens":750_000})
        self.assertTrue(all(message["params"]["operation"]["client"] == second.hello["client"]
                            for message in second.messages if message.get("method") == "operation/updated"))
        self._context_assert_no_live_mutation(fixture)
        self.assertFalse(self._context_path(fixture, "codex").exists())

    def test_owned_codex_applies_a_saved_size_only_when_another_native_process_opens(self):
        fixture = self._context_fixture("codex-launch", mode="codex-current")
        client = self._context_client(fixture)
        session = self._context_session(fixture, client, "codex")
        before = client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["context"]
        launches = lambda: [row for row in fixture.pids() if row["backend"] == "codex"
                            and "app-server" in row["argv"] and "--help" not in row["argv"]]
        self.assertEqual(len(launches()), 1)
        self.assertFalse(any(arg.startswith("model_context_window=") for arg in launches()[0]["argv"]))
        _, saved = self._context_write(client, session, 2, 500_000)
        self.assertEqual(saved["state"]["kind"], "accepted")
        self.assertEqual(len(launches()), 1, "save cannot restart the active native process")
        after = client.call("session/subscribe", session=session, since=None)["snapshot"]["info"]["context"]
        self.assertEqual(after, before, "a next-launch default is separate from live token usage")
        self._context_session(fixture, client, "codex", counter=3)
        self.assertEqual(len(launches()), 2)
        self.assertIn("model_context_window=500000", launches()[-1]["argv"])
        self.assertIn("model_auto_compact_token_limit=450000", launches()[-1]["argv"])
        self.assertFalse(fixture.native_requests("config/value/write"))
        self.assertFalse(fixture.native_requests("config/batchWrite"))

    def test_attached_codex_and_unsupported_grok_never_read_or_replace_the_saved_file(self):
        for backend, attached, policy in [("codex", True, "attached_codex"), ("grok", False, "unsupported")]:
            with self.subTest(backend=backend):
                fixture = self._context_fixture(backend, codex=attached)
                path = self._context_path(fixture, backend)
                path.write_text("invalid saved JSON")
                client = self._context_client(fixture)
                session = self._context_session(fixture, client, backend)
                self.assertEqual(client.call("settings/context/read", session=session),
                                 {"session":session, "saved_tokens":None, "policy":policy})
                _, rejected = self._context_write(client, session, 2, 500_000)
                self.assertEqual(rejected["state"]["kind"], "rejected")
                self.assertEqual(path.read_text(), "invalid saved JSON")
                self.assertFalse(fixture.native_requests("config/value/write"))

    def test_claude_shell_override_rejects_explicit_sizes_but_allows_auto(self):
        fixture = self._context_fixture("shell", shell="200000")
        client = self._context_client(fixture)
        session = self._context_session(fixture, client)
        path = self._context_path(fixture, "claude")
        path.write_text(json.dumps({"tokens":500_000}))
        self.assertEqual(client.call("settings/context/read", session=session),
                         {"session":session, "saved_tokens":500_000, "policy":"claude_shell_override"})
        _, rejected = self._context_write(client, session, 2, 750_000)
        self.assertEqual(rejected["state"]["kind"], "rejected")
        self.assertIn("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW", rejected["state"]["message"])
        self.assertEqual(json.loads(path.read_text()), {"tokens":500_000})
        _, saved = self._context_write(client, session, 3, None)
        self.assertEqual(saved["state"]["kind"], "accepted")
        self.assertEqual(json.loads(path.read_text()), {"tokens":None})
        self.assertEqual(client.call("settings/context/read", session=session)["saved_tokens"], None)
        self._context_assert_no_live_mutation(fixture)

    def test_malformed_files_invalid_sizes_and_disk_failures_remain_explicit(self):
        fixture = self._context_fixture("errors")
        client = self._context_client(fixture)
        session = self._context_session(fixture, client)
        path = self._context_path(fixture, "claude")
        path.write_text("broken JSON")
        error = client.response(client.send("settings/context/read", session=session))
        self.assertIn("Read Carlos context settings", error["error"]["message"])
        for counter, tokens in [(2, 99_999), (3, 1_000_001)]:
            _, result = self._context_write(client, session, counter, tokens)
            self.assertEqual(result["state"]["kind"], "rejected")
            self.assertIn("Choose Auto or 100k to 1M", result["state"]["message"])
            self.assertEqual(path.read_text(), "broken JSON")
        missing = client.response(client.send("settings/context/set", session=session, operation=client.operation(4)))
        self.assertIn("error", missing, "omitted choice must not silently mean Auto")
        path.unlink()
        path.mkdir()
        (path / "marker").write_text("keep")
        _, failure = self._context_write(client, session, 5, 500_000)
        self.assertEqual(failure["state"]["kind"], "rejected")
        self.assertIn("Save Carlos context settings", failure["state"]["message"])
        self.assertEqual((path / "marker").read_text(), "keep")
        self._context_assert_no_live_mutation(fixture)

    def test_blocked_context_read_keeps_manager_owner_and_ordinary_prompt_work_available(self):
        fixture = self._context_fixture("blocked-read")
        first, second = self._context_client(fixture), self._context_client(fixture, "second")
        session = self._context_session(fixture, first)
        path = self._context_path(fixture, "claude")
        os.mkfifo(path)
        identity = first.send("settings/context/read", session=session)
        descriptor = None
        try:
            def writer_ready():
                try:
                    return os.open(path, os.O_WRONLY | os.O_NONBLOCK)
                except OSError:
                    return None
            descriptor = wait_for(writer_ready, "context reader entered the existing worker")
            self.assertTrue(second.call("sessions/watch")["sessions"])
            second.call("session/subscribe", session=session, since=None)
            operation = second.operation(1)
            second.call("session/send", session=session, operation=operation, text="prompt while file read waits")
            self.assertEqual(second.outcome(operation)["state"]["kind"], "accepted")
            self.assertFalse(any(row.get("id") == identity for row in first.messages), "read reply still waits for file bytes")
            os.write(descriptor, b'{"tokens":500000}')
        finally:
            if descriptor is not None:
                os.close(descriptor)
            path.unlink(missing_ok=True)
            path.write_text('{"tokens":500000}')
        reply = first.response(identity)
        self.assertEqual(reply["result"], {"session":session, "saved_tokens":500_000, "policy":"owned"})
        self.assertEqual(second.call("settings/context/read", session=session)["saved_tokens"], 500_000)

    def test_disconnected_file_reads_release_reply_allowance_before_later_clients_read(self):
        fixture = self._context_fixture("disconnect-reads")
        observer = self._context_client(fixture, "observer")
        session = self._context_session(fixture, observer)
        path = self._context_path(fixture, "claude")
        for index in range(8):
            with self.subTest(read=index):
                path.unlink(missing_ok=True)
                os.mkfifo(path)
                reader = self._context_client(fixture, f"reader-{index}")
                reader.send("settings/context/read", session=session)
                descriptor = None
                try:
                    def writer_ready():
                        try:
                            return os.open(path, os.O_WRONLY | os.O_NONBLOCK)
                        except OSError:
                            return None
                    descriptor = wait_for(writer_ready, "original read entered the worker")
                    reader.close()
                    observer.call("sessions/watch")
                    observer.call("session/subscribe", session=session, since=None)
                    os.write(descriptor, b'{"tokens":500000}')
                finally:
                    if descriptor is not None:
                        os.close(descriptor)
                    path.unlink(missing_ok=True)
                    path.write_text('{"tokens":500000}')
                def completed_read():
                    response = observer.response(observer.send("settings/context/read", session=session))
                    if "error" in response and response["error"]["code"] == -32010:
                        return None
                    self.assertNotIn("error", response)
                    return response["result"] if response["result"]["saved_tokens"] == 500_000 else None
                wait_for(completed_read, "worker and reply reservations released for later reads")


if __name__ == "__main__":
    class StandaloneContextDefaultTests(ContextDefaultTests, unittest.TestCase):
        pass

    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
