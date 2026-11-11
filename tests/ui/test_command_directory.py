"""Use real UI keys and the external label process without running tool commands."""
import json
import shutil
import sys
import tempfile
from pathlib import Path

from directory_command_fixture import BODY, directory_items


class CommandDirectory:
    def directory_launch(self, root, backend, resume=None):
        settings = root / 'config/carlos/tool-summaries.json'
        settings.parent.mkdir(parents=True, exist_ok=True)
        settings.write_text(json.dumps({'provider': 'command', 'executable': sys.executable,
            'arguments': [str(Path(__file__).with_name('fake_summary.py').resolve()), str(root)],
            'timeout_seconds': 20}))
        return self.launch(mode='summary-cd', backend=backend, resume=resume, directory=root, width=120)

    def directory_requests(self, root):
        path = root / 'summary-requests.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def directory_toggle(self, terminal, enabled):
        terminal.send(b'\x1b[109;5u')
        terminal.contains('AI labels')
        terminal.send(b'\x1b[Z' * 4)
        terminal.send(b'\x1b[C' if enabled else b'\x1b[D')
        terminal.send(b'\r')
        terminal.wait(lambda: 'Settings' not in terminal.text, 'AI label setting applied')

    def directory_workflow(self, backend):
        # The cat|head fixture uses a long literal cd prefix with the same effective cwd.
        root = Path(tempfile.mkdtemp(prefix=f'carlos-cd-{backend}-')) / ('long-quoted-project-雪 ' * 2)
        root.mkdir()
        (self.summary_root() / 'fixture-root.txt').write_text(str(root))
        terminal = self.directory_launch(root, backend)
        terminal.resize(120, 48)
        terminal.wait(lambda: terminal.input_is('') and not any(word in terminal.text
            for word in ('Connecting', 'Reconnecting')), 'ready composer')
        if backend == 'claude':
            self.claude_ready(terminal)
        terminal.paste('inspect directory tools')
        terminal.send(b'\r')
        terminal.contains('Answer: inspect directory tools')
        terminal.contains('directory output 3')
        terminal.contains('DIRECTORY_BODY_MARKER')
        self.assertNotIn('cd -- ', terminal.text)
        self.assertNotIn('long-quoted-project', terminal.text)
        self.tools_command(terminal, '/tools compact', 'compact')
        terminal.contains('(in scripts)')
        terminal.wait(lambda: len(self.directory_requests(root)) >= 1, 'first label worker input')
        (root / 'release-summary').touch()
        terminal.contains('Rewrite the result report (in scripts)')
        self.directory_toggle(terminal, False)
        terminal.contains('(in scripts)')
        self.assertNotIn('Rewrite the result report', terminal.text)
        self.directory_toggle(terminal, True)
        terminal.contains('Rewrite the result report (in scripts)')
        for width in (54, 36, 120):
            terminal.resize(width, 40)
            terminal.wait(lambda: '(in scripts)' in ' '.join(terminal.text.split()), 'complete wrapped directory')
            self.assertNotIn('long-quoted-project', terminal.text)
            terminal.snapshot(f'directory-label-{backend}-{width}')
        requests = self.directory_requests(root)
        session = self.directory_wire(terminal, root, backend, requests)
        self.directory_resume(terminal, root, backend, session, len(requests))

    def directory_resume(self, terminal, root, backend, session, count):
        terminal.close()
        resumed = self.directory_launch(root, backend, session)
        resumed.resize(120, 48)
        resumed.contains('Answer: inspect directory tools')
        if backend == 'claude':
            self.claude_ready(resumed)
        resumed.contains('Rewrite the result report (in scripts)')
        self.assertEqual(len(self.directory_requests(root)), count)
        self.directory_toggle(resumed, False)
        resumed.contains('(in scripts)')
        self.assertNotIn('Rewrite the result report', resumed.text)
        self.directory_toggle(resumed, True)
        resumed.contains('Rewrite the result report (in scripts)')
        self.assertEqual(len(self.directory_requests(root)), count)
        resumed.close()
        if backend == 'claude':
            self.directory_cold_history(root, backend, session)

    def directory_cold_history(self, root, backend, session):
        count = len(self.directory_requests(root))
        # Keep exactly the same cwd/read authority, remove only Carlos's label cache.
        shutil.move(root / 'state', root / 'saved-label-state')
        cold = self.directory_launch(root, backend, session)
        cold.resize(120, 48)
        cold.contains('Answer: inspect directory tools')
        self.claude_ready(cold)
        cold.contains('(in scripts)')
        self.assertNotIn('Rewrite the result report', cold.text)
        self.assertEqual(len(self.directory_requests(root)), count)
        self.directory_toggle(cold, False)
        self.directory_toggle(cold, True)
        cold.contains('Answer: inspect directory tools')
        self.assertEqual(len(self.directory_requests(root)), count)
        cold.snapshot('uncached-same-cwd-history')

    def directory_wire(self, terminal, root, backend, requests):
        self.assertEqual({row['command'] for row in requests}, {BODY, 'python3 process.py'})
        self.assertTrue(all('read_root' not in row for row in requests))
        named = [row for row in requests if row['command'] == 'python3 process.py']
        self.assertTrue(all(row['cwd'] == str(root / 'scripts') and row['directory'] == 'scripts' for row in named))
        self.assertTrue(all(row['cwd'] == str(root) and row.get('directory') is None
                            for row in requests if row['command'] == BODY))
        expected = [item['command'] for item in directory_items(root)]
        self.assertGreaterEqual(len(expected[3]), 200)
        if backend == 'codex':
            state = json.loads((root / 'server-state.json').read_text())
            raw = state['thread-new']['turns'][0]['items']
            self.assertEqual([item['command'] for item in raw if item['type'] == 'commandExecution'], expected)
            session = 'thread-new'
        else:
            native = [m for m in terminal.messages() if m.get('type') == 'user']
            self.assertEqual(len(native), 1)
            session = next(m['session_id'] for m in terminal.messages('server') if m.get('type') == 'result')
            calls = [m for m in terminal.messages('server') if m.get('type') == 'assistant']
            self.assertEqual([m['message']['content'][0]['input']['command'] for m in calls], expected)
        return session

    def test_codex_cd_prefix_display_provider_input_toggle_and_resume(self):
        self.directory_workflow('codex')

    def test_claude_cd_prefix_display_provider_input_toggle_and_uncached_history(self):
        self.directory_workflow('claude')
