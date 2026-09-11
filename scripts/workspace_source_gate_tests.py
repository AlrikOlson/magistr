#!/usr/bin/env python3
"""Regression tests for source integrity, Cargo composition and structure ratchets."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


source = load("workspace_source", "workspace-source-gate.py")
structure = load("workspace_structure", "workspace-structure-gate.py")


class ReferenceIntegrity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.subsystem = self.root / source.SUBSYSTEM
        self.reference = self.root / source.REFERENCE
        self.reference.mkdir(parents=True)
        entries = []
        for index in range(525):
            content = f"source file {index}\n".encode()
            relative = f"input-{index}.rs"
            included = index < 428
            entries.append({"path": relative, "mode": "100644", "git_blob": source.git_object("blob", content),
                            "sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content),
                            "included": included, "reason": "retained source" if included else "excluded host configuration"})
            if included:
                (self.reference / relative).write_bytes(content)
        self.manifest = {"version": 1, "upstream_revision": source.REVISION,
                         "upstream_tree": source.tree_digest(entries), "files": entries}
        self.changes = {"version": 1, "files": []}
        self.write_manifests()
        patched = patch.object(source, "TREE", self.manifest["upstream_tree"])
        patched.start()
        self.addCleanup(patched.stop)

    def write_manifests(self):
        (self.subsystem / "UPSTREAM_MANIFEST.json").write_text(json.dumps(self.manifest))
        (self.subsystem / "LOCAL_CHANGES.json").write_text(json.dumps(self.changes))

    def errors(self):
        return source.reference_errors(self.root)

    def test_exact_source_accepts_then_changed_bytes_fail(self):
        self.assertEqual(self.errors(), [])
        (self.reference / "input-0.rs").write_bytes(b"changed\n")
        self.assertTrue(any("bytes/mode differ" in error for error in self.errors()))

    def test_mode_and_missing_source_fail(self):
        (self.reference / "input-0.rs").chmod(0o755)
        (self.reference / "input-1.rs").unlink()
        errors = self.errors()
        self.assertTrue(any("bytes/mode differ" in error for error in errors))
        self.assertTrue(any("missing, non-regular" in error for error in errors))

    def test_excluded_record_cannot_be_removed_or_materialized(self):
        (self.reference / "input-524.rs").write_text("excluded content")
        self.assertTrue(any("excluded upstream path" in error for error in self.errors()))
        self.manifest["files"].pop()
        self.write_manifests()
        errors = self.errors()
        self.assertTrue(any("audited Git tree" in error for error in errors))
        self.assertTrue(any("525 audited" in error for error in errors))

    def test_reviewed_modification_and_addition_are_pinned(self):
        changed = self.reference / "input-0.rs"
        changed.write_text("reviewed repair\n")
        added = self.reference / "build.sh"
        added.write_text("#!/bin/sh\nexit 0\n")
        added.chmod(0o755)
        self.changes["files"] = [
            {"path": "input-0.rs", "sha256": hashlib.sha256(changed.read_bytes()).hexdigest(),
             "mode": "100644", "reason": "repair"},
            {"path": "build.sh", "sha256": hashlib.sha256(added.read_bytes()).hexdigest(),
             "mode": "100755", "reason": "rebuild retained guest", "added": True},
        ]
        self.write_manifests()
        self.assertEqual(self.errors(), [])
        added.write_text("unexpected rebuild script\n")
        self.assertTrue(any("local addition bytes/mode" in error for error in self.errors()))

    def test_addition_path_escape_and_symlink_are_rejected(self):
        self.changes["files"] = [{"path": "../escape", "sha256": "0" * 64,
                                  "mode": "100644", "reason": "invalid", "added": True}]
        self.write_manifests()
        self.assertTrue(any("invalid, duplicate" in error for error in self.errors()))
        existing = self.reference / "input-0.rs"
        existing.unlink()
        existing.symlink_to(self.reference / "input-1.rs")
        self.assertTrue(any("missing, non-regular" in error for error in self.errors()))


class NativeComposition(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        native = self.root / source.NATIVE
        native.mkdir(parents=True)
        (native / "Cargo.toml").write_text('[package]\nname = "magistr-workspace"\n')
        packages = [
            {"id": "native", "name": "magistr-workspace", "version": "0.1.0", "source": None,
             "manifest_path": str(native / "Cargo.toml"), "license": "Apache-2.0",
             "targets": [{"kind": ["rlib"], "crate_types": ["rlib"]}]},
            {"id": "w36", "name": "wasmtime", "version": "36.0.0", "source": "registry"},
            {"id": "w48", "name": "wasmtime", "version": "48.0.0", "source": "registry"},
        ]
        self.metadata = {"workspace_root": str(self.root), "workspace_members": ["native"], "packages": packages,
                         "resolve": {"nodes": [{"id": "native", "deps": [self.edge("w36"), self.edge("w48")]},
                                               {"id": "w36", "deps": []}, {"id": "w48", "deps": []}]}}

    @staticmethod
    def edge(package):
        return {"pkg": package, "dep_kinds": [{"kind": None}]}

    def errors(self):
        result = types.SimpleNamespace(returncode=0, stdout=json.dumps(self.metadata), stderr="")
        with patch.object(source.subprocess, "run", return_value=result):
            return source.cargo_errors(self.root)

    def test_local_rlib_with_both_runtime_majors_is_accepted(self):
        self.assertEqual(self.errors(), [])

    def test_transitive_napi_dependency_is_rejected(self):
        self.metadata["packages"].append({"id": "addon", "name": "napi", "version": "3.0.0", "source": "registry"})
        self.metadata["resolve"]["nodes"][1]["deps"].append(self.edge("addon"))
        self.metadata["resolve"]["nodes"].append({"id": "addon", "deps": []})
        self.assertTrue(any("binding package napi" in error for error in self.errors()))

    def test_external_manifest_addon_target_and_lost_language_host_fail(self):
        native = self.metadata["packages"][0]
        native["manifest_path"] = str(self.root.parent / "external/Cargo.toml")
        native["targets"][0] = {"kind": ["cdylib"], "crate_types": ["cdylib"]}
        self.metadata["resolve"]["nodes"][0]["deps"] = [self.edge("w48")]
        errors = self.errors()
        self.assertTrue(any("repository-local manifest" in error for error in errors))
        self.assertTrue(any("ordinary Rust library" in error for error in errors))
        self.assertTrue(any("Wasmtime 36" in error for error in errors))


class StructureRatchet(unittest.TestCase):
    def test_imported_violations_are_bounded_and_new_files_obey_normal_limits(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            native = root / "crates/magistr-workspace/src/engine.rs"
            native.parent.mkdir(parents=True)
            native.write_text("pub struct A;\npub struct B;\n")
            reference = root / "subsystems/workspace/reference/core.rs"
            reference.parent.mkdir(parents=True)
            reference.write_text("fn inherited() {\n" + "    do_work();\n" * 155 + "}\n")
            baseline = root / structure.BASELINE
            baseline.write_text(json.dumps({"version": 1, "upstream_revision": structure.REVISION,
                                            "files": structure.measurements(root)}))
            self.assertEqual(structure.check(root), [])
            native.write_text(native.read_text() + "pub struct C;\n")
            reference.write_text(reference.read_text().replace("}\n", "    do_work();\n}\n"))
            fresh = native.with_name("fresh.rs")
            fresh.write_text("pub struct D;\npub struct E;\n")
            errors = structure.check(root)
            self.assertTrue(any("engine.rs: public_items grew" in error for error in errors))
            self.assertTrue(any("core.rs: long_function_excess_lines grew" in error for error in errors))
            self.assertTrue(any("fresh.rs: public_items grew" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
