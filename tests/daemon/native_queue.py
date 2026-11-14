"""Keep native queue payloads intact while holding acknowledgements independently of other work."""

import asyncio


class NativeQueue:
    def init_queue(self):
        self.queue_enabled = False
        self.queue_entries = []
        self.queue_serial = 0
        self.queue_hold = False
        self.queue_pending = None

    async def queue_request(self, link, request):
        if not self.queue_enabled:
            await self.send(link, {"id": request["id"], "error": {"code": -32601, "message": "queue feature disabled"}})
            return
        method, params = request["method"], request["params"]
        assert params["threadId"] == self.thread_id
        entries = self.queue_entries
        if method == "thread/queue/list":
            offset = int(params.get("cursor") or 0)
            end = min(len(entries), offset + params["limit"])
            result = {"data": entries[offset:end], "nextCursor": str(end) if end < len(entries) else None}
        else:
            text = (params.get("input") or [{}])[0].get("text")
            if text == "PRIVATE_QUEUE_REJECTION":
                await self.send(link, {"id": request["id"], "error": {"code": -32602, "message": text}})
                return
            if method == "thread/queue/add":
                self.queue_serial += 1
                entry = {"id": f"native-queued-{self.queue_serial}", "clientUserMessageId": params["clientUserMessageId"], "input": params["input"]}
                entries.append(entry)
                result = {"queuedSubmission": entry}
            elif method == "thread/queue/update":
                entry = next(entry for entry in entries if entry["id"] == params["queuedSubmissionId"])
                entry["input"] = params["input"]
                result = {"queuedSubmission": entry}
            elif method == "thread/queue/reorder":
                assert sorted(params["queuedSubmissionIds"]) == sorted(entry["id"] for entry in entries)
                self.queue_entries = [next(entry for entry in entries if entry["id"] == identity) for identity in params["queuedSubmissionIds"]]
                result = {}
            else:
                selected = params["queuedSubmissionId"]
                assert any(entry["id"] == selected for entry in entries)
                self.queue_entries = [entry for entry in entries if entry["id"] != selected]
                result = {"deleted": True} if method == "thread/queue/delete" else {"turn": {"id": "queue-started"}}
            await self.notify(link, "thread/queue/changed")
            if self.queue_hold:
                assert self.queue_pending is None
                self.queue_pending = (link, request["id"], result)
                return
        await self.send(link, {"id": request["id"], "result": result})

    def release_queue(self):
        async def release():
            link, identity, result = self.queue_pending
            self.queue_pending = None
            self.queue_hold = False
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
