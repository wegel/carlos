"""Fixed quota values with reset times relative to the terminal test's clock."""

import time


def window(used, days):
    return {"usedPercent": used, "windowDurationMins": 10080,
            "resetsAt": int(time.time() + days * 86400)}


def reply_limits(server, request):
    mode = server.mode
    if mode == "weekly-delayed":
        server.quota_request = request
        return
    if mode == "weekly-error":
        server.reply(request, error="Quota unavailable with this login")
        return
    days = {"weekly-yellow": 3.5, "weekly-red": 6.0, "weekly-expired": -1.0}.get(mode, 2.1)
    snapshot = {"limitId": "codex", "primary": {"usedPercent": 20, "windowDurationMins": 300},
                "secondary": window(60, days)} if mode.startswith("weekly-") else {}
    if mode == "weekly-missing":
        snapshot["secondary"]["resetsAt"] = None
    server.reply(request, {"rateLimits": {"limitId": "codex_other", "secondary": window(99, 6)},
                          "rateLimitsByLimitId": {"codex": snapshot}})
    if mode.startswith("weekly-"):
        thread_id = "thread-new" if "thread-new" in server.threads else "thread-original"
        server.notify("thread/tokenUsage/updated", {"threadId": thread_id, "tokenUsage": {
            "modelContextWindow": 100000, "total": {"totalTokens": 91000}}})


def after_turn(server, text):
    if server.mode == "weekly-delayed" and getattr(server, "quota_request", None):
        server.reply(server.quota_request, {"rateLimits": {"secondary": window(60, 2.1)}})
        server.quota_request = None
    if server.mode != "weekly-updates":
        return
    if text == "update quota":
        server.notify("account/rateLimits/updated", {"rateLimits": {
            "limitId": "codex_other", "secondary": window(1, 1)}})
        server.notify("account/rateLimits/updated", {"rateLimits": {
            "limitId": "codex", "primary": {"usedPercent": 30, "windowDurationMins": 300},
            "secondary": None}})
    elif text == "use quota":
        server.notify("account/rateLimits/updated", {"rateLimits": {
            "limitId": "codex", "secondary": {"usedPercent": 90}}})
    elif text == "expire quota":
        server.notify("account/rateLimits/updated", {"rateLimits": {
            "limitId": "codex", "secondary": {"usedPercent": 90, "resetsAt": int(time.time())-1}}})
