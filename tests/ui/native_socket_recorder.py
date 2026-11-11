"""Record exact per-connection native RPCs over standard local WebSocket links."""

import asyncio
import json
import threading

from websockets.asyncio.client import unix_connect
from websockets.asyncio.server import unix_serve


class SocketRecorder:
    def __init__(self,root,path,native):
        self.root,self.path,self.native=root,path,native
        self.connections={}
        self.upstreams={}
        self.handlers=set()
        self.serial=0
        self.ready=threading.Event()
        self.errors=[]
        self.loop=asyncio.new_event_loop()
        self.thread=threading.Thread(target=self.run,daemon=True)
        self.thread.start()
        assert self.ready.wait(5),"local recording endpoint startup"
        assert not self.errors,self.errors

    def run(self):
        asyncio.set_event_loop(self.loop)
        try: self.loop.run_until_complete(self.serve())
        except Exception as error:
            self.errors.append(str(error))
            self.ready.set()
        finally: self.loop.close()

    async def serve(self):
        self.done=self.loop.create_future()
        async with unix_serve(self.accept,path=str(self.path),compression=None,
                              max_size=8*1024*1024,max_queue=2,open_timeout=2,close_timeout=.5):
            self.ready.set()
            try:
                await self.done
            finally:
                links=[*self.connections.values(),*self.upstreams.values()]
                await asyncio.gather(*(link.close() for link in links))
                handlers=list(self.handlers)
                for handler in handlers: handler.cancel()
                await asyncio.gather(*handlers,return_exceptions=True)

    async def accept(self,client):
        handler=asyncio.current_task()
        self.handlers.add(handler)
        self.serial+=1
        identity=self.serial
        self.connections[identity]=client
        try:
            async with unix_connect(path=str(self.native),uri="ws://localhost/rpc",compression=None,max_size=64*1024*1024,max_queue=2,
                                    open_timeout=2,close_timeout=.5) as native:
                self.upstreams[identity]=native
                async def copy(source,destination,direction):
                    async for raw in source:
                        with (self.root/"rpc.jsonl").open("a") as log:
                            log.write(json.dumps({"connection":identity,"direction":direction,"message":json.loads(raw)})+"\n")
                        await destination.send(raw)
                jobs=[asyncio.create_task(copy(client,native,"client")),asyncio.create_task(copy(native,client,"server"))]
                try:
                    await asyncio.wait(jobs,return_when=asyncio.FIRST_COMPLETED)
                finally:
                    for job in jobs: job.cancel()
                    await asyncio.gather(*jobs,return_exceptions=True)
        finally:
            self.connections.pop(identity,None)
            self.upstreams.pop(identity,None)
            self.handlers.discard(handler)

    def disconnect(self):
        identity=max(self.connections)
        asyncio.run_coroutine_threadsafe(self.connections[identity].close(code=1012,reason="isolated reconnect fixture"),self.loop).result(timeout=3)

    def close(self):
        if self.loop.is_running():
            self.loop.call_soon_threadsafe(lambda: None if self.done.done() else self.done.set_result(None))
        self.thread.join(timeout=5)
        assert not self.thread.is_alive(),"socket recorder stopped"
        assert not self.errors,self.errors
