"""Hold exact native settings replies while other daemon requests continue."""

import asyncio


class NativeSettings:
    def init_settings(self):
        self.settings_enabled = False
        self.settings_hold = set()
        self.settings_pending = []
        self.settings_errors = {}
        self.native_runtime = {"model":"fixture-model", "effort":"low", "summary":None}
        self.settings_confirmed = []

    def native_runtime_response(self):
        return {"model":self.native_runtime["model"],
                "reasoningEffort":self.native_runtime["effort"],
                "summary":self.native_runtime["summary"]}

    async def settings_request(self, link, request):
        method = request["method"]
        if not self.settings_enabled or method not in ("thread/settings/update", "turn/settings/update"):
            return False
        assert request["params"]["threadId"] == self.thread_id
        if method in self.settings_hold:
            self.settings_pending.append((link, request))
        else:
            await self.settings_reply(link, request)
        return True

    async def settings_reply(self, link, request):
        method = request["method"]
        error = self.settings_errors.get(method)
        if not error and method == "thread/settings/update":
            params = request["params"]
            self.native_runtime.update({key:params[key] for key in ("model", "effort", "summary") if key in params})
            self.settings_confirmed.append(dict(params))
        value = {"error":error} if error else {"result":{} if method == "thread/settings/update" else {"status":"applied"}}
        await self.send(link, {"id":request["id"], **value})

    def release_settings(self):
        async def release():
            pending, self.settings_pending = self.settings_pending, []
            self.settings_hold.clear()
            for link, request in pending:
                await self.settings_reply(link, request)
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
