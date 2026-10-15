"""Evidence guard tests use synthetic bytes; they establish no visual quality."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
import sys
sys.dont_write_bytecode = True
from desktop_iteration_gate import validate_iteration


class IterationEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ui = self.root / "crates/magistr-desktop/ui"
        self.ui.mkdir(parents=True)
        (self.ui / "style.css").write_text("body{}")
        (self.ui.parent / "tauri.conf.json").write_text(json.dumps({"app": {"windows": [{"minWidth": 760, "minHeight": 550}]}}))
        self.directory = self.root / "docs/evidence/desktop-iterations/test"
        self.directory.mkdir(parents=True)
        self.review = {"scope": "Synthetic guard test", "limitations": "No real captures", "images": {}}
        source = "crates/magistr-desktop/ui/style.css"
        for theme in ("dark", "light"):
            for size, dimensions in (("desktop", (1280, 820)), ("compact", (760, 550))):
                name = f"{theme}-{size}.png"
                data = b"\x89PNG\r\n\x1a\n" + bytes(8) + struct.pack(">II", *dimensions)
                digest = hashlib.sha256(data).hexdigest()
                (self.directory / name).write_bytes(data)
                record = {"renderer": "headless WebKit", "captured_at": "test", "renderer_version": "test", "appearance": theme,
                          "source_hashes": {source: hashlib.sha256((self.root / source).read_bytes()).hexdigest()},
                          "viewport": dict(zip(("width", "height"), dimensions)), "image": {"sha256": digest}}
                (self.directory / Path(name).with_suffix(".json")).write_text(json.dumps(record))
                self.review["images"][name] = {"sha256": digest, "verdict": "pass", "task": "Test guard", "observations": "Synthetic"}
        self.save()

    def save(self):
        (self.directory / "review.json").write_text(json.dumps(self.review))

    def validate(self):
        with contextlib.redirect_stdout(io.StringIO()):
            validate_iteration(self.root, "test")

    def test_accepts_complete_current_evidence(self):
        self.validate()

    def test_rejects_stale_source(self):
        (self.ui / "style.css").write_text("body{color:red}")
        with self.assertRaisesRegex(AssertionError, "stale"):
            self.validate()

    def test_rejects_unreviewed_image(self):
        del self.review["images"]["light-desktop.png"]
        self.save()
        with self.assertRaisesRegex(AssertionError, "every image"):
            self.validate()

    def test_rejects_failed_review(self):
        self.review["images"]["dark-desktop.png"]["verdict"] = "fail"
        self.save()
        with self.assertRaisesRegex(AssertionError, "defect"):
            self.validate()

    def test_rejects_incomplete_matrix(self):
        for name in list(self.review["images"]):
            if name.startswith("light"):
                (self.directory / name).unlink()
                del self.review["images"][name]
        self.save()
        with self.assertRaisesRegex(AssertionError, "both appearances"):
            self.validate()

    def test_rejects_changed_image(self):
        (self.directory / "dark-desktop.png").write_bytes(b"bad")
        with self.assertRaises(AssertionError):
            self.validate()


if __name__ == "__main__":
    unittest.main()
