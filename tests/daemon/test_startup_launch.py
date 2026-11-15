#!/usr/bin/env python3
"""Check first-launch choices through daemon sockets and actual native child processes."""

import argparse
from pathlib import Path
import unittest

import support
from support import Client, Fixture, wait_for


class StartupLaunchTests(unittest.TestCase):
    def _launch_fixture(self, name, backend="claude", shell=None):
        fixture = Fixture("startup-launch-" + name, codex=backend == "codex")
        self.addCleanup(fixture.close)
        fixture.stop()
        fixture.env.pop("CLAUDE_CODE_AUTO_COMPACT_WINDOW", None)
        fixture.env.pop("CARLOS_REASONING_SUMMARY", None)
        if shell is not None:
            fixture.env["CLAUDE_CODE_AUTO_COMPACT_WINDOW"] = shell
        fixture.start()
        client = Client(fixture, "launcher")
        self.addCleanup(client.close)
        return fixture, client

    def _launch_create(self, fixture, client, backend="claude", launch=None, counter=1):
        operation = client.operation(counter)
        result = client.call("session/create", operation=operation, backend=backend,
                             cwd=str(fixture.root), name=None, launch=launch or {})
        return result["session"], client.outcome(operation)

    def test_explicit_codex_endpoint_accepts_aliases_and_rejects_mismatches_before_native_open(self):
        fixture, client = self._launch_fixture("endpoint", "codex")
        alias = fixture.root / "native-alias.sock"
        alias.symlink_to(fixture.native.path)
        self.addCleanup(lambda: alias.unlink(missing_ok=True))
        _, accepted = self._launch_create(fixture, client, "codex", {"codex_socket":str(alias)})
        self.assertEqual(accepted["state"]["kind"], "accepted")
        self.assertEqual(fixture.native.started, 1)
        different = fixture.root / "other.sock"
        different.write_text("a different endpoint")
        _, rejected = self._launch_create(fixture, client, "codex", {"codex_socket":str(different)}, 2)
        self.assertEqual(rejected["state"]["kind"], "rejected")
        self.assertIn("differs", rejected["state"]["message"])
        self.assertEqual(fixture.native.started, 1)

    def test_explicit_claude_child_values_override_only_that_launch_and_context_policy_matches(self):
        for name, daemon, explicit, expected, policy in [
                ("inherited", "200000", None, "200000", "claude_shell_override"),
                ("cleared", "200000", "", "", "owned"),
                ("unicode", "", " \u2003é🦊 ", " \u2003é🦊 ", "claude_shell_override"),
                ("absent", None, None, None, "owned")]:
            with self.subTest(case=name):
                fixture, client = self._launch_fixture(name, shell=daemon)
                launch = {} if explicit is None else {"claude_context_window":explicit}
                session, accepted = self._launch_create(fixture, client, launch=launch)
                self.assertEqual(accepted["state"]["kind"], "accepted")
                children = [row for row in fixture.pids() if row["backend"] == "claude"]
                self.assertEqual(len(children), 1)
                self.assertEqual(children[0]["context_window"], expected)
                self.assertEqual(client.call("settings/context/read", session=session)["policy"], policy)

    def test_explicit_codex_summary_wins_for_that_launch_without_rewriting_host_defaults(self):
        fixture, client = self._launch_fixture("summary", "codex")
        fixture.native.settings_enabled = True
        session, accepted = self._launch_create(fixture, client, "codex", {"reasoning_summary":"detailed"})
        self.assertEqual(accepted["state"]["kind"], "accepted")
        def applied():
            facts = client.call("settings/read", session=session)["settings"]
            return facts if not facts["native_pending"] and facts["current"]["summary"] == "detailed" else None
        wait_for(applied, "explicit summary accepted")
        native = [row["params"] for row in fixture.native.requests if row.get("method") == "thread/settings/update"]
        self.assertTrue(any(row.get("summary") == "detailed" for row in native), native)
        self.assertFalse((Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/runtime-defaults.json").exists())

    def test_claude_fork_replacement_keeps_the_original_explicit_child_environment(self):
        fixture, client = self._launch_fixture("fork", shell="200000")
        session, accepted = self._launch_create(fixture, client, launch={"claude_context_window":""})
        self.assertEqual(accepted["state"]["kind"], "accepted")
        for counter, text in ((2, "first saved prompt"), (3, "second saved prompt")):
            operation = client.operation(counter)
            client.call("session/send", session=session, operation=operation, text=text)
            self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
            wait_for(lambda: any(row["text"] == "Answer: " + text for row in
                     client.call("session/subscribe", session=session, since=None)["snapshot"]["entries"]), "saved answer")
        page = client.call("session/subscribe", session=session, since=None)["snapshot"]
        selected = next(row for row in page["entries"] if row["text"] == "second saved prompt")
        operation = client.operation(4)
        client.call("session/rewind", session=session, operation=operation, entry=selected["id"], text="forked prompt")
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        children = wait_for(lambda: (rows if len(rows := [row for row in fixture.pids()
                            if row["backend"] == "claude"]) == 2 else None), "replacement child started")
        self.assertEqual([row["context_window"] for row in children], ["", ""])
        self.assertEqual(client.call("settings/context/read", session=session)["policy"], "owned")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args, tests = parser.parse_known_args()
    support.BINARY = args.binary.resolve()
    support.ARTIFACTS = args.artifacts.resolve()
    unittest.main(argv=[__file__, *tests])
