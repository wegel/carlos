"""Opt-in native task pages for actual daemon tests; ordinary peers keep their empty lists."""

import asyncio


class NativeTasks:
    def init_tasks(self):
        self.task_agents = None
        self.task_processes = {}
        self.task_errors = set()
        self.task_hold = False
        self.task_pending = None

    async def task_request(self, link, request):
        if self.task_agents is None:
            return False
        method, params = request["method"], request["params"]
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
        if self.task_hold:
            assert self.task_pending is None
            self.task_pending = (link, request["id"], result)
        else:
            await self.send(link, {"id": request["id"], "result": result})
        return True

    def release_tasks(self):
        async def release():
            link, identity, result = self.task_pending
            self.task_pending = None
            self.task_hold = False
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
