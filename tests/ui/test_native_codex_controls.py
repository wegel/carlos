"""Prove native queue dispatch and live/future settings with isolated localhost model replies."""

import json

from mock_responses import answer, outputs, tool


class NativeCodexControls:
    def native_reply(self,terminal,request):
        return next((message for message in terminal.messages("server")
            if message.get("id")==request["id"] and "method" not in message),None)

    def wait_native_queue_add(self,terminal):
        terminal.wait(lambda:bool(terminal.requests("thread/queue/add")),"native queue add")
        request=terminal.requests("thread/queue/add")[-1]
        terminal.wait(lambda:self.native_reply(terminal,request) is not None,"native enqueue acknowledged")
        response=self.native_reply(terminal,request)
        self.assertNotIn("error",response)
        self.assertEqual(response["result"]["queuedSubmission"]["clientUserMessageId"],request["params"]["clientUserMessageId"])
        return request,response["result"]["queuedSubmission"]

    def test_native_codex_queue_automatically_consumes_once_after_the_running_turn_finishes(self):
        def reply(body):
            if "queued native human" in json.dumps(body.get("input",[])):
                return [answer("NATIVE_AUTO_QUEUE_CONSUMED")]
            self.assertTrue(model.release.wait(15),"release the isolated first response")
            return [answer("NATIVE_AUTO_ROOT_DONE")]

        terminal,model=self.launch(reply)
        terminal.paste("Hold the isolated root response")
        terminal.send(b"\r")
        terminal.wait(lambda:len(model.requests)==1,"native model request pending")
        terminal.send(b"\x1bOR")
        terminal.paste("queued native human")
        terminal.send(b"\r")
        request,queued=self.wait_native_queue_add(terminal)
        self.assertNotIn("queued native human",terminal.text)
        model.release.set()
        terminal.contains("NATIVE_AUTO_QUEUE_CONSUMED")
        native_users=[message["params"]["item"] for message in terminal.messages("server")
            if message.get("method")=="item/completed" and message["params"]["item"].get("type")=="userMessage"]
        self.assertEqual(len([item for item in native_users if item.get("clientId")==request["params"]["clientUserMessageId"]]),1)
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("thread/queue/start"),[])
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),2)
        terminal.snapshot("native-automatic-queue-consumed-once")

    def test_native_codex_interrupt_keeps_the_queue_until_explicit_native_start(self):
        def reply(body):
            if "queued interrupted human" in json.dumps(body.get("input",[])):
                return [answer("NATIVE_MANUAL_QUEUE_CONSUMED")]
            if "queue-isolated-sleep" in outputs(body):
                return [answer("NATIVE_SLEEP_DONE")]
            return [tool("queue-isolated-sleep","clock","sleep",{"duration_ms":120000})]

        terminal,model=self.launch(reply)
        terminal.paste("Start the isolated native wait")
        terminal.send(b"\r")
        terminal.contains("Wait up to 120 s")
        terminal.send(b"\x1bOR")
        terminal.paste("queued interrupted human")
        terminal.send(b"\r")
        request,queued=self.wait_native_queue_add(terminal)
        terminal.send(b"\x1b")
        terminal.contains("Turn interrupted")
        terminal.send(b"\x1bq")
        terminal.contains("queued interrupted human")
        terminal.wait(lambda:"Refreshing" not in terminal.text,"native interrupted queue snapshot")
        terminal.send(b"s")
        terminal.wait(lambda:bool(terminal.requests("thread/queue/start")),"explicit native queued start")
        self.assertEqual(terminal.requests("thread/queue/start")[0]["params"]["queuedSubmissionId"],queued["id"])
        terminal.send(b"\x1b")
        terminal.contains("NATIVE_MANUAL_QUEUE_CONSUMED")
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertEqual(len(model.requests),2)
        terminal.snapshot("native-queue-survives-interrupt")

    def test_native_codex_live_settings_preserve_captured_request_and_change_the_next_turn(self):
        def reply(body):
            if "Use the isolated saved future settings" in json.dumps(body.get("input",[])):
                return [answer("NATIVE_FUTURE_SETTINGS_DONE")]
            if "live-settings-followup" in outputs(body):
                return [answer("NATIVE_SAME_TURN_SETTINGS_DONE")]
            self.assertTrue(model.release.wait(15),"release the already captured request")
            return [tool("live-settings-followup","clock","sleep",{"duration_ms":1})]

        terminal,model=self.launch(reply)
        launches=[json.loads(line) for line in (terminal.root/"native-launches.jsonl").read_text().splitlines()]
        self.assertIn(["--enable","step_model_switching","app-server"],[launch["args"] for launch in launches])
        terminal.paste("Start the isolated captured request")
        terminal.send(b"\r")
        terminal.wait(lambda:len(model.requests)==1,"captured native request")
        captured=json.loads(json.dumps(model.requests[0]))
        terminal.send(b"\x1b[109;5u")
        terminal.contains("Settings")
        terminal.send(b"\x1b[B\x1b[C\r")
        terminal.contains("Published for later steps only")
        future=terminal.requests("thread/settings/update")[-1]
        live=terminal.requests("turn/settings/update")[0]
        self.assertNotIn("error",self.native_reply(terminal,future))
        self.assertEqual(self.native_reply(terminal,live)["result"]["status"],"applied")
        self.assertEqual(model.requests[0],captured)
        model.release.set()
        terminal.contains("NATIVE_SAME_TURN_SETTINGS_DONE")
        self.assertEqual(len(model.requests),2,"second capture belongs to the same running turn")
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(model.requests[1]["reasoning"]["effort"],live["params"]["effort"])
        terminal.wait(lambda:any(message.get("method")=="turn/completed" for message in terminal.messages("server")),"captured turn ended")
        terminal.paste("Use the isolated saved future settings")
        terminal.send(b"\r")
        terminal.contains("NATIVE_FUTURE_SETTINGS_DONE")
        self.assertEqual(len(model.requests),3,"third capture belongs to a new future turn")
        self.assertEqual(model.requests[2]["reasoning"]["effort"],future["params"]["effort"])
        self.assertNotEqual(captured["reasoning"]["effort"],future["params"]["effort"])
        self.assertEqual(terminal.requests("config/value/write"),[])
        self.assertEqual(terminal.requests("turn/steer"),[])
        terminal.snapshot("native-live-and-future-settings")
