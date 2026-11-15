"""Measure actual daemon/terminal work while diagnostics stay private to each device."""

import re

F8 = b"\x1b[19~"
CTRL_P = b"\x10"


class DaemonMetrics:
    def metrics_ready(self, terminal, session):
        terminal.contains("history-output 9999")
        terminal.wait(lambda:any(isinstance(message.get("result"), dict) and
            message["result"].get("snapshot",{}).get("info",{}).get("id") == session and
            message["result"]["snapshot"]["info"]["availability"] == "ready"
            for message in terminal.messages("server")), "terminal accepted its ready metrics fixture snapshot")

    def test_daemon_metrics_toggle_keeps_composer_and_reports_real_socket_and_draw_work(self):
        fixture, client, session = self.daemon_session("metrics-enabled")
        terminal = self.attach_daemon(fixture, session, "metrics", extra_env={"CARLOS_METRICS":"1"})
        self.metrics_ready(terminal, session)
        terminal.contains("event handle")
        terminal.contains("text render")
        terminal.send(F8)
        terminal.wait(lambda:"event handle" not in terminal.text, "F8 hides only the local diagnostics")
        terminal.paste("metrics prompt")
        terminal.send(b"\r")
        terminal.contains("Answer: metrics prompt")
        terminal.send(CTRL_P)
        terminal.contains("event handle")
        terminal.paste("private metrics draft 日本語")
        terminal.wait(lambda:terminal.input_is("private metrics draft 日本語"), "metrics preserve the ordinary draft")
        terminal.send(F8)
        terminal.wait(lambda:"event handle" not in terminal.text, "F8 hides after Ctrl+P showed")
        terminal.send(CTRL_P)
        terminal.contains("key -> draw")
        self.assertEqual([row["params"]["text"] for row in terminal.requests("session/send")], ["metrics prompt"])
        self.assertEqual(terminal.requests("session/approve"), [])
        self.assertEqual([row["params"]["input"][0]["text"] for row in fixture.native.requests if row.get("method") == "turn/start"], ["metrics prompt"])
        terminal.snapshot("metrics-overlay-keeps-private-draft")
        terminal.send(b"\x03")
        raw = terminal.root/"terminal.ansi"
        terminal.wait(lambda:b"carlos perf metrics" in raw.read_bytes(), "metrics print after leaving the alternate screen")
        terminal.close_process()
        report = raw.read_text(errors="replace").split("carlos perf metrics", 1)[1]
        counts = dict(re.findall(r"(\w+)=(\d+)", report.splitlines()[1]))
        for name in ("loop", "frame", "key", "key_press", "paste", "notifications"):
            self.assertGreater(int(counts[name]), 0, name)
        for phase in ("draw:", "poll_wait:", "event_handle:", "text_render:", "input_layout:", "key_to_draw:"):
            self.assertIn(phase, report)
        self.assertIsNone(fixture.process.poll(), "diagnostic exit detaches from the live daemon")

    def test_daemon_metrics_disabled_keys_do_not_type_or_send_or_print_a_report(self):
        fixture, client, session = self.daemon_session("metrics-disabled")
        terminal = self.attach_daemon(fixture, session, "metrics-off", extra_env={"CARLOS_METRICS":"0"})
        self.metrics_ready(terminal, session)
        terminal.paste("private draft without diagnostics")
        terminal.send(F8 + CTRL_P + b"!")
        terminal.wait(lambda:terminal.input_is("private draft without diagnostics!"), "disabled metric shortcuts preserve the composer")
        self.assertNotIn("event handle", terminal.text)
        self.assertEqual(terminal.requests("session/send"), [])
        self.assertEqual(terminal.requests("session/approve"), [])
        terminal.close_process()
        self.assertNotIn(b"carlos perf metrics", (terminal.root/"terminal.ansi").read_bytes())
        self.assertIsNone(fixture.process.poll())
