"""Return complete public Codex turn pages without inference or deprecated full reads."""


import asyncio


class NativeHistoryWindows:
    def __init__(self, native, turns=240):
        self.native = native
        self.turns = [{"id":f"turn-{index}", "status":"completed", "itemsView":"full", "items":[
            {"type":"userMessage", "id":f"user-{index}", "content":[{"type":"text", "text":f"native user {index} 🌍"}]},
            {"type":"agentMessage", "id":f"answer-{index}", "text":f"native answer {index} 🌍", "phase":"final_answer"},
        ]} for index in range(turns)]
        self.pages = []
        self.fail_cursor = None
        self.next_override = None
        self.pause = False
        self.held = []
        self.handle = native.handle
        native.handle = self.request
        # Native resume/metadata replies omit saved turns. Public pages provide them.
        native.history = lambda: {"id":native.thread_id, "status":{"type":"idle"}, "turns":[], **native.native_runtime_response()}

    async def request(self, link, request):
        method, params = request.get("method"), request.get("params", {})
        if method != "thread/turns/list" or params["threadId"] != self.native.thread_id:
            await self.handle(link, request)
            return
        self.pages.append(dict(params))
        if self.pause and params.get("cursor") and params.get("sortDirection") == "desc":
            self.held.append((link, request))
            return
        await self.reply_page(link, request)

    async def reply_page(self, link, request):
        params = request["params"]
        if self.fail_cursor is not None and params.get("cursor") == self.fail_cursor:
            self.fail_cursor = None
            await self.native.send(link, {"id":request["id"], "error":{"code":-32000, "message":"native history fixture read failed"}})
            return
        direction = params.get("sortDirection", "asc")
        rows = self.turns if direction == "asc" else list(reversed(self.turns))
        cursor = params.get("cursor")
        offset = int(cursor.rsplit(":", 1)[1]) if cursor else 0
        count = params.get("limit") or 100
        end = min(len(rows), offset + count)
        result = {"data":rows[offset:end], "nextCursor":f"PRIVATE_HISTORY:{direction}:{end}" if end < len(rows) else None}
        if self.next_override is not None:
            result["nextCursor"] = self.next_override
        await self.native.send(link, {"id":request["id"], "result":result})

    def release(self):
        async def reply():
            self.pause = False
            held, self.held = self.held, []
            for link, request in held:
                await self.reply_page(link, request)
        self.native.loop.call_soon_threadsafe(lambda: asyncio.create_task(reply()))
