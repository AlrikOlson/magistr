#!/usr/bin/env python3
"""Independent evaluation acceptance must preserve intentional diagnostic failures."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("agent_mcp_eval", Path(__file__).with_name("agent-mcp-eval.py"))
evaluation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluation)


def check(name="unittest", *, passed=True, required=True, verified=True, exit_code=0):
    return dict(name=name, passed=passed, required=required, verified=verified, exit_code=exit_code)


class RequiredEvidence(unittest.TestCase):
    def test_reproduced_defect_does_not_invalidate_verified_fix(self):
        records = [check("baseline", passed=False, required=False, exit_code=1), check()]
        self.assertTrue(evaluation.required_checks_passed(records))

    def test_unrecovered_required_failure_blocks(self):
        records = [check("required failure", passed=False, exit_code=1), check()]
        self.assertFalse(evaluation.required_checks_passed(records))

    def test_verified_retry_recovers_the_same_required_check(self):
        records = [check(passed=False, exit_code=1), check()]
        self.assertTrue(evaluation.required_checks_passed(records))

    def test_optional_pass_cannot_hide_a_required_failure(self):
        records = [check(passed=False, exit_code=1), check(required=False)]
        self.assertFalse(evaluation.required_checks_passed(records))

    def test_self_reported_pass_does_not_qualify(self):
        self.assertFalse(evaluation.required_checks_passed([check(verified=False)]))

    def test_success_requires_an_actual_zero_exit_code(self):
        self.assertFalse(evaluation.required_checks_passed([check(exit_code=1)]))
        self.assertFalse(evaluation.required_checks_passed([check(exit_code=None)]))

    def test_absent_required_verification_does_not_qualify(self):
        self.assertFalse(evaluation.required_checks_passed([]))
        self.assertFalse(evaluation.required_checks_passed([check(required=False)]))


if __name__ == "__main__":
    unittest.main()
