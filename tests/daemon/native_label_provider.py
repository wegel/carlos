#!/usr/bin/env python3
"""Record the label child identity, then run the existing deterministic label provider."""

import json
import os
from pathlib import Path
import runpy
import sys

root = Path(sys.argv[1])
(root / "summary-child.json").write_text(json.dumps({"pid":os.getpid(), "parent":os.getppid()}))
runpy.run_path(str(Path(__file__).resolve().parents[1] / "ui/fake_summary.py"), run_name="__main__")
