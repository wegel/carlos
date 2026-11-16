"""Current Codex file changes and complete MCP request/result data."""

import asyncio
import copy

PATCH = {"id":"native-patch", "type":"fileChange", "status":"completed", "changes":[
    {"path":"src/one.rs", "kind":{"type":"update", "movePath":None},
     "diff":"@@ -1 +1 @@\n-old\n+new 🌍\n"},
    {"path":"src/two.rs", "kind":{"type":"add"}, "diff":"@@ -0,0 +1 @@\n+second 🌍\n"},
]}
MCP = {"id":"native-mcp", "type":"mcpToolCall", "server":"fixture", "tool":"lookup",
       "arguments":{"query":"native 🌍", "command":"bash -lc 'keep this literal'"},
       "status":"completed", "result":{"content":[
           {"type":"text", "text":"native MCP response 🌍\nsecond line"},
           {"type":"resource", "resource":{"uri":"fixture://source", "text":"whole resource"}},
       ], "structuredContent":{"answer":42, "diff":"@@ -1 +1 @@\n-before\n+after\n"}},
       "error":None}


class NativeItems:
    def __init__(self, native, items=None, hold=False):
        self.native = native
        self.items = copy.deepcopy(items if items is not None else [PATCH, MCP])
        self.hold = hold
        self.link = None
        native.prompt = self.prompt

    async def prompt(self, link, request):
        self.native.prompts += 1
        self.link = link
        await self.native.notify(link, "turn/started", turn={"id":"native-items"})
        for item in self.items:
            await self.native.notify(link, "item/started" if self.hold else "item/completed",
                                     turnId="native-items", item=item)
        if not self.hold:
            await self.complete()

    async def complete(self):
        if self.hold:
            await self.native.notify(self.link, "item/completed", turnId="native-items", item=MCP)
        await self.native.notify(self.link, "turn/completed", turn={"id":"native-items", "status":"completed"})

    def finish(self):
        self.native.loop.call_soon_threadsafe(lambda: asyncio.create_task(self.complete()))
