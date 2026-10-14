#!/usr/bin/env bash
# Publish the reviewed current native desktop images to local website assets.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 scripts/desktop-visual-gate.py
python3 - <<'PY'
import datetime, hashlib, json, pathlib, shutil, struct
root=pathlib.Path.cwd()
evidence=root/"docs/evidence/desktop-current"
out=root/"apps/website/public/screenshots"
out.mkdir(parents=True,exist_ok=True)
images=[]
for theme in ("dark","light"):
    name=f"desktop-{theme}.png"
    shutil.copyfile(evidence/f"{theme}.png",out/name)
    data=(out/name).read_bytes()
    width,height=struct.unpack(">II",data[16:24])
    images.append({"file":name,"screen":"conversation","sha256":hashlib.sha256(data).hexdigest(),"width":width,"height":height})
manifest={"renderer":"magistr-desktop","surface":"Desktop","captured_on":datetime.date.today().isoformat(),"state":"Native desktop journey","annotations":False,"images":images}
(out/"captures.json").write_text(json.dumps(manifest,indent=2)+"\n")
print("PASS: local website assets contain the reviewed native desktop")
PY
