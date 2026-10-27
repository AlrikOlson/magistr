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

    def test_website_does_not_compile_rust_or_package(self):
        plan = self.plan("apps/website/src/pages/index.astro", "README.md")
        self.assertTrue(plan["website"])
        self.assertFalse(any(plan[key] for key in ("rust", "package")))

    def test_recorded_website_demo_runs_website_checks_without_rust(self):
        for path in ("examples/support-inbox/store.py", "examples/bevy-tic-tac-toe/src/lib.rs",
                     "examples/bevy-tic-tac-toe/Cargo.toml", "examples/bevy-tic-tac-toe/Cargo.lock"):
            with self.subTest(path=path):
                plan = self.plan(path)
                self.assertTrue(plan["website"])
                self.assertFalse(any(plan[key] for key in ("rust", "package", "contracts")))
        self.assertTrue(self.plan("examples/unclassified-project/main.py")["full"])

    def test_reverse_dependencies_exclude_unrelated_crates_and_the_desktop_app(self):
        plan = self.plan("crates/leaf/src/lib.rs")
        self.assertEqual(plan["rust_packages"], ["consumer", "leaf"])
        self.assertNotIn("desktop", plan)
        self.assertTrue(plan["contracts"])
        self.assertFalse(plan["package"])

    def test_unknown_inputs_and_workflows_select_every_gate(self):
        for path in ("new-generator.py", ".github/workflows/ci.yml",
                     "crates/new-owner/src/lib.rs", "Cargo.lock", "crates/leaf/Cargo.toml"):
            with self.subTest(path=path):
                plan = self.plan(path)
                self.assertTrue(all(plan[key] for key in ci.JOBS))

    def test_desktop_only_and_visual_evidence_select_their_owners(self):
        desktop = self.plan("crates/magistr-desktop/ui/index.html")
        self.assertFalse(desktop["rust"] or desktop["full"])
        self.assertTrue(self.plan("docs/evidence/iteration/review.png")["visual"])

    def test_current_manifest_graph_includes_consumers(self):
        graph, locations = ci.graph()
        plan = ci.affected(["crates/magistr-workflow-contracts/src/lib.rs"], graph, locations)
        self.assertIn("magistr-cli", plan["rust_packages"])
        self.assertNotIn("magistr-desktop", plan["rust_packages"])

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

    def test_broad_selections_slice_tests_and_small_ones_stay_on_one_worker(self):
        graph, locations = ci.graph()
        broad = ci.affected(["Cargo.lock"], graph, locations)
        self.assertEqual(broad["slices"], [1, 2, 3])
        self.assertEqual(broad["slice_count"], 3)
        small = self.plan("crates/leaf/src/lib.rs")
        self.assertEqual(small["slices"], [1])
        self.assertEqual(small["slice_count"], 1)
        self.assertNotIn("reuse", small)

    def test_stage_deadline_reports_real_command_exit_and_timeout(self):
        import sys
        self.assertEqual(run([sys.executable, "-c", "raise SystemExit(7)"], 2)["exit_code"], 7)
        timed = run([sys.executable, "-c", "import time; time.sleep(30)"], 0.05)
        self.assertTrue(timed["timed_out"])
        self.assertEqual(timed["exit_code"], 124)


if __name__ == "__main__":
    unittest.main()
