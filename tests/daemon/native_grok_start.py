"""Hold session/new after initialize so stopping cannot save Grok's temporary starting label."""

import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ui"))
import fake_grok as peer

for line in sys.stdin:
    request = json.loads(line)
    peer.record("client", request)
    if request["method"] == "session/new":
        (peer.ROOT / "native-open-held").write_text("native address not acknowledged")
        for remaining in sys.stdin:
            pass
        break
    peer.handle(request)
