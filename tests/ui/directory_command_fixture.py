"""Commands are native fixture data, never shell instructions run by the peer."""
import shlex


BODY = "python3 - <<'PY'\nfrom pathlib import Path\nprint('DIRECTORY_BODY_MARKER', Path('report.txt').name)\n# literal cd /inside/string&& stays unchanged\nPY\n"


def directory_items(root):
    scripts = root / 'scripts'
    scripts.mkdir(exist_ok=True)
    (scripts / 'process.py').write_text("raise RuntimeError('fixture script must never execute')\n")
    commands = [f'cd -- {shlex.quote(str(root))}&&{BODY}',
                f'cd {shlex.quote(str(scripts))} && python3 process.py',
                f'cd {shlex.quote(str(root))} && rtk read file.rs',
                f'cd {shlex.quote(str(root) + "/." * 100)} && cat file | head -150' ]
    return [{'type': 'commandExecution', 'id': f'directory-{index}',
             'command': command, 'cwd': str(root),
             'aggregatedOutput': f'directory output {index}', 'exitCode': 0}
            for index, command in enumerate(commands)]


def claude_tools(peer, prompt):
    import uuid
    for item in directory_items(peer.ROOT):
        message = {'id': str(uuid.uuid4()), 'model': peer.model, 'role': 'assistant',
                   'content': [{'type': 'tool_use', 'id': item['id'], 'name': 'Bash',
                                'input': {'command': item['command']}}]}
        row = {'type': 'assistant', 'uuid': str(uuid.uuid4()), 'message': message}
        peer.sessions.append(row)
        peer.send({**row, 'session_id': peer.SESSION, 'user_message_uuid': prompt})
        message = {'role': 'user', 'content': [{'type': 'tool_result',
                   'tool_use_id': item['id'], 'is_error': False, 'content': item['aggregatedOutput']}]}
        row = {'type': 'user', 'uuid': str(uuid.uuid4()), 'message': message,
               'toolUseResult': {'stdout': item['aggregatedOutput'], 'stderr': '', 'interrupted': False}}
        peer.sessions.append(row)
        peer.send({**row, 'session_id': peer.SESSION})
