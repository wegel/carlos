"""Add fixed eligible tool commands to the existing native peer without another backend shim."""

import asyncio
import json
from pathlib import Path
import sys

COMMAND = "python3 - <<'PY'\nfrom pathlib import Path\ntext = 'report 🌍'\nPath('report.txt').write_text(text)\nprint('source marker')\nPY"
OUTPUT = "original output marker 🌍\n\x1b[31mred source\x1b[0m\n"
LABEL = "Rewrite the result report"


def configure(fixture):
    path = Path(fixture.env["XDG_CONFIG_HOME"]) / "carlos/tool-summaries.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"provider":"command", "executable":sys.executable,
        "arguments":[str(Path(__file__).with_name("native_label_provider.py")), str(fixture.root)],
        "timeout_seconds":20}))
    native = fixture.native
    original_prompt, original_history = native.prompt, native.history
    native.summary_history = False

    def history():
        result = original_history()
        if native.summary_history:
            result["turns"].append({"id":"summary-turn", "status":"completed", "items":[item()]})
        return result

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != "summarize command":
            await original_prompt(link, request)
            return
        native.prompts += 1
        native.summary_history = True
        await native.notify(link, "turn/started", turn={"id":"summary-turn"})
        await native.notify(link, "item/completed", turnId="summary-turn", item=item())
        await native.notify(link, "turn/completed", turn={"id":"summary-turn", "status":"completed"})

    native.history, native.prompt = history, prompt


def item():
    return {"id":"summary-tool", "type":"commandExecution", "command":COMMAND,
            "aggregatedOutput":OUTPUT, "exitCode":0, "status":"completed"}


def announce(fixture, method, **params):
    async def send():
        for link in list(fixture.native.connections):
            await fixture.native.notify(link, method, **params)
    asyncio.run_coroutine_threadsafe(send(), fixture.native.loop).result(timeout=3)
