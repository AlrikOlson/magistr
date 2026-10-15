"""Validate a focused GUI iteration; image inspection remains a human/agent review."""
import hashlib
import json
from pathlib import Path
import struct


def validate_iteration(root, name):
    archive = (root / "docs/evidence/desktop-iterations").resolve()
    directory = (archive / name).resolve()
    assert directory.is_relative_to(archive), "Iteration must stay in the canonical archive"
    review = json.loads((directory / "review.json").read_text())
    assert review["scope"] and review["limitations"], "Describe the review scope and limitations"
    entries = review["images"]
    assert entries, "Review every capture"
    assert set(entries) == {path.name for path in directory.glob("*.png")}, "Review every image"
    matrix = set()
    source_paths = {str(path.relative_to(root)) for path in (root / "crates/magistr-desktop/ui").iterdir() if path.is_file()}
    minimum = json.loads((root / "crates/magistr-desktop/tauri.conf.json").read_text())["app"]["windows"][0]
    for filename, entry in entries.items():
        assert Path(filename).name == filename, "Use direct iteration image names"
        assert entry["verdict"] == "pass", "Visual defect remains open: " + filename
        assert entry["task"] and entry["observations"], "Record the task and concrete observations"
        record = json.loads((directory / Path(filename).with_suffix(".json")).read_text())
        assert record["renderer"] == "headless WebKit"
        assert record["captured_at"] and record["renderer_version"]
        assert source_paths.issubset(record["source_hashes"]), "Capture omitted interface sources"
        for path, expected in record["source_hashes"].items():
            assert hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, "Focused capture is stale: " + path
        data = (directory / filename).read_bytes()
        assert data[:8] == b"\x89PNG\r\n\x1a\n"
        assert hashlib.sha256(data).hexdigest() == record["image"]["sha256"] == entry["sha256"], "Image changed after review"
        width, height = struct.unpack(">II", data[16:24])
        assert record["viewport"] == {"width": width, "height": height}
        assert width >= minimum["minWidth"] and height >= minimum["minHeight"]
        size = "compact" if (width, height) == (minimum["minWidth"], minimum["minHeight"]) else "desktop"
        matrix.add((record["appearance"], size))
    assert {("dark", "compact"), ("light", "compact"), ("dark", "desktop"), ("light", "desktop")}.issubset(matrix), "Review both appearances at desktop and native minimum"
    print(f"PASS: {len(entries)} inspected focused captures in {name}; current sources, dimensions and image integrity")
