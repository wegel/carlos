"""Drive bounded saved pages and archive consent using only the daemon's opaque choices."""

import sys

from daemon_recorder import DaemonRecorder
from test_daemon_client import daemon_support


class DaemonSavedPicker:
    def saved_picker(self, name, count=80):
        runner = sys.modules[type(self).__module__]
        daemon_support.BINARY = runner.BINARY.parent / "carlosd"
        daemon_support.ARTIFACTS = runner.ARTIFACTS / "daemon"
        fixture = daemon_support.Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.saved_rows = [{"id":f"PRIVATE_NATIVE_{index}", "name":f"Saved {index}",
            "preview":f"Preview {index}", "cwd":str(fixture.root), "createdAt":index,
            "updatedAt":index} for index in range(count)]
        watcher = daemon_support.Client(fixture, "watcher")
        self.addCleanup(watcher.close)
        recorder = DaemonRecorder(fixture.root / "picker.sock", fixture.socket, fixture.root)
        self.addCleanup(recorder.close)
        terminal = self.launch(directory=fixture.root, resume="__picker__",
                               extra_args=["--daemon-socket", str(recorder.path)])
        terminal.daemon_recorder = recorder
        return fixture, watcher, terminal

    def test_daemon_picker_jumps_global_pages_archives_exact_choice_and_opens_only_selected_row(self):
        fixture, watcher, terminal = self.saved_picker("picker-pages")
        terminal.contains("Saved 79")
        self.assertEqual(watcher.call("sessions/watch")["sessions"], [])
        terminal.send(b"G")
        terminal.contains("Saved 0")
        terminal.send(b"g")
        terminal.contains("Saved 79")
        terminal.send(b"j" * 32)
        terminal.contains("Saved 47")
        terminal.send(b"d")
        terminal.contains("Delete session?")
        terminal.send(b"n")
        terminal.wait(lambda: "Delete session?" not in terminal.text, "archive cancelled locally")
        self.assertEqual(fixture.native.saved_archives, [])
        terminal.send(b"dy")
        terminal.wait(lambda: fixture.native.saved_archives == ["PRIVATE_NATIVE_47"], "exact captured row archived")
        terminal.contains("Archived session")
        self.assertEqual(watcher.call("sessions/watch")["sessions"], [])
        terminal.send(b"G")
        terminal.contains("Saved 0")
        terminal.send(b"\r")
        terminal.contains("history-output 9999")
        self.assertEqual([request["params"]["threadId"] for request in fixture.native.requests
                          if request.get("method") == "thread/resume"], ["PRIVATE_NATIVE_0"])
        self.assertEqual(fixture.native.started, 0)
        self.assertEqual(len(fixture.saved()), 1)
        self.assertEqual(len(terminal.requests("session/open")), 1)
        self.assertEqual(len(terminal.requests("saved/archive")), 1)
        self.assertEqual(terminal.requests("session/create"), [])
        self.assertEqual([request["params"]["direction"] for request in terminal.requests("saved/page")],
                         ["oldest", "newest", "older", "oldest"])
        terminal.snapshot("global-picker-to-one-native-owner")

    def test_daemon_picker_uncertain_archive_requires_fresh_choices_before_another_action(self):
        fixture, _, terminal = self.saved_picker("picker-uncertain", count=3)
        fixture.native.saved_disconnect_archive = True
        terminal.contains("Saved 2")
        terminal.send(b"dy")
        terminal.contains("Archive could not be confirmed")
        terminal.send(b"dy\r")
        for _ in range(3): terminal.pump(.03)
        self.assertEqual(fixture.native.saved_archives, ["PRIVATE_NATIVE_2"])
        self.assertEqual(terminal.requests("session/open"), [])
        terminal.send(b"r")
        terminal.contains("Saved 1")
        self.assertEqual(fixture.native.resumed, 0)
        self.assertEqual(fixture.saved(), [])
        terminal.snapshot("uncertain-archive-fresh-private-choices")
