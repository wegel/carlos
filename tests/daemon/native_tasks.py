"""Opt-in native task pages for actual daemon tests; ordinary peers keep their empty lists."""

import asyncio


class NativeTasks:
    def init_tasks(self):
        self.task_agents = None
        self.task_processes = {}
        self.task_errors = set()
        self.task_hold = False
        self.task_pending = None
        self.task_turns = {}
        self.task_run_hold = set()
        self.task_run_pending = {}
        self.task_stop_hold = set()
        self.task_stop_pending = {}
        self.task_stop_errors = set()
        self.task_window_hold = False
        self.task_window_pending = None
        self.task_window_error = False
        self.task_process_hold = set()
        self.task_process_pending = {}

    async def task_request(self, link, request):
        if self.task_agents is None:
            return False
        method, params = request["method"], request["params"]
        if method in ("turn/interrupt", "thread/backgroundTerminals/terminate"):
            owner = params["threadId"]
            identity = params.get("processId", owner)
            if owner != self.thread_id or method.endswith("/terminate"):
                result = {"terminated": True} if "processId" in params else {}
                reply = {"id": request["id"], "result": result}
                if identity in self.task_stop_errors:
                    reply = {"id": request["id"], "error": {"code": -32602, "message": "PRIVATE_TASK_REJECTION"}}
                if identity in self.task_stop_hold:
                    assert identity not in self.task_stop_pending
                    self.task_stop_pending[identity] = (link, reply)
                else:
                    await self.send(link, reply)
                return True
        if method == "thread/turns/list" and params["threadId"] != self.thread_id:
            thread = params["threadId"]
            assert params == {"threadId": thread, "sortDirection": "desc", "itemsView": "notLoaded", "limit": 1}
            result = {"data": self.task_turns.get(thread, []), "nextCursor": None}
            if thread in self.task_run_hold:
                assert thread not in self.task_run_pending
                self.task_run_pending[thread] = (link, request["id"], result)
            else:
                await self.send(link, {"id": request["id"], "result": result})
            return True
        if method == "thread/list":
            assert params["ancestorThreadId"] == self.thread_id
            data = self.task_agents
        elif method == "thread/backgroundTerminals/list":
            owner = params["threadId"]
            if owner in self.task_errors:
                await self.send(link, {"id": request["id"], "error": {
                    "code": -32000, "message": "process scope unavailable"}})
                return True
            data = self.task_processes.get(owner, [])
        elif method == "thread/read" and params["threadId"] != self.thread_id:
            agent = next(agent for agent in self.task_agents if agent["id"] == params["threadId"])
            assert params["includeTurns"] is False
            await self.send(link, {"id": request["id"], "result": {"thread": agent}})
            return True
        else:
            return False
        offset = int((params.get("cursor") or "task:0").split(":")[1])
        end = min(len(data), offset + params.get("limit", 1))
        result = {"data": data[offset:end], "nextCursor": f"task:{end}" if end < len(data) else None}
        if method == "thread/list" and offset >= 256 and self.task_window_error:
            await self.send(link, {"id":request["id"], "error":{"code":-32000,"message":"OLDER_NATIVE_READ_FAILED"}})
            return True
        if method == "thread/list" and offset >= 256 and self.task_window_hold:
            assert self.task_window_pending is None
            self.task_window_pending = (link, request["id"], result)
            return True
        if method == "thread/backgroundTerminals/list" and params["threadId"] in self.task_process_hold:
            assert params["threadId"] not in self.task_process_pending
            self.task_process_pending[params["threadId"]] = (link, request["id"], result)
            return True
        if self.task_hold:
            assert self.task_pending is None
            self.task_pending = (link, request["id"], result)
        else:
            await self.send(link, {"id": request["id"], "result": result})
        return True

    def release_task_window(self):
        async def release():
            link, identity, result = self.task_window_pending
            self.task_window_pending = None
            self.task_window_hold = False
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    def release_task_process(self, owner):
        async def release():
            link, identity, result = self.task_process_pending.pop(owner)
            self.task_process_hold.discard(owner)
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    def release_tasks(self):
        async def release():
            link, identity, result = self.task_pending
            self.task_pending = None
            self.task_hold = False
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    def release_task_run(self, thread):
        async def release():
            link, identity, result = self.task_run_pending.pop(thread)
            self.task_run_hold.discard(thread)
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    def observe_task_turn(self, thread, identity):
        async def observe():
            turn = {"id": identity, "status": "inProgress"}
            next(agent for agent in self.task_agents if agent["id"] == thread)["lastObservedTurn"] = turn
            self.task_turns[thread] = [turn]
            for link in list(self.connections):
                await self.notify(link, "turn/started", threadId=thread, turn=turn)
        asyncio.run_coroutine_threadsafe(observe(), self.loop).result(timeout=3)

    def release_task_stop(self, identity):
        async def release():
            link, reply = self.task_stop_pending.pop(identity)
            self.task_stop_hold.discard(identity)
            await self.send(link, reply)
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
