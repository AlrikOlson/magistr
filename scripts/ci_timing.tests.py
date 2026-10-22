#!/usr/bin/env python3
"""Deadlines own descendant processes and retain the right test report."""
from pathlib import Path
import sys
import tempfile
import time
import unittest
from ci_timing import run, select_budget, slow_tests


class TimingTests(unittest.TestCase):
    def test_timeout_cleans_a_descendant_that_ignores_term_after_its_parent_exits(self):
        with tempfile.TemporaryDirectory() as directory:
            heartbeat = Path(directory) / "heartbeat"
            child = ("import signal,time; from pathlib import Path; "
                     "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                     f"p=Path({str(heartbeat)!r}); "
                     "exec('while True:\\n p.write_text(str(time.monotonic()))\\n time.sleep(0.02)')")
            parent = ("import subprocess,sys,time; "
                      f"subprocess.Popen([sys.executable,'-c',{child!r}]); time.sleep(60)")
            result = run([sys.executable, "-c", parent], 0.5)
            self.assertEqual(result["exit_code"], 124)
            self.assertTrue(result["timed_out"])
            self.assertTrue(heartbeat.is_file(), "the child must actually have started")
            final = heartbeat.read_text()
            time.sleep(0.1)
            self.assertEqual(heartbeat.read_text(), final, "a timed-out child survived")
            self.assertLess(result["seconds"], 7)

    def test_cold_budget_applies_only_when_the_rust_cache_missed(self):
        self.assertEqual(select_budget(900, 1500, "true"), (900, False))
        self.assertEqual(select_budget(900, 1500, "false"), (1500, True))
        self.assertEqual(select_budget(900, 1500, ""), (1500, True))
        self.assertEqual(select_budget(900, None, "false"), (900, True))
        self.assertEqual(select_budget(900, None, None), (900, False))

    def test_junit_timings_are_ordered_and_missing_reports_are_not_invented(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "diagnostic.xml"
            self.assertEqual(slow_tests(path), [])
            path.write_text('<testsuites><testsuite><testcase classname="a" name="fast" time="0.1"/>'
                            '<testcase classname="a" name="slow" time="2"/></testsuite></testsuites>')
            self.assertEqual(slow_tests(path)[0], {"name": "a::slow", "seconds": 2.0})


if __name__ == "__main__":
    unittest.main()
