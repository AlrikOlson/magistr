#!/usr/bin/env bash
# The production WebView frontend is the same source in debug and release.
# Check its batch budget and the actual native input/reply observation.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
node --test crates/magistr-desktop/tests/response.mjs
python3 - <<'PY'
import json
from pathlib import Path
receipt=json.loads(Path("docs/evidence/desktop-current/receipt.json").read_text())
latency=receipt["latency"]["native_event_to_accessible_reply_ms"]
assert 0 < latency < 2000,latency
print(f"PASS: native keyboard event to accessible reply {latency:.1f} ms (2 s bound)")
PY
