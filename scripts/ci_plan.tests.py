#!/usr/bin/env python3
"""The verification selector must never turn missing coverage into a green check."""
from datetime import datetime, timedelta, timezone
import unittest
import ci_plan as ci
from ci_timing import run


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.graph = {"leaf": set(), "consumer": {"leaf"}, "other": set(),
                      "magistr-desktop": {"consumer"}}
        self.locations = {f"crates/{name}": name for name in self.graph}

    def plan(self, *paths):
        return ci.affected(paths, self.graph, self.locations)

    def test_website_does_not_compile_rust_or_package_desktop(self):
        plan = self.plan("apps/website/src/pages/index.astro", "README.md")
        self.assertTrue(plan["website"])
        self.assertFalse(any(plan[key] for key in ("rust", "desktop", "package")))

    def test_reverse_dependencies_include_desktop_and_exclude_unrelated_crates(self):
        plan = self.plan("crates/leaf/src/lib.rs")
        self.assertEqual(plan["rust_packages"], ["consumer", "leaf"])
        self.assertTrue(plan["desktop"])
        self.assertTrue(plan["contracts"])
        self.assertFalse(plan["package"])

    def test_unknown_inputs_and_workflows_select_every_gate(self):
        for path in ("new-generator.py", ".github/workflows/public-release.yml",
                     "crates/new-owner/src/lib.rs", "Cargo.lock", "crates/leaf/Cargo.toml"):
            with self.subTest(path=path):
                plan = self.plan(path)
                self.assertTrue(all(plan[key] for key in ci.JOBS))

    def test_desktop_only_and_visual_evidence_select_their_owners(self):
        self.assertFalse(self.plan("crates/magistr-desktop/ui/index.html")["rust"])
        self.assertTrue(self.plan("docs/evidence/iteration/review.png")["visual"])

    def test_current_manifest_graph_includes_consumers(self):
        graph, locations = ci.graph()
        plan = ci.affected(["crates/magistr-workflow-contracts/src/lib.rs"], graph, locations)
        self.assertIn("magistr-cli", plan["rust_packages"])
        self.assertTrue(plan["desktop"])

    def needs(self, plan):
        result = {"plan": {"result": "success"}, "policy": {"result": "success"}}
        for kind in ci.JOBS:
            for name in (("rust-build", "rust-tests") if kind == "rust" else (kind,)):
                result[name] = {"result": "success" if plan[kind] else "skipped"}
        return result

    def test_required_summary_refuses_missing_failed_and_unexpectedly_skipped_jobs(self):
        plan = self.plan("apps/website/a")
        needs = self.needs(plan)
        ci.validate(plan, needs)
        for bad in ("failure", "cancelled", "skipped", None):
            with self.subTest(bad=bad):
                needs["website"]["result"] = bad
                with self.assertRaises(ValueError):
                    ci.validate(plan, needs)

    def test_reuse_needs_a_source_bound_successful_eligible_verification(self):
        now = datetime.now(timezone.utc)
        valid = {"updated_at": now.isoformat(), "head_sha": "a" * 40, "event": "push",
                 "head_branch": "verify/change", "path": ".github/workflows/public-release.yml",
                 "status": "completed", "conclusion": "success"}
        self.assertTrue(ci.reusable(valid, "a" * 40, now))
        for key, value in (("head_sha", "b" * 40), ("event", "workflow_dispatch"),
                           ("head_branch", "main"), ("conclusion", "failure"),
                           ("updated_at", (now - timedelta(hours=25)).isoformat())):
            with self.subTest(key=key):
                self.assertFalse(ci.reusable(dict(valid, **{key: value}), "a" * 40, now))

    def test_stage_deadline_reports_real_command_exit_and_timeout(self):
        import sys
        self.assertEqual(run([sys.executable, "-c", "raise SystemExit(7)"], 2)["exit_code"], 7)
        timed = run([sys.executable, "-c", "import time; time.sleep(30)"], 0.05)
        self.assertTrue(timed["timed_out"])
        self.assertEqual(timed["exit_code"], 124)


if __name__ == "__main__":
    unittest.main()
