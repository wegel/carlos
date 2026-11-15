"""Opt-in saved-session pages and held exact native opens for startup socket proofs."""

import asyncio


class NativeSaved:
    def init_saved(self):
        self.saved_rows = None
        self.saved_page_size = 7
        self.saved_hold_list = False
        self.saved_hold_resume = False
        self.saved_fail_resume = False
        self.saved_pending = []
        self.saved_archives = []
        self.saved_hold_archive = False
        self.saved_reject_archive = False
        self.saved_disconnect_archive = False

    async def saved_request(self, link, request):
        if self.saved_rows is None:
            return False
        method = request["method"]
        if method == "thread/list":
            params = request["params"]
            if "cwd" not in params:
                return False
            assert params["cwd"] == str(self.root), params
            assert params["limit"] == 64 and params["sortKey"] == "updated_at", params
            rows = sorted(self.saved_rows, key=lambda row: (row["updatedAt"], row["createdAt"], row["id"]),
                          reverse=params["sortDirection"] != "asc")
            cursor = params.get("cursor")
            assert cursor is None or cursor.startswith("PRIVATE_NATIVE_CURSOR:"), cursor
            offset = int(cursor.split(":")[1]) if cursor else 0
            page = rows[offset:offset + self.saved_page_size]
            next_offset = offset + len(page)
            result = {"data":page, "nextCursor":f"PRIVATE_NATIVE_CURSOR:{next_offset}" if next_offset < len(rows) else None}
            if self.saved_hold_list:
                self.saved_pending.append(("list", link, request, result))
                (self.root / "saved-list-held").write_text("held")
            else:
                await self.send(link, {"id":request["id"], "result":result})
            return True
        if method in ("thread/resume", "thread/read"):
            if method == "thread/resume":
                self.resumed += 1
            if method == "thread/resume" and self.saved_fail_resume:
                await self.send(link, {"id":request["id"], "error":{"code":-32602, "message":"saved fixture rejected native open"}})
                return True
            self.thread_id = request["params"]["threadId"]
            thread = self.history()
            selected = next((row for row in self.saved_rows if row["id"] == self.thread_id), None)
            if selected:
                thread["name"] = selected.get("name")
            result = {"thread":thread, **self.native_runtime_response()}
            if method == "thread/resume" and self.saved_hold_resume:
                self.saved_pending.append(("resume", link, request, result))
                (self.root / "saved-resume-held").write_text("held")
            else:
                await self.send(link, {"id":request["id"], "result":result})
            return True
        if method == "thread/archive":
            target = request["params"]["threadId"]
            self.saved_archives.append(target)
            if self.saved_reject_archive:
                await self.send(link, {"id":request["id"], "error":{"code":-32602, "message":"fixture rejected archive"}})
                return True
            self.saved_rows = [row for row in self.saved_rows if row["id"] != target]
            if self.saved_disconnect_archive:
                await link.close(code=1012, reason="archive reply lost")
            elif self.saved_hold_archive:
                self.saved_pending.append(("archive", link, request, {}))
            else:
                await self.send(link, {"id":request["id"], "result":{}})
            return True
        return False

    def release_saved(self, kind):
        async def release():
            self.saved_hold_list = self.saved_hold_resume = self.saved_hold_archive = False
            pending, self.saved_pending = self.saved_pending, []
            for target, link, request, result in pending:
                if target != kind:
                    self.saved_pending.append((target, link, request, result))
                    continue
                try:
                    await self.send(link, {"id":request["id"], "result":result})
                except Exception as error:
                    if type(error).__name__ not in ("ConnectionClosedOK", "ConnectionClosedError"):
                        raise
        asyncio.run_coroutine_threadsafe(release(), self.loop).result(timeout=3)
