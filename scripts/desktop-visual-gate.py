#!/usr/bin/env python3
"""Check current UI layout evidence and the integrity of recorded native evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--native-current",action="store_true",help="also require every recorded native journey to match all current integration sources")
args=parser.parse_args()
ROOT=Path(__file__).resolve().parents[1]
UI=ROOT/"crates/magistr-desktop/ui"
EVIDENCE=ROOT/"docs/evidence/desktop-current"
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def ratio(a,b):
    def luminance(value):
        channels=[int(value[i:i+2],16)/255 for i in (1,3,5)]
        linear=[v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4 for v in channels]
        return sum(v*w for v,w in zip(linear,(.2126,.7152,.0722)))
    a,b=sorted((luminance(a),luminance(b)))
    return (b+.05)/(a+.05)
css=(UI/"style.css").read_text()
palettes=re.findall(r"--base:(#[a-f0-9]+);.*?--focus:(#[a-f0-9]+)",css)
assert len(palettes)>=2,"Both appearances need palettes"
blocks=re.findall(r"\{[^{}]*--base:[^{}]+\}",css)
for block in blocks:
    colors=dict(re.findall(r"--([a-z-]+):(#[a-f0-9]+)",block))
    # CSS #fff is expanded to the same sRGB value.
    colors={key:("#"+value[1]*2+value[2]*2+value[3]*2 if len(value)==4 else value) for key,value in colors.items()}
    for background in ("base","panel","raised"):
        for foreground in ("text","muted","accent","danger","success","warning"):
            contrast=ratio(colors[foreground],colors[background])
            assert contrast>=4.5,(foreground,background,round(contrast,2))
        for foreground in ("focus","line"):
            contrast=ratio(colors[foreground],colors[background])
            assert contrast>=3,(foreground,background,round(contrast,2))
    assert ratio(colors["button-text"],colors["button"])>=4.5
assert "animation:" not in css and "transition:" not in css
assert "prefers-reduced-motion" in css
assert sha(UI/"icon.png")=="392002a92fdd8dc81923944cd5abd170d0867fe8eb3701d71d4df4aea083dd10","Selected brand icon changed; review its provenance"
receipt=json.loads((EVIDENCE/"receipt.json").read_text())
assert receipt["checks"] and all(receipt["checks"].values()),"Native journey has an incomplete check"
drafts=json.loads((EVIDENCE/"draft-persistence-receipt.json").read_text())
assert drafts["checks"] and all(drafts["checks"].values()),"Draft reopen journey has an incomplete check"
for file,expected in drafts["evidence_hashes"].items():
    assert sha(EVIDENCE/file)==expected,"Draft recovery evidence changed: "+file
work=json.loads((EVIDENCE/"work-receipt.json").read_text())
assert work["checks"] and all(work["checks"].values()),"Work journey has an incomplete check"
review=json.loads((EVIDENCE/"review.json").read_text())
assert review["work"]==sha(EVIDENCE/"work-receipt.json")
for name,expected in work["captures"].items():
    assert sha(EVIDENCE/name)==expected==review["images"][name],"Work capture changed after review: "+name
assert review["draft_persistence"]==sha(EVIDENCE/"draft-persistence-receipt.json")
for name in ("dark","light","narrow","chooser-dark","chooser-light","setup-review","setup-partial","provider-unavailable","supervision","agent-choice","conversation-controls","resume-unavailable","saved-summary","resume-refused","rich-dark","rich-light","rich-narrow","attachments-dark","attachments-light","attachments-narrow","attachments-refused","drafts-saved-message","drafts-restored-dark","drafts-restored-light","drafts-restored-narrow","drafts-save-failure"):
    path=EVIDENCE/(name+".png")
    assert path.read_bytes()[:8]==b"\x89PNG\r\n\x1a\n"
    width,height=struct.unpack(">II",path.read_bytes()[16:24])
    assert width>=1500 and height>=1100,(name,width,height)
    assert review["images"][path.name]==sha(path),"Capture was changed after review"
assert review["accessibility"]==sha(EVIDENCE/"accessibility.json")
assert review["chooser_accessibility"]==sha(EVIDENCE/"chooser-accessibility.json")
assert review["rich_accessibility"]==sha(EVIDENCE/"rich-accessibility.json")
assert review["attachments_accessibility"]==receipt["attachments"]["native_accessibility_sha256"]==sha(EVIDENCE/"attachments-accessibility.json")
assert review["attachment_observation"]==receipt["attachment_observation_sha256"]==sha(EVIDENCE/"attachment-observation.json")
assert review["attachment_prompts"]==receipt["attachment_prompts_sha256"]==sha(EVIDENCE/"attachment-prompts.json")
assert review["rich_observation"]==receipt["rich_observation_sha256"]==sha(EVIDENCE/"rich-observation.json")
layout=ROOT/"docs/evidence/desktop-layout"
current=json.loads((layout/"receipt.json").read_text())
reviewed=json.loads((layout/"review.json").read_text())
assert current["renderer"]=="headless WebKit"
assert current["checks"] and all(current["checks"].values()),"Background layout check is incomplete"
assert reviewed["receipt"]==sha(layout/"receipt.json"),"Review the current background layout receipt"
expected_sources={str(file.relative_to(ROOT)) for file in UI.iterdir() if file.is_file()}
assert expected_sources.issubset(current["source_hashes"]),"Background layout omits a UI source"
for file,expected in current["source_hashes"].items():
    assert sha(ROOT/file)==expected,"Background layout is stale for "+file+"; run the affected headless scenario and review its captures"
window=json.loads((ROOT/"crates/magistr-desktop/tauri.conf.json").read_text())["app"]["windows"][0]
minimum=(window["minWidth"],window["minHeight"])
for name,expected in current["images"].items():
    data=(layout/name).read_bytes()
    assert data[:8]==b"\x89PNG\r\n\x1a\n"
    width,height=struct.unpack(">II",data[16:24])
    assert width>=minimum[0] and height>=minimum[1],(name,width,height)
    assert current["viewports"][name]=={"width":width,"height":height},"Capture dimensions do not match the recorded viewport: "+name
    assert sha(layout/name)==expected==reviewed["images"][name],"Background capture changed after review: "+name
for name in ("minimum-workspace-dark.png","minimum-workspace-light.png","minimum-projects-dark.png","minimum-projects-light.png","minimum-settings-light.png"):
    assert current["viewports"][name]=={"width":minimum[0],"height":minimum[1]},"Capture the actual native minimum: "+name
print("PASS: current headless WebKit layout, native minimum size, reviewed captures, contrast, icon and reduced motion")
for name,record in (("window",receipt),("draft recovery",drafts),("work",work)):
    changed=[file for file,expected in record["source_hashes"].items() if not (ROOT/file).is_file() or sha(ROOT/file)!=expected]
    if args.native_current:
        assert not changed,"Native "+name+" evidence is stale for "+", ".join(changed)
    elif changed:
        print("NOTE: native "+name+" evidence is retained at its recorded source version; current OS-input behavior was not re-run.")
print("PASS: recorded native evidence and review integrity")
