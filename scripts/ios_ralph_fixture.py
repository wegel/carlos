"""Run one real automatic prompt until the phone explicitly stops its native turn."""

PROMPT = "phone ralph working"


def configure_ralph(fixture):
    directory = fixture.root / ".agents"
    directory.mkdir(exist_ok=True)
    (directory / "ralph-prompt.md").write_text(PROMPT)
    native = fixture.native
    native.goals_enabled = True
    native.queue_enabled = True
    original = native.prompt

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != PROMPT:
            await original(link, request)
            return
        native.prompts += 1
        await native.notify(link, "turn/started", turn={"id": "live"})
        await native.notify(link, "item/agentMessage/delta", turnId="live",
                            itemId="phone-ralph-answer", delta="Phone Ralph is working 🌍")

    native.prompt = prompt
