"""Prove phone send modes and explicit old-message forks with the owned native Codex peer."""


def configure_prompt_controls(native, workflow):
    original_history, original_prompt, original_handle = native.history, native.prompt, native.handle
    native.history_user = True

    def history():
        thread = original_history()
        if not native.forked:
            thread["turns"][0]["items"] = [
                {"id":"native-user", "type":"userMessage", "content":[{"type":"text", "text":"original prompt"}]},
                {"id":"original-answer", "type":"agentMessage", "phase":"final_answer", "text":"Original answer before branch"},
            ]
        return thread

    async def prompt(link, request):
        text = request["params"]["input"][0]["text"]
        if text == "phone mode run":
            native.prompts += 1
            await native.notify(link, "turn/started", turn={"id":"live"})
            await native.notify(link, "item/agentMessage/delta", turnId="live", itemId="phone-modes", delta="Phone mode response running")
        elif request["method"] == "turn/steer":
            assert text == "phone steer exact 🌍", request
            assert request["params"]["expectedTurnId"] == "live", request
            await native.notify(link, "item/agentMessage/delta", turnId="live", itemId="phone-modes", delta="\nSteer accepted")
        else:
            await original_prompt(link, request)

    async def handle(link, request):
        if workflow == "edit-rejected" and request.get("method") == "thread/fork":
            await native.send(link, {"id":request["id"], "error":{"code":-32602, "message":"Phone branch rejected"}})
            return
        await original_handle(link, request)
        if request.get("method") == "turn/interrupt":
            assert request["params"]["turnId"] == "live", request
            await native.notify(link, "turn/completed", turn={"id":"live", "status":"interrupted"})

    native.history, native.prompt, native.handle = history, prompt, handle


def check_prompt_controls(native, workflow):
    requests = native.requests
    forks = [request for request in requests if request.get("method") == "thread/fork"]
    if workflow == "send-mode":
        steers = [request for request in requests if request.get("method") == "turn/steer"]
        assert len(steers) == 1 and steers[0]["params"]["input"][0]["text"] == "phone steer exact 🌍", steers
        assert native.interrupts == 1 and not forks
        stopped = next(index for index, request in enumerate(requests) if request.get("method") == "turn/interrupt")
        queued = next(index for index, request in enumerate(requests) if request.get("method") == "turn/start"
                      and request["params"]["input"][0]["text"] == "phone queued exact 🌍")
        assert stopped < queued, "Queue must wait for the original response to stop"
    else:
        assert len(forks) == 1, "Opening and cancelling an editor must never fork"
        assert forks[0]["params"]["beforeTurnId"] == "history-turn", forks
        assert native.forked == (workflow == "edit"), "A rejected fork must leave the original conversation intact"
        assert native.interrupts == 0
