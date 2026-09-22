"""Protect the distinction between benchmark failure and invalid evaluation."""

import json
import tempfile
import unittest
from pathlib import Path

from summarize_pilot import classify, trial_row


class ResultIntegrityTests(unittest.TestCase):
    def test_run_error_is_not_a_failed_task(self):
        result = {
            "exception_info": {"exception_type": "RuntimeError"},
            "verifier_result": {"rewards": {"reward": 0}},
        }
        self.assertEqual(classify(result), "run_error")

    def test_timeout_stays_visible_even_with_a_reward(self):
        result = {
            "exception_info": {"exception_type": "AgentTimeoutError"},
            "verifier_result": {"rewards": {"reward": 1}},
        }
        self.assertEqual(classify(result), "timeout")

    def test_missing_reward_is_not_zero(self):
        self.assertEqual(classify({"verifier_result": None}), "not_scored")
        self.assertEqual(
            classify({"verifier_result": {"rewards": {"other": 0}}}), "not_scored"
        )

    def test_binary_verdicts(self):
        for reward, status in [(0, "failed"), (1, "passed"), (0.5, "nonbinary_reward")]:
            with self.subTest(reward=reward):
                self.assertEqual(
                    classify({"verifier_result": {"rewards": {"reward": reward}}}),
                    status,
                )

    def test_tool_started_and_completed_count_once(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "agent").mkdir()
            (root / "result.json").write_text(
                json.dumps(
                    {
                        "task_name": "example",
                        "trial_name": "example__trial",
                        "verifier_result": {"rewards": {"reward": 1}},
                    }
                )
            )
            item = {
                "id": "item_1",
                "type": "mcp_tool_call",
                "server": "workbench-mcp",
                "tool": "read",
            }
            events = [
                {"type": "item.started", "item": {**item, "status": "in_progress"}},
                {"type": "item.completed", "item": {**item, "status": "completed"}},
            ]
            (root / "agent" / "codex.txt").write_text(
                "\n".join(json.dumps(event) for event in events)
            )
            row = trial_row(root / "result.json", "magistr")
            self.assertEqual(row["mcp_calls"], {"workbench-mcp/read": 1})
            self.assertEqual(row["mcp_errors"], {})
            events[-1]["item"]["result"] = {"structured_content": {"ok": False}}
            (root / "agent" / "codex.txt").write_text(
                "\n".join(json.dumps(event) for event in events)
            )
            row = trial_row(root / "result.json", "magistr")
            self.assertEqual(row["mcp_errors"], {"workbench-mcp/read": 1})


if __name__ == "__main__":
    unittest.main()
