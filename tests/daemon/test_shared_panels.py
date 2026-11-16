"""Drive the pure Rust panel consumer against the existing daemon/native fixtures."""

import json
import os
import subprocess


class SharedPanelTests:
    def test_shared_rust_panel_consumer_reads_complete_windows_and_rejects_retired_text(self):
        consumer = os.environ.get("CARLOS_PANEL_CONSUMER")
        if not consumer:
            self.skipTest("scripts/test.sh builds the matched Rust panel consumer")
        fixture, first, other, session = self.task_clients("shared-rust-panels", count=257)
        fixture.native.task_agents[256]["status"] = {"type":"active"}
        fixture.native.task_agents[256]["lastObservedTurn"] = {"id":"older-run", "status":"inProgress"}
        fixture.native.task_turns["native-child-256"] = [{"id":"older-run", "status":"inProgress"}]
        root_command = "read root 🌍 " * 8000 + "HEADLESS_ROOT_END"
        older_command = "older command 日本語 🌍 " * 6000 + "HEADLESS_OLDER_END"
        fixture.native.task_processes = {
            "native-root":[{"processId":"root-process", "itemId":"root-item", "osPid":123,
                "command":root_command, "cwd":"/tmp/root"}],
            "native-child-256":[{"processId":"older-process", "itemId":"older-item", "osPid":456,
                "command":older_command, "cwd":"/tmp/older"}],
        }
        report = fixture.root / "rust-panel-report.json"
        environment = {**os.environ, "CARLOS_TEST_DAEMON_SOCKET":str(fixture.socket),
            "CARLOS_TEST_SESSION":session, "CARLOS_TEST_PANEL_REPORT":str(report)}
        result = subprocess.run([consumer, "--ignored", "--exact", "actual_daemon_panels_use_shared_readers", "--nocapture"],
            env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=40)
        (fixture.root / "rust-panel-consumer.log").write_text(result.stdout)
        self.assertEqual(result.returncode, 0, result.stdout)
        proof = json.loads(report.read_text())
        self.assertEqual(proof["root_command"], root_command)
        self.assertEqual(proof["older_command"], older_command)
        self.assertTrue(proof["rejected_old_window_reply"])
        self.assertTrue(proof["reconnected"])
        methods = proof["methods"]
        self.assertEqual(methods.count("hello"), 2)
        self.assertGreater(methods.count("task/list"), 8)
        self.assertGreater(methods.count("task/text"), 4)
        self.assertGreaterEqual(methods.count("task/window/close"), 1)
        self.assertFalse(any(method in methods for method in ("task/stop", "session/send", "session/create", "session/rewind")))
        self.assertFalse(any(row.get("method") in ("turn/interrupt", "thread/backgroundTerminals/terminate", "turn/start")
            for row in fixture.native.requests))
