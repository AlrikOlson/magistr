#!/usr/bin/env python3
"""Every current shared interface module and desktop harness has a maintained index."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
page=(ROOT/"docs/reference/cockpit-modules.md").read_text()
files=[]
for crate in ("magistr-cockpit","magistr-cockpit-runtime","magistr-desktop"):
    files.extend(p for p in (ROOT/"crates"/crate/"src").rglob("*.rs") if not p.name.endswith(".tests.rs"))
files.extend((ROOT/"crates/magistr-desktop/ui").glob("*.mjs"))
files.extend(p for p in (ROOT/"crates/magistr-desktop/tests").iterdir() if p.is_file())
import re
obsolete=[file for file in re.findall(r"^- `([^`]+)`",page,re.M) if not (ROOT/file).is_file()]
assert not obsolete,"Index names deleted sources: "+", ".join(obsolete)
missing=[str(p.relative_to(ROOT)) for p in files if "`"+str(p.relative_to(ROOT))+"`" not in page]
assert not missing,"Unindexed interface modules: "+", ".join(missing)
print("PASS: all",len(files),"current interface modules and harnesses are indexed")