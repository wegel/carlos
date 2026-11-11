"""Keep the local wire recorder bounded even when its upstream ignores close frames."""

import asyncio
from pathlib import Path
import tempfile
import time
import unittest

from websockets.asyncio.client import unix_connect
from websockets.asyncio.server import unix_serve

from native_socket_recorder import SocketRecorder


class SocketRecorderCleanup(unittest.TestCase):
    def test_unresponsive_upstream_does_not_outlive_the_recorder(self):
        with tempfile.TemporaryDirectory(prefix="carlos-recorder-close-") as directory:
            root=Path(directory)
            async def verify():
                peer_ready=asyncio.Event()
                release=asyncio.Event()
                async def ignore_close(peer):
                    peer.transport.pause_reading()
                    peer_ready.set()
                    await release.wait()
                    peer.transport.abort()
                async with unix_serve(ignore_close,path=str(root/"native.sock"),close_timeout=.5):
                    recorder=SocketRecorder(root,root/"client.sock",root/"native.sock")
                    try:
                        async with unix_connect(path=str(root/"client.sock"),uri="ws://localhost/rpc",close_timeout=.5):
                            await asyncio.wait_for(peer_ready.wait(),3)
                            started=time.monotonic()
                            await asyncio.to_thread(recorder.close)
                            self.assertLess(time.monotonic()-started,3)
                            self.assertFalse(recorder.handlers)
                            self.assertFalse(recorder.connections)
                            self.assertFalse(recorder.upstreams)
                    finally:
                        release.set()
            asyncio.run(verify())


if __name__ == "__main__":
    unittest.main()
