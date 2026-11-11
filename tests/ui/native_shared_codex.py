"""Own only an isolated fixture server; Carlos owns its separately recorded local proxy."""

import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

from mock_responses import Model
from terminal import Terminal
from native_socket_recorder import SocketRecorder


def launch_shared(case,binary,codex,root,reply,feature_config=""):
    root.mkdir(parents=True)
    model=Model(root,reply)
    case.addCleanup(model.close)
    config=root/"codex-home/config.toml"
    extra=feature_config(root) if callable(feature_config) else feature_config
    config.write_text(config.read_text()+extra)
    terminal,server,endpoint=attach_shared(case,binary,codex,root)
    return terminal,model,server,endpoint


def attach_shared(case,binary,codex,root,resume=None,width=100):
    """Attach Carlos to an owned native fixture with its existing config and sessions."""
    sockets=tempfile.TemporaryDirectory(prefix="carlos-uds-")
    case.addCleanup(sockets.cleanup)
    socket=Path(sockets.name)/"native.sock"
    endpoint=Path(sockets.name)/"client.sock"
    env={key:value for key,value in os.environ.items() if not key.startswith(("OPENAI_","CODEX_","CARLOS_"))}
    env.update({"HOME":str(root),"CODEX_HOME":str(root/"codex-home")})
    errors=(root/"shared-server-stderr.log").open("w")
    case.addCleanup(errors.close)
    server=subprocess.Popen([str(codex),"app-server","--listen","unix://"+str(socket)],
        env=env,cwd=root,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=errors,start_new_session=True)
    def close_server():
        if server.poll() is None:
            os.killpg(server.pid,signal.SIGTERM)
            try: server.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid,signal.SIGKILL)
                server.wait(timeout=3)
    case.addCleanup(close_server)
    deadline=time.monotonic()+5
    while not socket.exists() and time.monotonic()<deadline:
        if server.poll() is not None: raise AssertionError((root/"shared-server-stderr.log").read_text())
        time.sleep(.01)
    case.assertTrue(socket.exists(),"isolated native Unix listener")
    recorder=SocketRecorder(root,endpoint,socket)
    case.addCleanup(recorder.close)
    terminal=Terminal(binary,root,resume=resume,codex=codex,width=width,extra_args=["--codex-socket",str(endpoint)])
    terminal.socket_recorder=recorder
    case.addCleanup(terminal.close)
    terminal.wait(lambda:terminal.input_is(""),"attached composer")
    return terminal,server,endpoint


def drop_connection(terminal):
    terminal.socket_recorder.disconnect()
    terminal.contains("connection lost")
    terminal.contains("F5 reconnect")
