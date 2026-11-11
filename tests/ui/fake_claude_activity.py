"""Native task/reset/state fixtures sent through the real SDK, without a model or account."""
import time
import threading
import uuid


class Activity:
    def __init__(self, host):
        self.h = host
        self.queued = []
        self.clear = None
        self.live = []
        self.answer = None
        self.compacted = False
        self.reset_answers = set()

    def emit(self, subtype, **fields):
        self.h.send({"type": "system", "subtype": subtype, "session_id": self.h.SESSION,
                     "uuid": str(uuid.uuid4()), **fields})

    def init(self, prompt):
        self.emit("init", model=self.h.model, user_message_uuids=[prompt])

    def done(self, prompt, text, idle=True):
        self.init(prompt)
        self.h.event("message_start", prompt, message={"id": "native-" + prompt, "role": "assistant", "content": []})
        self.h.event("content_block_start", prompt, index=0, content_block={"type": "text", "text": text})
        self.h.event("content_block_stop", prompt, index=0)
        self.h.result(prompt)
        self.h.active = None
        if idle:
            self.h.state("idle")

    def level(self, tasks):
        self.live = tasks
        self.emit("background_tasks_changed", tasks=tasks)

    def task(self, task="child", run="z", description="Independent child"):
        self.emit("task_started", task_id=task, run_id=run, task_type="agent", description=description,
                  parent_task_id="main-native", tool_use_id="child-caller", is_backgrounded=True)
        return {"task_id": task, "run_id": run, "task_type": "agent", "description": description}

    def control(self, message):
        request = message["request"]
        kind = request["subtype"]
        if kind == "get_usage":
            self.h.control_reply(message, {"rate_limits_available": False, "rate_limits": None, "behaviors": None})
            return True
        if kind == "get_context_usage" and self.compacted:
            self.h.control_reply(message, {"totalTokens": 900, "rawMaxTokens": 123456})
            return True
        if kind == "stop_task":
            if request["task_id"] == "reset-trigger":
                self.callback_reset()
            if request["task_id"] == "fail-child":
                self.h.send({"type": "control_response", "response": {"subtype": "error", "request_id": message["request_id"], "error": "Native stop failed"}})
            else:
                self.h.control_reply(message, {})
            return True
        if kind == "interrupt" and self.queued:
            queued, self.queued = self.queued, []
            self.h.control_reply(message, {"still_queued": [entry["uuid"] for entry in queued]})
            if self.h.active:
                self.h.result(self.h.active, interrupted=True)
            self.h.state("running")
            for index, entry in enumerate(queued):
                self.done(entry["uuid"], "Queued once: " + entry["message"]["content"], idle=index == len(queued)-1)
            return True
        return False

    def response(self, message):
        identifier = message["response"]["request_id"]
        if identifier == "reset-two":
            return True
        if identifier.startswith("reset-"):
            self.reset_answers.add(identifier)
            if self.reset_answers == {"reset-active", "reset-three", "reset-four"}:
                self.emit("informational", level="notice", content="Reset callbacks settled")
            return True
        if identifier != "late-child-question":
            return False
        self.answer = message
        self.emit("task_progress", task_id="child", run_id="z", description="Child answered", summary="Child got its answer", usage={"total_tokens": 9, "tool_uses": 1, "duration_ms": 20})
        return True

    def prompt(self, message):
        text, prompt = message["message"]["content"], message["uuid"]
        if self.clear and text != "/clear":
            return self.reset(message)
        if text == "/clear" and self.h.MODE == "claude-native-reset":
            self.clear = message
            self.h.sessions.prompt(message)
            return True
        if self.h.active and message.get("priority") == "later" and self.h.MODE == "claude-native-queue":
            self.h.sessions.prompt(message)
            self.queued.append(message)
            self.emit("command_lifecycle", command_uuid=prompt, state="queued")
            return True
        if text not in ("native tasks", "native compact", "native thinking", "native quotas", "finish task", "task levels", "failed task", "native notice", "native phone tasks", "reset questions", "native changing tasks", "native hidden children"):
            return False
        self.h.sessions.prompt(message)
        self.h.state("running")
        self.h.active = prompt
        task_prompts = {"native hidden children", "reset questions", "native changing tasks", "native phone tasks",
            "native tasks", "failed task", "finish task", "task levels"}
        if text in task_prompts:
            self.task_prompt(text,prompt)
        else:
            self.state_prompt(text,prompt)
        return True

    def task_prompt(self,text,prompt):
        if text == "native hidden children":
            self.done(prompt,"Parent before hidden children")
            for index in range(180):
                self.h.send({"type":"assistant","session_id":self.h.SESSION,"agent_id":f"active-child-{index}","parent_tool_use_id":f"caller-{index}","uuid":str(uuid.uuid4()),"message":{"id":f"child-{index}","role":"assistant","content":[{"type":"text","text":f"PRIVATE_CHILD {index}"}]}})
        elif text == "reset questions":
            self.level([self.task("reset-trigger", description="Reset while children wait")])
            self.done(prompt, "Parent before callback reset")
            self.h.send({"type":"control_request","request_id":"reset-active","request":{"subtype":"elicitation","mcp_server_name":"reset-server","mode":"form","message":"Reset form","requested_schema":{"type":"object","properties":{"age":{"type":"integer","title":"Age"}},"required":["age"]}}})
            for identifier, title in [("reset-two", "Reset second"), ("reset-three", "Reset third")]:
                self.permission(identifier, title)
        elif text == "native changing tasks":
            tasks = [self.task("alpha", description="Selected first task"), self.task("beta", description="Second task remains")]
            self.level(tasks)
            self.done(prompt,"Changing native tasks")
            def finish_alpha():
                gate = self.h.ROOT / "finish-alpha"
                while not gate.exists():
                    time.sleep(.01)
                self.emit("task_notification",task_id="alpha",run_id="z",status="completed",summary="Alpha completed",output_file="/tmp/alpha")
                self.level([tasks[1]])
            threading.Thread(target=finish_alpha,daemon=True).start()
        elif text == "native phone tasks":
            self.level([self.task("alpha", description="First visible task"),self.task("beta", description="Second visible task")])
            self.done(prompt, "Two native tasks")
        elif text == "native tasks":
            task = self.task()
            self.level([task])
            self.tool_records({}, "main-call", "echo MAIN_TOOL_COMMAND", "MAIN_TOOL_OUTPUT")
            self.done(prompt, "Parent finished")
            for tags in [{"agent_id":"child"}, {"parent_tool_use_id":"child-caller"}, {"parent_agent_id":"nested-parent"}]:
                self.tool_records(tags, str(uuid.uuid4()), "echo PRIVATE_CHILD_COMMAND", "PRIVATE_CHILD_RESULT")
                for event in [{"type":"message_start","message":{"id":str(uuid.uuid4()),"role":"assistant","content":[]}},
                              {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},
                              {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"PRIVATE_CHILD_STREAM"}},
                              {"type":"content_block_stop","index":0}]:
                    self.h.send({"type":"stream_event","session_id":self.h.SESSION,"uuid":str(uuid.uuid4()),"event":event,**tags})
                self.h.send({"type":"assistant","session_id":self.h.SESSION,"uuid":str(uuid.uuid4()),**tags,
                             "message":{"id":str(uuid.uuid4()),"role":"assistant","content":[
                                 {"type":"thinking","thinking":"PRIVATE_CHILD_THOUGHT"},
                                 {"type":"text","text":"PRIVATE_CHILD Late 日本語"}]}})
            self.h.send({"type": "control_request", "request_id": "late-child-question", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "tool_use_id": "child-question-tool", "agent_id": "child", "title": "Child permission after parent result", "input": {"command": "echo child"}}})
        elif text == "failed task":
            self.level([self.task("fail-child", description="Stop failure fixture")])
            self.done(prompt, "Failure child running")
        elif text == "finish task":
            self.emit("task_notification", task_id="child", run_id="z", status="stopped", summary="Native child stopped", output_file="/old-session/child.output", usage={"total_tokens": 9, "tool_uses": 1, "duration_ms": 20})
            self.level([])
            self.done(prompt, "Task is done")
        elif text == "task levels":
            self.level([{**self.task("ambient", description="Ambient watcher"), "ambient": True}, self.task("visible", description="Foreground member")])
            self.level([{**self.live[0], "ambient": True}])
            self.done(prompt, "Only ambient remains")

    def state_prompt(self,text,prompt):
        if text == "native compact":
            self.emit("status", status="compacting")
            self.emit("status", status=None, compact_result="failed", compact_error="Fixture compaction refused")
            self.emit("status", status="compacting")
            self.emit("compact_boundary", compact_metadata={"trigger": "auto", "pre_tokens": 32123})
            self.emit("status", status=None, compact_result="success")
            self.compacted = True
            self.done(prompt, "Compaction finished")
        elif text == "native thinking":
            self.init(prompt)
            self.h.event("message_start", prompt, message={"id": "thinking-api", "role": "assistant", "content": []})
            self.h.event("content_block_start", prompt, index=0, content_block={"type": "thinking", "thinking": ""})
            self.h.event("content_block_delta", prompt, index=0, delta={"type": "thinking_delta", "thinking": "Visible native thought 日本語"})
            self.h.event("content_block_stop", prompt, index=0)
            self.h.send({"type": "assistant", "session_id": self.h.SESSION, "uuid": str(uuid.uuid4()), "message": {"id": "thinking-api", "role": "assistant", "content": [{"type": "thinking", "thinking": "Visible native thought 日本語", "signature": "HIDDEN_SIGNATURE"}]}})
            self.emit("thinking_tokens", estimated_tokens=43, estimated_tokens_delta=37)
            self.h.event("content_block_start", prompt, index=1, content_block={"type": "redacted_thinking", "data": "NEVER_SHOW_SECRET"})
        elif text == "native quotas":
            reset = int(time.time()) + 86400
            self.h.send({"type": "rate_limit_event", "session_id": self.h.SESSION, "rate_limit_info": {"status": "allowed", "unifiedWindows": {"five_hour": {"utilization": .25, "resetsAt": reset}, "seven_day": {"utilization": .4, "resetsAt": reset}}, "rateLimitType": "five_hour"}})
            self.done(prompt, "Quota received")
        else:
            self.emit("informational", level="warning", content="Native settings warning", prevent_continuation=False)
            self.emit("session_writes_lost", error_code="ENOSPC", lost=["transcript"], transcript_session_ids=[self.h.SESSION])
            self.done(prompt, "Healthy after warning")

    def reset(self, message):
        old = self.h.SESSION
        clear, self.clear = self.clear, None
        self.h.send({"type": "conversation_reset", "session_id": old, "uuid": str(uuid.uuid4()), "new_conversation_id": str(uuid.uuid4()), "trigger": "clear", "user_message_uuid": clear["uuid"]})
        self.emit("background_tasks_changed", tasks=self.live)
        self.h.SESSION = str(uuid.uuid4())
        self.h.sessions = type(self.h.sessions)(["--session-id", self.h.SESSION], self.h.Path.cwd())
        self.init(clear["uuid"])
        self.h.result(clear["uuid"])
        self.h.sessions.prompt(message)
        self.done(message["uuid"], "After reset once: " + message["message"]["content"])
        return True

    def permission(self, identifier, title):
        self.h.send({"type":"control_request","request_id":identifier,"request":{"subtype":"can_use_tool","tool_name":"Bash","tool_use_id":identifier,"agent_id":identifier,"title":title,"input":{"command":"echo child"}}})

    def callback_reset(self):
        old = self.h.SESSION
        self.h.send({"type":"conversation_reset","session_id":old,"uuid":str(uuid.uuid4()),"new_conversation_id":str(uuid.uuid4()),"trigger":"plan_mode_exit"})
        self.emit("background_tasks_changed",tasks=self.live)
        self.permission("reset-four","Reset fourth")
        self.h.SESSION = str(uuid.uuid4())
        self.h.sessions = type(self.h.sessions)(["--session-id", self.h.SESSION], self.h.Path.cwd())
        self.emit("init",model=self.h.model)
        self.h.send({"type":"control_cancel_request","request_id":"reset-two"})

    def tool_records(self, tags, identifier, command, output):
        self.h.send({"type":"assistant","session_id":self.h.SESSION,"uuid":str(uuid.uuid4()),**tags,
                     "message":{"id":str(uuid.uuid4()),"role":"assistant","content":[
                         {"type":"tool_use","id":identifier,"name":"Bash","input":{"command":command}}]}})
        self.h.send({"type":"user","session_id":self.h.SESSION,"uuid":str(uuid.uuid4()),**tags,
                     "message":{"role":"user","content":[{"type":"tool_result","tool_use_id":identifier,"content":output}]}})
