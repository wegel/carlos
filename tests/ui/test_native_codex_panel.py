"""Exercise real native descendant metadata and background process controls using localhost replies."""

import json

from mock_responses import answer, outputs, tool


class NativeCodexPanel:
    def test_native_codex_f2_reads_child_without_resume_and_interrupts_its_exact_turn(self):
        def reply(body):
            text=json.dumps(body.get("input",[]))
            if "panel-child-isolated" in text and "panel-spawn" not in text:
                if "panel-child-sleep" in outputs(body):
                    return [answer("CHILD_AFTER_WAIT")]
                return [{**answer("CHILD_PANEL_PROGRESS"),"phase":"commentary"},
                    tool("panel-child-sleep","clock","sleep",{"duration_ms":120000})]
            if "panel-spawn" in outputs(body):
                return [answer("MAIN_AGENT_REPORT_STAYS_VISIBLE")]
            return [tool("panel-spawn","collaboration","spawn_agent",{
                "task_name":"panel_worker","message":"panel-child-isolated","fork_turns":"none"})]

        terminal,model=self.launch(reply)
        terminal.paste("Start the isolated panel child")
        terminal.send(b"\r")
        terminal.contains("MAIN_AGENT_REPORT_STAYS_VISIBLE")
        terminal.wait(lambda:len(model.requests)>=3,"child native wait started")
        terminal.paste("retain this main draft")
        terminal.send(b"\x1bOQ")
        terminal.contains("Codex agents / processes")
        terminal.wait(lambda:bool([request for request in terminal.requests("thread/list") if "ancestorThreadId" in request["params"]]),"native descendant list")
        root=terminal.requests("turn/start")[0]["params"]["threadId"]
        request=next(request for request in terminal.requests("thread/list") if request["params"].get("ancestorThreadId")==root)
        terminal.wait(lambda:self.native_reply(terminal,request) is not None,"native descendants returned")
        result=self.native_reply(terminal,request)
        self.assertNotIn("error",result)
        child=result["result"]["data"][0]
        self.assertEqual(child["parentThreadId"],root)
        self.assertIn("subAgent",child["source"])
        terminal.contains("Thread: "+child["id"])
        terminal.send(b"\r")
        terminal.contains("Read-only child activity")
        terminal.contains("CHILD_PANEL_PROGRESS")
        terminal.wait(lambda:bool(terminal.requests("thread/items/list")),"public child item read")
        lists_before_stop=len(terminal.requests("thread/list"))
        terminal.send(b"x")
        terminal.wait(lambda:bool(terminal.requests("turn/interrupt")),"native selected-child interrupt")
        stop=terminal.requests("turn/interrupt")[-1]
        self.assertEqual(stop["params"]["threadId"],child["id"])
        terminal.wait(lambda:self.native_reply(terminal,stop) is not None,"native interrupt result")
        self.assertNotIn("error",self.native_reply(terminal,stop))
        terminal.wait(lambda:len(terminal.requests("thread/list"))>lists_before_stop,"readonly metadata refresh after detail stop")
        terminal.wait(lambda:any(message.get("method")=="turn/completed" and message["params"].get("threadId")==child["id"] for message in terminal.messages("server")),"child turn ended")
        terminal.send(b"r")
        terminal.wait(lambda:any(request["params"]["threadId"]==child["id"] and self.native_reply(terminal,request) is not None
            and self.native_reply(terminal,request).get("result",{}).get("data")
            for request in terminal.requests("thread/items/list")),"native persisted child details")
        self.assertEqual(terminal.requests("thread/resume"),[])
        self.assertEqual(len(terminal.requests("turn/start")),1)
        self.assertEqual(terminal.requests("thread/backgroundTerminals/clean"),[])
        terminal.send(b"\x1bOQ")
        terminal.wait(lambda:terminal.input_is("retain this main draft"),"draft after native child stop")
        terminal.contains("MAIN_AGENT_REPORT_STAYS_VISIBLE")
        self.assertNotIn("CHILD_AFTER_WAIT",terminal.text)
        self.assertNotIn("CHILD_PANEL_PROGRESS",terminal.text)
        terminal.snapshot("native-selected-child-stop")

    def test_native_codex_f2_lists_and_terminates_only_the_selected_background_process(self):
        def reply(body):
            if "panel-background-exec" in outputs(body):
                return [answer("ROOT_BACKGROUND_STARTED")]
            return [tool("panel-background-exec","functions","exec_command",{
                "cmd":"sleep 120","yield_time_ms":1000,"max_output_tokens":100})]

        terminal,model=self.launch(reply)
        terminal.paste("Start one isolated background terminal")
        terminal.send(b"\r")
        terminal.contains("ROOT_BACKGROUND_STARTED")
        terminal.paste("keep process-stop draft")
        terminal.send(b"\x1bOQ")
        terminal.contains("Codex agents / processes")
        terminal.wait(lambda:bool(terminal.requests("thread/backgroundTerminals/list")),"native terminal list")
        listing=terminal.requests("thread/backgroundTerminals/list")[0]
        terminal.wait(lambda:self.native_reply(terminal,listing) is not None,"native terminals returned")
        result=self.native_reply(terminal,listing)
        self.assertNotIn("error",result)
        process=result["result"]["data"][0]
        terminal.contains("Process: "+process["processId"])
        terminal.send(b"x")
        terminal.wait(lambda:bool(terminal.requests("thread/backgroundTerminals/terminate")),"selected native process stop")
        stop=terminal.requests("thread/backgroundTerminals/terminate")[0]
        self.assertEqual(stop["params"],{"threadId":listing["params"]["threadId"],"processId":process["processId"]})
        terminal.wait(lambda:self.native_reply(terminal,stop) is not None,"native process stop result")
        self.assertTrue(self.native_reply(terminal,stop)["result"]["terminated"])
        terminal.wait(lambda:any(request["id"]!=listing["id"] and self.native_reply(terminal,request) is not None
            and self.native_reply(terminal,request).get("result",{}).get("data")==[]
            for request in terminal.requests("thread/backgroundTerminals/list")),"terminated process absent on native refresh")
        self.assertEqual(terminal.requests("thread/backgroundTerminals/clean"),[])
        self.assertEqual(terminal.requests("turn/interrupt"),[])
        self.assertEqual(len(terminal.requests("turn/start")),1)
        terminal.send(b"\x1bOQ")
        terminal.wait(lambda:terminal.input_is("keep process-stop draft"),"process stop preserves composer")
        terminal.snapshot("native-selected-background-terminate")
