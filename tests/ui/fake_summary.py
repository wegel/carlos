#!/usr/bin/env python3
"""Return fixed labels after a test-controlled gate, without executing input."""
import json
import sys
import time
from pathlib import Path

root = Path(sys.argv[1])
value = json.load(sys.stdin)
with (root / 'summary-requests.jsonl').open('a') as stream:
    stream.write(json.dumps(value) + '\n')
deadline = time.monotonic() + 15
while not (root / 'release-summary').exists() and time.monotonic() < deadline:
    time.sleep(.02)
if 'final268-screen' in value['command']:
    label = 'Read saved playback positions'
elif 'tmdb.' in value['command']:
    label = 'Read remote simulator language preferences'
elif 'print(1)' in value['command']:
    label = 'Print the number one'
else:
    label = 'Rewrite the result report' if 'python3' in value['command'] else 'Stage and commit source files'
override = root / 'summary-label.txt'
print(override.read_text().strip() if override.exists() else label, flush=True)
