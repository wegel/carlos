"""A bounded local WebSocket peer for the real Codex adapter, with causal send/control gates."""

import asyncio
import json
import threading

from websockets.asyncio.server import unix_serve
from native_queue import NativeQueue


class NativeCodex(NativeQueue):
    def __init__(self, root):
        self.init_queue()
        self.root = root
        self.path = root / "native.sock"
        self.requests = []
        self.errors = []
        self.connections = set()
        self.started = 0
        self.resumed = 0
        self.answers = 0
        self.interrupts = 0
        self.prompts = 0
        self.history_padding = ""
        self.history_user = False
        self.thread_id = "native-root"
        self.forked = False
        self.tool_first = "chunk-a\n"
        self.tool_second = "chunk-b\n"
        self.hold_interrupts = False
        self.goals_enabled = False
        self.goal = None
        self.goal_hold = False
        self.goal_pending = None
        self.ready = threading.Event()
        self.loop = asyncio.new_event_loop()
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()
        assert self.ready.wait(5), "native fixture listens"
        assert not self.errors, self.errors

    def run(self):
        asyncio.set_event_loop(self.loop)
        try:
            self.loop.run_until_complete(self.serve())
        except Exception as error:
            self.errors.append(repr(error))
            self.ready.set()
        finally:
            self.loop.close()

    async def serve(self):
        self.done = self.loop.create_future()
        async with unix_serve(self.accept, path=str(self.path), compression=None,
                              max_size=8*1024*1024, max_queue=2, close_timeout=.2):
            self.ready.set()
            await self.done
            await asyncio.gather(*(link.close() for link in list(self.connections)))

    def record(self, direction, message):
        with (self.root / "codex.jsonl").open("a") as log:
            log.write(json.dumps({"direction": direction, "message": message}) + "\n")

    async def send(self, link, value):
        self.record("server", value)
        await link.send(json.dumps(value))

    async def notify(self, link, method, **params):
        await self.send(link, {"method": method, "params": {"threadId": self.thread_id, **params}})

    def history(self):
        turns = [{"id": "history-turn", "status": "completed", "items": [
            {"id": f"history-{index}", "type": "agentMessage", "phase": "final_answer", "text": f"history {index}" + self.history_padding}
            for index in range(130)]}]
        turns[0]["items"].append({"id": "history-tool", "type": "commandExecution", "command": "cat history.txt",
                                  "aggregatedOutput": "".join(f"history-output {n}\n" for n in range(10000)), "exitCode": 0})
        if self.history_user:
            turns[0]["items"].insert(0, {"id": "native-user", "type": "userMessage", "content": [{"type": "text", "text": "original prompt"}]})
        if self.forked:
            turns = []
        return {"id": self.thread_id, "status": {"type": "idle"}, "turns": turns,
                "model": "fixture-model", "reasoningEffort": "low"}

    async def accept(self, link):
        self.connections.add(link)
        held = None
        held_interrupts = []
        controlled = set()
        try:
            async for raw in link:
                request = json.loads(raw)
                self.record("client", request)
                self.requests.append(request)
                if "method" not in request:
                    assert request["id"] == "card" and self.answers == 0, request
                    self.answers += 1
                    controlled.add("answer")
                elif request["method"] == "turn/start" and request["params"]["input"][0]["text"] == "held":
                    assert held is None
                    held = request["id"]
                    await self.held_facts(link)
                else:
                    if request["method"] == "turn/interrupt":
                        self.interrupts += 1
                        controlled.add("interrupt")
                    if request["method"] == "turn/interrupt" and self.hold_interrupts and "answer" not in controlled:
                        held_interrupts.append(request["id"])
                    else:
                        await self.handle(link, request)
                if "answer" in controlled:
                    for identity in held_interrupts:
                        await self.send(link, {"id": identity, "result": {}})
                    held_interrupts.clear()
                if held is not None and controlled == {"answer", "interrupt"}:
                    await self.send(link, {"id": held, "result": {"turn": {"id": "live"}}})
                    held = None
        except Exception as error:
            if type(error).__name__ not in ("ConnectionClosedOK", "ConnectionClosedError"):
                self.errors.append(repr(error))
        finally:
            self.connections.discard(link)

    async def handle(self, link, request):
        method = request["method"]
        if "id" not in request:
            return
        result = {}
        if method == "thread/start":
            self.started += 1
            result = {"thread": self.history(), "model": "fixture-model", "reasoningEffort": "low"}
        elif method in ("thread/resume", "thread/read"):
            self.resumed += method == "thread/resume"
            result = {"thread": self.history(), "model": "fixture-model", "reasoningEffort": "low"}
        elif method == "thread/turns/list":
            result = {"data": self.history()["turns"], "nextCursor": None}
        elif method == "thread/fork":
            assert request["params"]["beforeTurnId"] == "history-turn"
            self.thread_id = "native-fork"
            self.forked = True
            self.goal = None
            self.queue_entries = []
            result = {"thread": self.history()}
        elif method == "thread/name/set":
            if request["params"]["name"] == "reject rename":
                await self.send(link, {"id": request["id"], "error": {"code": -32602, "message": "fixture rejected rename"}})
                return
        elif method in ("thread/goal/get", "thread/goal/set", "thread/goal/clear"):
            await self.goal_request(link, request)
            return
        elif method.startswith("thread/queue/"):
            await self.queue_request(link, request)
            return
        elif method in ("model/list", "thread/list", "thread/backgroundTerminals/list"):
            result = {"data": [], "nextCursor": None}
        elif method in ("turn/start", "turn/steer"):
            await self.prompt(link, request)
            result = {"turn": {"id": "live"}}
        elif method not in ("initialize", "turn/interrupt"):
            await self.send(link, {"id": request["id"], "error": {"code": -32601, "message": "unsupported fixture method"}})
            return
        await self.send(link, {"id": request["id"], "result": result})

    async def goal_request(self, link, request):
        if not self.goals_enabled:
            await self.send(link, {"id": request["id"], "error": {"code": -32601, "message": "goals feature is disabled"}})
            return
        method, params = request["method"], request["params"]
        assert params["threadId"] == self.thread_id
        if method == "thread/goal/set":
            if params.get("objective") == "reject objective":
                await self.send(link, {"id": request["id"], "error": {"code": -32602, "message": "PRIVATE_GOAL_REJECTION"}})
                return
            previous = self.goal or {"threadId": self.thread_id, "objective": "", "status": "paused",
                "tokenBudget": None, "tokensUsed": 9007199254740993, "timeUsedSeconds": 7,
                "createdAt": 1700000000, "updatedAt": 1700000000}
            self.goal = {**previous, **{key: params[key] for key in ("objective", "tokenBudget", "status") if key in params}}
            self.goal["updatedAt"] += 1
            result = {"goal": dict(self.goal)}
            await self.notify(link, "thread/goal/updated", goal=self.goal)
            if self.goal_hold:
                assert self.goal_pending is None
                self.goal_pending = (link, request["id"], result)
                return
        elif method == "thread/goal/clear":
            self.goal = None
            result = {"cleared": True}
        else:
            result = {"goal": self.goal}
        await self.send(link, {"id": request["id"], "result": result})

    def release_goal(self):
        async def release():
            link, identity, result = self.goal_pending
            self.goal_pending = None
            self.goal_hold = False
            await self.send(link, {"id": identity, "result": result})
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)

    async def held_facts(self, link):
        await self.notify(link, "turn/started", turn={"id": "live"})
        await self.notify(link, "item/agentMessage/delta", itemId="waiting", turnId="live", delta="fact while send waits")
        await self.notify(link, "item/started", turnId="live", item={"type": "commandExecution", "id": "live-tool", "command": "tail -f log", "status": "inProgress"})
        await self.notify(link, "item/commandExecution/outputDelta", turnId="live", itemId="live-tool", delta=self.tool_first)
        await self.send(link, {"id": "card", "method": "item/commandExecution/requestApproval", "params": {
            "threadId": "native-root", "turnId": "live", "itemId": "live-tool", "command": "true"}})

    async def prompt(self, link, request):
        text = request["params"]["input"][0]["text"]
        if request["method"] == "turn/steer":
            assert self.answers == 1 and self.interrupts == 1, "ordinary send passed held reply"
            await self.notify(link, "item/commandExecution/outputDelta", turnId="live", itemId="live-tool", delta=self.tool_second)
            return
        self.prompts += 1
        turn = f"turn-{self.prompts}"
        await self.notify(link, "turn/started", turn={"id": turn})
        if text == "format-facts":
            items = [
                {"type": "userMessage", "id": "wire-first", "content": [{"type": "text", "text": "wire first"}]},
                {"type": "userMessage", "id": "wire-steer", "content": [{"type": "text", "text": "wire second"}]},
                {"type": "toolCall", "id": "wire-call", "tool": "Bash", "name": "Bash", "input": {"command": "pwd"}},
                {"type": "toolOutput", "id": "wire-diff", "files": [
                    {"filePath": "src/é.rs", "diff": "@@ -1 +1 @@\n-old\n+new 🌍\n"}]},
            ]
            for item in items:
                await self.notify(link, "item/completed", turnId=turn, item=item)
        if text == "many-rows":
            for index in range(1500):
                await self.notify(link, "item/completed", turnId=turn, item={
                    "type": "agentMessage", "id": f"row-{self.prompts}-{index}", "phase": "final_answer", "text": f"row {index}"})
        if text == "large-plain-source":
            for _ in range(224):
                await self.notify(link, "item/agentMessage/delta", turnId=turn, itemId="large-source", delta="\0🌍\x1b[31m\n"*4096)
        if text == "flood-tool":
            await self.notify(link, "item/started", turnId=turn, item={"type": "commandExecution", "id": "flood-tool", "command": "cat big.log", "status": "inProgress"})
            for index in range(256):
                await self.notify(link, "item/commandExecution/outputDelta", turnId=turn, itemId="flood-tool", delta=f"block {index}\n" + "x"*65520 + "\n")
            await self.notify(link, "item/completed", turnId=turn, item={"type": "commandExecution", "id": "flood-tool", "command": "cat big.log", "status": "completed", "exitCode": 0})
        await self.notify(link, "item/agentMessage/delta", itemId=f"answer-{self.prompts}", turnId=turn, delta="Answer: " + text)
        await self.notify(link, "turn/completed", turn={"id": turn, "status": "completed"})

    def close(self):
        if self.loop.is_running():
            self.loop.call_soon_threadsafe(lambda: None if self.done.done() else self.done.set_result(None))
        self.thread.join(timeout=5)
        assert not self.thread.is_alive(), "native fixture stopped"
        assert not self.errors, self.errors

    def disconnect(self):
        async def close_links():
            await asyncio.gather(*(link.close(code=1012, reason="fixture disconnect") for link in list(self.connections)))
        asyncio.run_coroutine_threadsafe(close_links(), self.loop).result(timeout=3)
