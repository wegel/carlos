#!/usr/bin/env python3
"""Emit captured native stderr without an SDK, then close the failed helper."""

import json
from pathlib import Path
import sys

fixture = Path(__file__).resolve().parents[2] / "src/app/fixtures/ansi_controller_notices.json"
for notice in json.loads(fixture.read_text()):
    sys.stderr.write(notice.removeprefix("Claude controller: ") + "\n")
sys.stderr.write("ANSI notice END\n")
sys.stderr.flush()
