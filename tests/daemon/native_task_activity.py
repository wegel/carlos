"""Opt-in bounded native child items and live deltas for actual daemon/terminal checks."""

import asyncio


class NativeTaskActivity:
    def init_task_activity(self):
        self.task_items = {}
        self.task_item_hold = set()
        self.task_item_pending = {}
        self.task_item_errors = set()

    async def task_activity_request(self, link, request):
        if request["method"] != "thread/items/list":
            return False
        params = request["params"]
        thread = params["threadId"]
        if thread == self.thread_id:
            return False
        assert params["sortDirection"] == "desc" and params["limit"] == 1
        if thread in self.task_item_errors:
            await self.send(link, {"id":request["id"], "error":{"code":-32000,"message":"PRIVATE_CHILD_READ_FAILED"}})
            return True
        data = self.task_items.get(thread, [])
        offset = int((params.get("cursor") or "child:0").split(":")[1])
        end = min(len(data), offset + params["limit"])
        result = {"data":data[offset:end], "nextCursor":f"child:{end}" if end < len(data) else None}
        if offset == 0 and thread in self.task_item_hold:
            self.task_item_pending[request["id"]] = (link, result, thread)
        else:
            await self.send(link, {"id":request["id"], "result":result})
        return True

    def release_child_items(self, last=False):
        async def release():
            identity = list(self.task_item_pending)[-1 if last else 0]
            link, result, _ = self.task_item_pending.pop(identity)
            await self.send(link, {"id":identity, "result":result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    def child_delta(self, thread, text):
        async def delta():
            for link in list(self.connections):
                await self.notify(link, "item/agentMessage/delta", threadId=thread,
                                  turnId="child-turn", itemId="live-child-item", delta=text)
        asyncio.run_coroutine_threadsafe(delta(), self.loop).result(timeout=3)


def child_item(index, prefix="CHILD_DETAIL_ONLY"):
    return {"turnId":"child-turn", "item":{"id":f"child-item-{index}", "type":"agentMessage", "text":f"{prefix}_{index} 日本語"}}
