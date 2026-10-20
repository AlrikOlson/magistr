#!/usr/bin/env python3
"""Run installed-binary onboarding without project builds or agent inference."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from onboarding_terminal import interview

BINARY = Path(os.environ.get("MAGISTR_TEST_BINARY", "target/debug/magistr")).resolve()

class Onboarding(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="magistr onboarding ")
        self.base = Path(self.temp.name)
        self.root = self.base / "project with spaces λ"
        self.root.mkdir()
        (self.root / "Cargo.toml").write_text('[package]\nname="onboarding-fixture"\nversion="0.1.0"\nedition="2024"\n')
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("pub fn value() -> u32 { 42 }\n")
        self.env = {k:v for k,v in os.environ.items() if not k.startswith(("MAGISTR_", "MAGISTR_WORKSPACE_", "MAGISTR_WORKFLOW_"))}
        self.env.update(MAGISTR_WORKFLOW_DATA_DIR=str(self.base / "state"),
            XDG_CONFIG_HOME=str(self.base / "config"), XDG_DATA_HOME=str(self.base / "data"))

    def tearDown(self):
        self.temp.cleanup()

    def run_cli(self, *args, ok=True, env=None):
        result = subprocess.run([str(BINARY), *args], cwd=self.root, env=env or self.env,
            input="", text=True, capture_output=True, timeout=60)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def test_codex_only_repeated_setup_and_read_only_preview(self):
        self.run_cli("init", "--agent", "codex", "--dry-run")
        self.assertFalse((self.root / ".magistr").exists())
        result = self.run_cli("init", "--agent", "codex", "--yes")
        self.assertIn("context check passed", result.stderr)
        self.assertTrue((self.root / ".codex/config.toml").exists())
        self.assertFalse((self.root / ".claude").exists())
        self.assertFalse((self.root / ".mcp.json").exists())
        self.assertFalse((self.root / "target").exists(), "onboarding must not build the project")
        before = (self.root / ".magistr/project.json").read_bytes()
        self.run_cli("init", "--yes")
        self.assertEqual((self.root / ".magistr/project.json").read_bytes(), before)
        self.assertFalse((self.root / ".claude").exists())

    def test_multiple_agents_preserve_custom_configuration_and_external_connections(self):
        (self.root / ".codex").mkdir()
        config = self.root / ".codex/config.toml"
        config.write_text('# my settings\nmodel="chosen"\n[mcp_servers.external]\ncommand="external"\n')
        external = {"command":"external", "args":["space in argument"], "env":{"CUSTOM":"keep"}}
        (self.root / ".mcp.json").write_text(json.dumps({"mcpServers":{"external":external}}))
        self.run_cli("init", "--agent", "codex,claude", "--yes")
        mcp = json.loads((self.root / ".mcp.json").read_text())["mcpServers"]
        self.assertEqual(set(mcp), {"magistr", "external"})
        self.assertEqual(mcp["magistr"]["args"], ["mcp", "--dir", "."])
        self.assertEqual(mcp["external"], external)
        self.assertIn('model="chosen"', config.read_text())
        settings = self.root / ".magistr/project.json"
        configured = json.loads(settings.read_text())
        configured["quality_gate"]["test"] = "custom-test-command --focused"
        settings.write_text(json.dumps(configured))
        self.run_cli("init", "--yes")
        self.assertIn("custom-test-command --focused", (self.root / "CLAUDE.md").read_text())
        project = settings.read_bytes()
        custom = self.root / ".claude/rules/tools.md"
        custom.write_text("custom instructions\n")
        missing = self.root / ".claude/skills/craft/references/runtime.md"
        missing.unlink()
        self.run_cli("repair", "--dry-run")
        self.assertFalse(missing.exists())
        self.run_cli("repair", "--yes")
        self.assertTrue(missing.exists())
        self.assertEqual(custom.read_text(), "custom instructions\n")
        self.run_cli("remove", "--dry-run")
        self.assertTrue(missing.exists())
        self.run_cli("remove", "--yes")
        self.assertEqual((self.root / ".magistr/project.json").read_bytes(), project)
        self.assertEqual(custom.read_text(), "custom instructions\n")
        self.assertEqual(json.loads((self.root / ".mcp.json").read_text())["mcpServers"], {"external":external})
        self.assertIn("external", config.read_text())
        self.run_cli("remove", "--yes")

    def test_unavailable_agent_and_portable_setup(self):
        env = dict(self.env, PATH="/usr/bin:/bin")
        result = self.run_cli("init", "--agent", "claude,portable", "--yes", env=env)
        self.assertIn("claude is not installed", result.stderr)
        self.assertTrue((self.root / ".magistr/instructions.md").exists())
        self.assertFalse((self.root / ".codex").exists())

    def test_malformed_custom_settings_fail_before_any_setup(self):
        (self.root / ".mcp.json").write_text("{broken")
        self.run_cli("init", "--agent", "claude", "--yes", ok=False)
        self.assertEqual((self.root / ".mcp.json").read_text(), "{broken")
        self.assertFalse((self.root / ".magistr/project.json").exists())

    def test_custom_launcher_is_preserved_and_actually_checked(self):
        self.run_cli("init", "--agent", "claude", "--yes")
        config = self.root / ".mcp.json"
        document = json.loads(config.read_text())
        custom = {"type":"stdio", "command":"./missing launcher", "args":[]}
        document["mcpServers"]["magistr"] = custom
        config.write_text(json.dumps(document))
        result = self.run_cli("doctor", ok=False)
        self.assertIn("configured Magistr MCP command", result.stdout + result.stderr)
        self.assertEqual(json.loads(config.read_text())["mcpServers"]["magistr"], custom)

    def test_terminal_interview_reviews_changes_before_apply(self):
        def review(text):
            self.assertIn("Proposed project setup", text)
            self.assertIn("cargo check --locked", text)
            self.assertFalse((self.root / ".magistr").exists())
        result = interview(BINARY, self.root, self.env, True, review)
        self.assertIn("read-only context check passed", result)
        self.assertIn("Starter prompt:", result)
        self.assertTrue((self.root / ".codex/config.toml").exists())
        self.assertTrue((self.root / ".mcp.json").exists())
        project = json.loads((self.root / ".magistr/project.json").read_text())
        self.assertEqual(project["project"]["type"], "library")
        self.assertEqual(project["project"]["description"], "Interactive setup acceptance")
        self.assertEqual(project["quality_gate"]["check"], "cargo check --locked")
        self.assertEqual(project["quality_gate"]["test"], "cargo test --lib")
        self.assertFalse((self.root / "target").exists())

    def test_terminal_decline_leaves_project_untouched(self):
        before = {str(p.relative_to(self.root)): p.read_bytes()
                  for p in self.root.rglob("*") if p.is_file()}
        result = interview(BINARY, self.root, self.env, False, lambda text: None)
        self.assertIn("Setup cancelled. No files changed.", result)
        after = {str(p.relative_to(self.root)): p.read_bytes()
                 for p in self.root.rglob("*") if p.is_file()}
        self.assertEqual(after, before)
        self.assertFalse((self.root / ".magistr").exists())
        self.assertFalse((self.root / ".codex").exists())
        self.assertFalse((self.root / ".claude").exists())

    def test_doctor_reports_actionable_missing_setup(self):
        result = self.run_cli("--json", "doctor", ok=False)
        self.assertIn("init", result.stdout + result.stderr)

if __name__ == "__main__":
    unittest.main(verbosity=2)
