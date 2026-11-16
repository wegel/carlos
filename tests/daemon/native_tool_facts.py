"""Emit concrete native command/diff outcomes without a model or a UI-side formatter."""

COMMAND = "printf PHONE_TOOL_FACTS"
OUTPUT = "hello 🌍\nworld\n"
COMMAND_SOURCE = f"$ {COMMAND}\n{OUTPUT}\nexit code: 0"
DIFF = "--- a/src/é.rs\n+++ b/src/é.rs\n@@ -1 +1 @@\n-old\n+new 🌍\n"
ITEMS = [
    {"type":"commandExecution", "id":"phone-command", "command":COMMAND,
     "aggregatedOutput":OUTPUT, "status":"completed", "exitCode":0},
    {"type":"toolOutput", "id":"phone-diff", "files":[{"filePath":"src/é.rs", "diff":DIFF}]},
    {"type":"commandExecution", "id":"phone-unknown", "command":"observed-only",
     "aggregatedOutput":"no reported exit\n", "status":"completed"},
]


def configure(native):
    original = native.prompt

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != "tool-facts":
            await original(link, request)
            return
        native.prompts += 1
        turn = "phone-tool-facts"
        await native.notify(link, "turn/started", turn={"id":turn})
        for item in ITEMS:
            await native.notify(link, "item/completed", turnId=turn, item=item)
        await native.notify(link, "turn/completed", turn={"id":turn, "status":"completed"})

    native.prompt = prompt
