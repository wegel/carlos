"""Opt-in native callback identities and exact query controls for task-panel checks."""

import asyncio
import json


def callback_key(identity):
    return json.dumps(identity, sort_keys=True, separators=(",", ":"))


def question(identity, thread="native-child-0", turn="run-0", blocking=False, secret=False):
    return {"id":identity, "method":"item/tool/requestUserInput", "params":{
        "threadId":thread, "turnId":turn, "itemId":f"query-{identity}", "isBlocking":blocking,
        "questions":[{"id":f"field-{type(identity).__name__}", "header":f"Native {type(identity).__name__} field",
            "question":"Choose a listed value or enter the original native answer", "isSecret":secret,
            "isOther":True, "options":[{"label":"Listed value", "description":"Native option"}]}]}}


def form(identity, schema=None, turn=None, mode="form"):
    params = {"threadId":"native-child-0", "turnId":turn, "serverName":"native-forms.test",
        "mode":mode, "message":"Native typed form", "requestedSchema":schema or {
            "type":"object", "required":["count", "mode", "enabled"], "properties":{
                "count":{"type":"integer", "minimum":1, "maximum":9, "default":3},
                "mode":{"type":"string", "oneOf":[{"const":"jp", "title":"日本語"}, {"const":"fr", "title":"Français"}], "default":"jp"},
                "enabled":{"type":"boolean", "default":False}, "optional":{"type":"string"}}}}
    if mode == "url":
        params.pop("requestedSchema")
        params.update(url="https://native-forms.test/device?code=original", elicitationId=f"browser-{identity}")
    return {"id":identity, "method":"mcpServer/elicitation/request", "params":params}


class NativeTaskQuestions:
    def init_task_questions(self):
        self.task_callbacks = {}
        self.task_callback_ids = set()
        self.task_answers = []
        self.task_query_interrupts = []
        self.task_query_interrupt_errors = set()
        self.task_query_interrupt_hold = False
        self.task_query_interrupt_pending = []

    def emit_question(self, frame):
        async def emit():
            key = callback_key(frame["id"])
            self.task_callbacks[key] = frame
            self.task_callback_ids.add(key)
            for link in list(self.connections):
                await self.send(link, frame)
        asyncio.run_coroutine_threadsafe(emit(), self.loop).result(timeout=3)

    def resolve_question(self, identity):
        async def resolve():
            frame = self.task_callbacks.pop(callback_key(identity))
            for link in list(self.connections):
                await self.notify(link, "serverRequest/resolved", threadId=frame["params"]["threadId"], requestId=identity)
        asyncio.run_coroutine_threadsafe(resolve(), self.loop).result(timeout=3)

    async def task_question_response(self, link, response):
        key = callback_key(response.get("id"))
        if key not in self.task_callback_ids:
            return False
        assert key in self.task_callbacks, ("duplicate or retired native callback response", response)
        self.task_callbacks.pop(key)
        self.task_answers.append(response)
        return True

    async def task_question_request(self, link, request):
        if request["method"] != "turn/interrupt":
            return False
        params = request["params"]
        target = (params.get("threadId"), params.get("turnId"))
        if not any((frame["params"].get("threadId"), frame["params"].get("turnId")) == target
                   for frame in list(self.task_callbacks.values())):
            return False
        self.task_query_interrupts.append(request)
        if self.task_query_interrupt_hold:
            self.task_query_interrupt_pending.append((link, request))
        elif target in self.task_query_interrupt_errors:
            await self.send(link, {"id":request["id"], "error":{"code":-32000, "message":"PRIVATE_QUERY_INTERRUPT_FAILED"}})
        else:
            await self.send(link, {"id":request["id"], "result":{}})
        return True

    def release_query_interrupt(self):
        async def release():
            link, request = self.task_query_interrupt_pending.pop(0)
            await self.send(link, {"id":request["id"], "result":{}})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
