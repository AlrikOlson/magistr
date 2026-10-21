#!/usr/bin/env python3
"""Exercise architecture refusals and nextest's real parser on temporary projects."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from source_scan import SourceTree

SCRIPTS = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


native = load("native-write-path-gate")
nextest = load("nextest-config-gate")


class OwnershipTests(unittest.TestCase):
    def test_every_ownership_refusal_and_valid_comments(self):
        cases = [
            ("crates/example/src/lib.rs", "ShipEngine::new();", "composes"),
            (native.NATIVE + "writer.rs", "ship_engine();", "engine lock"),
            (native.NATIVE + "native_engines.rs", "let x: CallToolResult;", "typed application"),
            ("crates/magistr-cli/src/mcp_tools/tool.rs", "CallToolRequestParams::new();", "own wire"),
            (native.WORKFLOW + "ship/application/use_case.rs", "use rmcp::Parameters;", "couples"),
            ("crates/example/src/lib.rs", "x.ship_admit_check();", "composition boundary"),
            (native.ADMISSIONS[0], "ProjectAuthority::admit();", "evidence capability"),
            ("crates/example/src/lib.rs", "WorkspaceService::open();", "hosted workspace"),
            ("crates/example/src/lib.rs", "writer_config(); ProjectAuthority::admit();", "admission sites"),
            ("crates/example/src/lib.rs", "OwnerClient::attach_workspace();", "attached surfaces"),
            ("crates/magistr-acp/src/lib.rs", "tokio::fs::write();", "ACP host"),
            ("crates/magistr-cli/src/scaffold/new.rs", "std::fs::write();", "administrative service"),
            (native.NATIVE + "handler_call.rs", "", "trampoline"),
        ]
        for name, source, expected in cases:
            with self.subTest(rule=expected), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                path = root / name
                path.parent.mkdir(parents=True)
                path.write_text(source)
                errors = native.violations(SourceTree(root), check_dependencies=False)
                self.assertIn(expected, "\n".join(errors))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "crates/example/src/lib.rs"
            path.parent.mkdir(parents=True)
            path.write_text("// ShipEngine::new(); WorkspaceService::open();\n")
            self.assertEqual(native.violations(SourceTree(root), False), [])

    def test_dependency_command_failures_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = SourceTree(directory)
            for output in ("", "magistr-workflow\nmagistr-cli\nother\n"):
                with patch.object(native.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, output, "")):
                    self.assertTrue(native.violations(tree))
            with patch.object(native.subprocess, "run", return_value=subprocess.CompletedProcess(
                    [], 0, "magistr-workflow\nmagistr-cli\nmagistr-engine\n", "")):
                self.assertEqual(native.violations(tree), [])


class NextestTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "Cargo.toml").write_text('[package]\nname="configuration-fixture"\nversion="0.1.0"\nedition="2021"\n[lib]\npath="lib.rs"\n')
        (self.root / "lib.rs").write_text("#[test] fn known() {}\n")
        subprocess.run(["cargo", "generate-lockfile"], cwd=self.root, check=True, capture_output=True)
        self.config = self.root / "nextest.toml"
        self.config.write_text("[profile.default]\nretries=0\n")

    def test_real_parser_requires_no_test_build_and_rejects_unknown_or_malformed_config(self):
        nextest.validate_syntax(self.root, self.config)
        self.assertFalse((self.root / "target").exists())
        for text in ("[profile.default]\nimaginary=1\n", "[profile.default\n",
                     '[profile.default]\ndefault-filter="test("\n'):
            with self.subTest(text=text):
                self.config.write_text(text)
                with self.assertRaises(RuntimeError):
                    nextest.validate_syntax(self.root, self.config)
                self.assertFalse((self.root / "target").exists())

    def test_missing_tools_are_failures(self):
        with patch.object(nextest.subprocess, "run", side_effect=FileNotFoundError("cargo")):
            with self.assertRaises(FileNotFoundError):
                nextest.validate_syntax(self.root, self.config)

    def test_profile_filters_need_real_tests_and_refuse_empty_matches(self):
        self.config.write_text('[profile.default]\ndefault-filter="test(known)"\n')
        with self.assertRaisesRegex(RuntimeError, "compiled nextest archive"):
            nextest.validate_filters(self.root, self.config, None)
        archive = self.root / "tests.tar.zst"
        subprocess.run(["cargo", "nextest", "archive", "--archive-file", str(archive)],
                       cwd=self.root, check=True, capture_output=True)
        self.assertEqual(nextest.validate_filters(self.root, self.config, archive), 1)
        self.config.write_text('[profile.default]\ndefault-filter="test(absent)"\n')
        with self.assertRaisesRegex(RuntimeError, "selects no test"):
            nextest.validate_filters(self.root, self.config, archive)


if __name__ == "__main__":
    unittest.main()
