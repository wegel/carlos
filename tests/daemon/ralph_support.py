"""Create a private native peer with explicit running turns and Ralph marker replies."""

import asyncio
from pathlib import Path

from support import Client, Fixture, wait_for


class RalphFixture:
    def ralph_session(self, name):
        fixture = Fixture(name, codex=True)
        self.addCleanup(fixture.close)
        fixture.native.goals_enabled = True
        fixture.native.queue_enabled = True
        fixture.ralph_texts = []
        fixture.ralph_active = None
        fixture.ralph_marker = "DONE_SPIKE"

        async def prompt(link, request):
            text = request["params"]["input"][0]["text"]
            fixture.ralph_texts.append(text)
            turn = "live" if text == "working" else f"ralph-{len(fixture.ralph_texts)}"
            await fixture.native.notify(link, "turn/started", turn={"id":turn})
            if text == "working":
                fixture.ralph_active = turn
                return
            delta = "automatic result\n" + fixture.ralph_marker if text == "automatic" else "Answer: " + text
            await fixture.native.notify(link, "item/agentMessage/delta", turnId=turn,
                                        itemId="answer-"+turn, delta=delta)
            await fixture.native.notify(link, "turn/completed", turn={"id":turn, "status":"completed"})

        fixture.native.prompt = prompt
        first, other = Client(fixture, "ralph-first"), Client(fixture, "ralph-other")
        self.addCleanup(first.close)
        self.addCleanup(other.close)
        create = first.operation(1)
        session = first.call("session/create", operation=create, backend="codex",
                             cwd=str(fixture.root), name=None)["session"]
        self.assertEqual(first.outcome(create)["state"]["kind"], "accepted")
        def ready():
            page = first.call("session/subscribe", session=session, since=None)["snapshot"]
            queue = first.call("queue/read", session=session)["queue"]
            return (page["info"]["availability"] == "ready"
                    and first.call("goal/read", session=session)["goal"]["knowledge"] == "known"
                    and queue["known"] and not queue["reading"])
        wait_for(ready, "native startup and known inactive goal")
        other.call("session/subscribe", session=session, since=None)
        return fixture, first, other, session

    @staticmethod
    def ralph_options(path="seed.md"):
        return {"prompt_path":path, "done_marker":"DONE_SPIKE",
                "blocked_marker":"BLOCK_SPIKE", "waiting_marker":"WAIT_SPIKE"}

    @staticmethod
    def ralph_snapshot(client, session):
        return client.call("session/subscribe", session=session, since=None)["snapshot"]

    def start_ralph_working(self, fixture, client, session, counter=2):
        operation = client.operation(counter)
        client.call("session/send", session=session, operation=operation, text="working")
        self.assertEqual(client.outcome(operation)["state"]["kind"], "accepted")
        wait_for(lambda:self.ralph_snapshot(client, session)["info"]["current_turn"], "original turn running")

    @staticmethod
    def finish_ralph_native(fixture, turn="live"):
        async def finish():
            for link in list(fixture.native.connections):
                await fixture.native.notify(link, "turn/completed", turn={"id":turn, "status":"completed"})
        asyncio.run_coroutine_threadsafe(finish(), fixture.native.loop).result(timeout=3)
