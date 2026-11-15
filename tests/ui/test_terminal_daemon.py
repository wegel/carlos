"""Prove the opt-in harness preserves startup arguments, wire logs and private process ownership."""

import json
import os
from pathlib import Path
import tempfile
import textwrap
import unittest

from terminal import Terminal
from terminal_daemon import OwnedDaemon

DAEMON = '''#!/usr/bin/env python3
import json,os,signal,socket,subprocess,sys,threading
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
(root/'owned-daemon-pid').write_text(str(os.getpid()))
child=subprocess.Popen([sys.executable,'-c','import sys; sys.stdin.buffer.read()'],stdin=subprocess.PIPE)
(root/'owned-native-pid').write_text(str(child.pid))
(root/'daemon-args.json').write_text(json.dumps(sys.argv[1:]))
listener=socket.socket(socket.AF_UNIX)
listener.bind(sys.argv[sys.argv.index('--socket')+1])
listener.listen()
def stop(*_):
    child.stdin.close()
    child.wait(timeout=2)
    listener.close()
    raise SystemExit(0)
signal.signal(signal.SIGTERM,stop)
def peer(link):
    with link,link.makefile('rb') as source:
        for line in source:
            request=json.loads(line)
            link.sendall((json.dumps({'jsonrpc':'2.0','id':request['id'],'result':{}})+'\\n').encode())
while True:
    link,_=listener.accept()
    threading.Thread(target=peer,args=(link,),daemon=True).start()
'''

CLIENT = '''#!/usr/bin/env python3
import json,os,socket,sys,tty
from pathlib import Path
root=Path(os.environ['CARLOS_TEST_ROOT'])
tty.setraw(0)
(root/'client-args.json').write_text(json.dumps(sys.argv[1:]))
(root/'rpc.jsonl').write_text(json.dumps({'direction':'client','message':{'id':91,'method':'native-fixture-check'}})+'\\n')
if '--daemon-socket' in sys.argv:
    endpoint=sys.argv[sys.argv.index('--daemon-socket')+1]
    if Path(endpoint).exists():
        with socket.socket(socket.AF_UNIX) as link:
            link.connect(endpoint)
            link.sendall(b'{"jsonrpc":"2.0","id":1,"method":"hello","params":{}}\\n')
            link.recv(65536)
while os.read(0,1)!=b'\\x03': pass
'''


class TerminalDaemonTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="carlos-terminal-daemon-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.previous = Terminal.daemon_binary
        self.addCleanup(setattr, Terminal, "daemon_binary", self.previous)
        self.daemon = self.program("daemon", DAEMON)
        self.client = self.program("client", CLIENT)

    def program(self, name, source):
        path = self.root / name
        path.write_text(textwrap.dedent(source))
        path.chmod(0o755)
        return path

    def launch(self, resume=None, extra_args=None):
        Terminal.daemon_binary = self.daemon
        terminal = Terminal(self.client, self.root, resume=resume, extra_args=extra_args)
        self.addCleanup(terminal.close)
        terminal.wait(lambda:(self.root/"client-args.json").exists(), "fixture client started")
        return terminal

    def test_detach_keeps_owned_daemon_alive_then_fixture_close_reaps_its_native_child(self):
        terminal = self.launch("native-original")
        terminal.wait(lambda:terminal.daemon_requests("hello"), "typed wire was recorded separately")
        self.assertEqual(terminal.requests("native-fixture-check"), [{"id":91, "method":"native-fixture-check"}])
        self.assertEqual(terminal.requests("hello"), [])
        self.assertEqual(terminal.daemon_requests("native-fixture-check"), [])
        daemon = terminal.owned_daemon
        child = int((self.root/"owned-native-pid").read_text())
        terminal.close_process()
        self.assertIsNone(daemon.process.poll(), "closing the client only detaches")
        os.kill(child, 0)
        terminal.close()
        self.assertEqual(daemon.process.returncode, 0)
        with self.assertRaises(ProcessLookupError):
            os.kill(child, 0)
        self.assertFalse(daemon.recorder.thread.is_alive())
        self.assertFalse(daemon.socket.exists())

    def test_new_resume_continue_and_picker_arguments_reach_client_unchanged(self):
        for intent, expected in ((None, []), ("native-original", ["resume","native-original"]),
                                 ("__continue__", ["continue"]), ("__picker__", ["resume"])):
            with self.subTest(intent=intent):
                marker = self.root/"client-args.json"
                marker.unlink(missing_ok=True)
                terminal = self.launch(intent)
                actual = json.loads(marker.read_text())
                self.assertEqual(actual[:2], ["--daemon-socket",str(terminal.owned_daemon.endpoint)])
                self.assertEqual(actual[2:], ["--backend","codex",*expected])
                terminal.close()
                self.assertTrue((self.root/"rpc.jsonl").exists(), "same-root native fixture files persist")

    def test_explicit_daemon_fixture_never_starts_a_nested_owned_daemon(self):
        Terminal.daemon_binary = self.root/"does-not-exist"
        terminal = Terminal(self.client, self.root, resume="logical-address",
                            extra_args=["--daemon-socket","provided.sock"])
        self.addCleanup(terminal.close)
        terminal.wait(lambda:(self.root/"client-args.json").exists(), "explicit attach client started")
        self.assertIsNone(terminal.owned_daemon)
        self.assertFalse((self.root/"owned-daemon-pid").exists())
        args = json.loads((self.root/"client-args.json").read_text())
        self.assertEqual(args.count("--daemon-socket"), 1)

    def test_client_exec_failure_still_closes_the_test_owned_daemon(self):
        Terminal.daemon_binary = self.daemon
        terminal = Terminal(self.root/"missing-client", self.root, resume=None)
        with self.assertRaisesRegex(AssertionError, "Carlos exited abnormally"):
            terminal.close()
        self.assertEqual(terminal.owned_daemon.process.returncode, 0)
        child = int((self.root/"owned-native-pid").read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(child, 0)

    def test_daemon_start_failure_keeps_diagnostics_and_removes_its_private_socket_directory(self):
        broken = self.program("broken-daemon", "#!/usr/bin/env python3\nimport sys\nfrom pathlib import Path\nPath(sys.argv[sys.argv.index('--state-dir')+1]).parent.joinpath('failed-socket').write_text(sys.argv[sys.argv.index('--socket')+1])\nprint('OWNED_START_FAILED',flush=True)\nsys.exit(7)\n")
        env = {**os.environ,"CARLOS_TEST_ROOT":str(self.root)}
        with self.assertRaisesRegex(AssertionError, "owned daemon exited 7"):
            OwnedDaemon(broken, self.root, env)
        self.assertIn("OWNED_START_FAILED", (self.root/"daemon.log").read_text())
        self.assertFalse(Path((self.root/"failed-socket").read_text()).parent.exists())

    def test_explicit_native_socket_is_passed_to_both_existing_consumers(self):
        terminal = self.launch(extra_args=["--codex-socket","native-attach.sock"])
        daemon_args = json.loads((self.root/"daemon-args.json").read_text())
        client_args = json.loads((self.root/"client-args.json").read_text())
        for arguments in (daemon_args, client_args):
            self.assertEqual(arguments[arguments.index("--codex-socket")+1], "native-attach.sock")


if __name__ == "__main__":
    unittest.main()
