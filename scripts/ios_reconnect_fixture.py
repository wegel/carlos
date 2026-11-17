"""Withhold an actual phone result, then restart only its private test daemon."""

import asyncio
from contextlib import contextmanager
import json
import secrets
from threading import Event, Thread

from websockets.asyncio.client import connect
from websockets.asyncio.server import serve
from websockets.datastructures import Headers
from websockets.exceptions import ConnectionClosed
from websockets.http11 import Response

from support import Client, wait_for


class ReconnectRelay:
    def __init__(self, fixture):
        self.fixture = fixture
        self.control = "/restart/" + secrets.token_hex(24)
        self.links = set()
        self.operation = None
        self.accepted = None
        self.restarted = False
        self.commands = []
        self.ready = Event()
        self.loop = asyncio.new_event_loop()
        self.thread = Thread(target=self.run, daemon=True)

    def run(self):
        asyncio.set_event_loop(self.loop)
        self.loop.run_until_complete(self.listen())
        self.loop.close()

    async def listen(self):
        self.done = self.loop.create_future()
        self.accepted = asyncio.Event()
        async with serve(self.relay, "127.0.0.1", 0, process_request=self.request,
                         compression=None, max_size=8 * 1024 * 1024, ping_interval=None) as server:
            self.port = server.sockets[0].getsockname()[1]
            self.ready.set()
            await self.done

    async def request(self, _connection, request):
        if request.headers.get("Upgrade", "").lower() == "websocket":
            return None
        if request.path != self.control or self.restarted:
            return Response(404, "Not Found", Headers(), b"")
        await asyncio.wait_for(self.accepted.wait(), timeout=8)
        await asyncio.gather(*(link.close() for link in list(self.links)))
        await asyncio.to_thread(self.fixture.stop)
        await asyncio.to_thread(self.fixture.start)
        await asyncio.to_thread(self.stop_restored_session)
        self.restarted = True
        return Response(200, "OK", Headers(), b"")

    def stop_restored_session(self):
        reader = Client(self.fixture, "phone-stop-operator")
        try:
            session = reader.call("sessions/watch")["sessions"][0]["id"]
            def ready():
                page = reader.call("session/subscribe", session=session, since=None)["snapshot"]
                return page["info"]["availability"] == "ready"
            wait_for(ready, "actual Codex recovery becomes ready before the explicit stop")
            operation = reader.operation(1)
            reader.call("session/stop", operation=operation, session=session)
            assert reader.outcome(operation)["state"]["kind"] == "accepted"
        finally:
            reader.close()

    async def relay(self, phone):
        self.links.add(phone)
        hold = False
        try:
            async with connect(self.fixture.ws_uri, compression=None, proxy=None,
                               max_size=8 * 1024 * 1024, ping_interval=None) as daemon:
                async def outgoing():
                    nonlocal hold
                    async for raw in phone:
                        message = json.loads(raw)
                        self.commands.append(message.get("method"))
                        if message.get("method") == "session/send":
                            self.operation = message["params"]["operation"]
                            hold = True
                        await daemon.send(raw)

                async def incoming():
                    async for raw in daemon:
                        message = json.loads(raw)
                        if hold and message.get("method") == "operation/updated":
                            result = message["params"]
                            if result.get("operation") == self.operation and result.get("state", {}).get("kind") == "accepted":
                                self.accepted.set()
                        if not hold:
                            await phone.send(raw)

                tasks = [asyncio.create_task(outgoing()), asyncio.create_task(incoming())]
                try:
                    await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
                finally:
                    for task in tasks:
                        task.cancel()
                    await asyncio.gather(*tasks, return_exceptions=True)
        except ConnectionClosed:
            pass
        finally:
            self.links.discard(phone)


@contextmanager
def reconnect_relay(fixture):
    relay = ReconnectRelay(fixture)
    relay.thread.start()
    assert relay.ready.wait(5), "owned phone relay started"
    try:
        yield relay
    finally:
        relay.loop.call_soon_threadsafe(relay.done.set_result, None)
        relay.thread.join(timeout=8)
        assert not relay.thread.is_alive(), "owned relay stopped"


def check_reconnect(fixture, relay):
    assert relay.restarted and relay.commands.count("session/send") == 1, relay.commands
    assert fixture.native.started == 1 and fixture.native.resumed == 2
    reader = Client(fixture, "phone-independent-reader")
    try:
        session = reader.call("sessions/watch")["sessions"][0]["id"]
        snapshot = reader.call("session/subscribe", session=session, since=None)["snapshot"]
        assert "phone private draft 🌍" not in json.dumps(snapshot, ensure_ascii=False)
        foreign = reader.response(reader.send("operation/read", operation=relay.operation))
        assert "error" in foreign, foreign
    finally:
        reader.close()
