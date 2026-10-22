#!/usr/bin/env python3
"""Exercise version drift and real Cargo resolution in an isolated workspace."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("release_version", Path(__file__).with_name("version.py"))
version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(version)


class VersionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="magistr version fixture ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.write("Cargo.toml", """[workspace]
members = ["crates/magistr-domain", "crates/magistr-cli"]
exclude = ["crates/magistr-desktop", "vendor/independent"]
resolver = "3"
[workspace.package]
version = "0.1.0"
[workspace.dependencies]
magistr-domain = { version = "=0.1.0", path = "crates/magistr-domain" }
magistr-cli = { version = "=0.1.0", path = "crates/magistr-cli" }
""")
        for name in ("magistr-domain", "magistr-cli"):
            dependencies = ""
            if name == "magistr-cli":
                dependencies = """[dependencies]
magistr-domain = { workspace = true }
independent = { path = "../../vendor/independent" }
"""
            self.write(f"crates/{name}/Cargo.toml",
                       f'[package]\nname = "{name}"\nversion.workspace = true\nedition = "2024"\n' + dependencies)
            self.write(f"crates/{name}/src/lib.rs", "")
        self.write("vendor/independent/Cargo.toml",
                   '[workspace]\n[package]\nname = "independent"\nversion = "9.2.0"\nedition = "2024"\n')
        self.write("vendor/independent/src/lib.rs", "")
        self.write(version.DESKTOP, """[workspace]
[package]
name = "magistr-desktop"
version = "0.1.0"
edition = "2024"
[dependencies]
magistr-domain = { path = "../magistr-domain" }
""")
        self.write("crates/magistr-desktop/src/lib.rs", "")
        self.write("crates/magistr-desktop/tauri.conf.json", "{}\n")
        for relative in version.JSON_COPIES:
            self.write(relative, json.dumps({"name": "magistr", "version": "0.1.0", "description": "keep me"}, indent=2) + "\n")
        for relative in version.PRIVATE_PACKAGES:
            self.write(relative, '{"private": true}\n')
            self.write(str(Path(relative).with_name("package-lock.json")), '{"packages":{"":{}}}\n')
        version.sync(self.root)

    def write(self, relative, content):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def snapshot(self):
        return {str(p.relative_to(self.root)): p.read_bytes() for p in self.root.rglob("*") if p.is_file()}

    def test_real_bump_updates_both_cargo_graphs_and_all_product_consumers(self):
        version.sync(self.root, "0.2.0-rc.1")
        self.assertEqual(version.check(self.root), "0.2.0-rc.1")
        for relative in version.JSON_COPIES:
            data = json.loads((self.root / relative).read_text())
            self.assertEqual(data["version"], "0.2.0-rc.1")
            self.assertEqual(data["description"], "keep me")
        for relative in ("Cargo.lock", "crates/magistr-desktop/Cargo.lock"):
            packages = version.toml(self.root / relative)["package"]
            self.assertTrue(all(p["version"] == "0.2.0-rc.1" for p in packages if p["name"].startswith("magistr-")))
        independent = next(p for p in version.toml(self.root / "Cargo.lock")["package"] if p["name"] == "independent")
        self.assertEqual(independent["version"], "9.2.0")

    def test_check_and_repeated_sync_do_not_change_files(self):
        before = self.snapshot()
        version.check(self.root)
        version.sync(self.root)
        self.assertEqual(self.snapshot(), before)

    def test_every_required_generated_copy_is_checked(self):
        paths = ["Cargo.toml", version.DESKTOP, *version.JSON_COPIES, "Cargo.lock",
                 "crates/magistr-desktop/Cargo.lock"]
        for relative in paths:
            with self.subTest(path=relative):
                path = self.root / relative
                original = path.read_text()
                path.write_text(original.replace("0.1.0", "0.9.0"))
                with self.assertRaises(ValueError):
                    version.check(self.root)
                path.write_text(original)

    def test_new_members_cannot_reintroduce_independent_versions_or_dependencies(self):
        path = self.root / "crates/magistr-cli/Cargo.toml"
        original = path.read_text()
        for changed in (original.replace("version.workspace = true", 'version = "0.1.0"'),
                        original.replace("{ workspace = true }", '{ version = "=0.1.0", path = "../magistr-domain" }')):
            path.write_text(changed)
            with self.assertRaises(ValueError):
                version.check(self.root)
        path.write_text(original)

    def test_invalid_versions_fail_before_any_write(self):
        before = self.snapshot()
        for candidate in ("", "1", "01.2.3", "1.2.3-01", "v1.2.3", "1.2.3-"):
            with self.subTest(version=candidate), self.assertRaises(ValueError):
                version.sync(self.root, candidate)
        self.assertEqual(self.snapshot(), before)

    def test_pre_release_versions_publish_on_the_beta_channel(self):
        for candidate in ("1.0.0-beta.1", "0.2.0-rc.1", "2.0.0-alpha"):
            with self.subTest(version=candidate):
                self.assertEqual(version.npm_dist_tag(candidate), "beta")
        for candidate in ("1.0.0", "0.1.0", "1.2.3+build.5"):
            with self.subTest(version=candidate):
                self.assertEqual(version.npm_dist_tag(candidate), "latest")
        with self.assertRaises(ValueError):
            version.npm_dist_tag("1.0.0.beta-1")

    def test_beta_round_trip_keeps_every_consumer_on_the_pre_release(self):
        version.sync(self.root, "1.0.0-beta.1")
        self.assertEqual(version.check(self.root), "1.0.0-beta.1")
        self.assertEqual(version.npm_dist_tag(version.product_version(self.root)), "beta")
        workspace = version.toml(self.root / "Cargo.toml")["workspace"]
        self.assertEqual(workspace["dependencies"]["magistr-cli"]["version"], "=1.0.0-beta.1")
        for relative in version.JSON_COPIES:
            self.assertEqual(json.loads((self.root / relative).read_text())["version"], "1.0.0-beta.1")

    def test_missing_product_lock_entry_and_extra_tauri_version_are_refused(self):
        path = self.root / "Cargo.lock"
        original = path.read_text()
        path.write_text('version = 4\npackage = []\n')
        with self.assertRaisesRegex(ValueError, "missing product packages"):
            version.check(self.root)
        path.write_text(original)
        self.write("crates/magistr-desktop/tauri.conf.json", '{"version":"0.1.0"}\n')
        with self.assertRaisesRegex(ValueError, "Tauri"):
            version.check(self.root)


if __name__ == "__main__":
    unittest.main()
