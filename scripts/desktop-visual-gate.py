#!/usr/bin/env python3
"""Check the current native capture review, palette, icon and source freshness."""
import hashlib
import json
from pathlib import Path
import re
import struct
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
        for foreground in ("text","muted","accent","danger"):
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
for file,expected in receipt["source_hashes"].items():
    assert sha(ROOT/file)==expected,"Native evidence is stale for "+file
review=json.loads((EVIDENCE/"review.json").read_text())
for name in ("dark","light","narrow","chooser-dark","chooser-light","setup-review","setup-partial","provider-unavailable","supervision","agent-choice","conversation-controls","resume-unavailable","saved-summary","resume-refused","rich-dark","rich-light","rich-narrow","attachments-dark","attachments-light","attachments-narrow","attachments-refused"):
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
print("PASS: current native screenshots, reviewed source, accessible controls, contrast, icon and reduced motion")
