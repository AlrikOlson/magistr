"""Regression checks for ordering, deadlines, runtime and preflight admission."""

import asyncio
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, patch

from pilot_protocol import (
    ORIGINAL_DEADLINE,
    TASKS,
    TIMEOUT_SECONDS,
    pilot_instruction,
    run_configs,
    runtime_command,
    trial_order,
)
from summarize_pilot import summarize
from terminal_bench_pilot import digest, execute, frozen_write, prepare


class ProtocolTests(unittest.TestCase):
    def test_deadline_changes_only_the_budget_sentence(self):
        original = "Keep 28800 records.\n" + ORIGINAL_DEADLINE + " Do not cheat.\n"
        actual = pilot_instruction(original, TIMEOUT_SECONDS)
        self.assertEqual(
            actual,
            "Keep 28800 records.\nYou have 1800 seconds to complete this task. Do not cheat.\n",
        )
        for value in (original + ORIGINAL_DEADLINE, "no deadline"):
            with self.assertRaises(ValueError):
                pilot_instruction(value, TIMEOUT_SECONDS)

    def test_nonfinite_or_nonpositive_deadlines_are_refused(self):
        for seconds in (0, -1, float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                pilot_instruction(ORIGINAL_DEADLINE, seconds)

    def test_alternating_pairs_are_complete_and_predetermined(self):
        order = trial_order()
        self.assertEqual(
            [j["arm"] for j in order],
            ["baseline", "magistr", "magistr", "baseline", "baseline", "magistr"],
        )
        self.assertEqual([j["position"] for j in order], list(range(1, 7)))
        for task in TASKS:
            self.assertEqual(
                {j["arm"] for j in order if j["task"] == task}, {"baseline", "magistr"}
            )

    def test_runtime_activates_nvm_and_preserves_pipeline_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            nvm = Path(directory)
            (nvm / "nvm.sh").write_text("export PILOT_RUNTIME_LOADED=1\n")
            env = {**os.environ, "NVM_DIR": directory}
            result = subprocess.run(
                ["bash", "-c", runtime_command('test "$PILOT_RUNTIME_LOADED" = 1')],
                env=env,
                check=False,
            )
            self.assertEqual(result.returncode, 0)
            result = subprocess.run(
                ["bash", "-c", runtime_command("false | true")], env=env, check=False
            )
            self.assertNotEqual(result.returncode, 0)

    def test_serial_execution_keeps_all_jobs_after_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            configs = [root / f"{i}.json" for i in range(6)]
            with patch(
                "pilot_protocol.subprocess.run",
                side_effect=[
                    types.SimpleNamespace(returncode=n) for n in (1, 0, 0, 0, 0, 0)
                ],
            ) as run:
                outcomes = run_configs(configs, root, "python", {})
            self.assertEqual(
                [call.args[0][-1] for call in run.call_args_list],
                list(map(str, configs)),
            )
            self.assertEqual(len(outcomes), 6)
            self.assertEqual(outcomes[0]["exit_code"], 1)

    def test_missing_results_are_visible_and_execution_cannot_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = {
                "scored_jobs": [
                    {
                        **j,
                        "config": f"{j['position']}.json",
                        "result_glob": f"missing/{j['position']}/*/result.json",
                    }
                    for j in trial_order()
                ]
            }
            frozen_write(root / "manifest.json", json.dumps(manifest))
            result = summarize(root)
            self.assertEqual(
                result["arms"]["baseline"]["statuses"], {"missing_result": 3}
            )
            (root / "preflight").mkdir()
            receipt = {
                "passed": False,
                "manifest_sha256": digest(root / "manifest.json"),
            }
            (root / "preflight" / "controls.json").write_text(json.dumps(receipt))
            args = types.SimpleNamespace(output=root)
            with self.assertRaises(ValueError):
                execute(args, manifest, {})
            self.assertFalse((root / "scoring-started.json").exists())
            receipt["passed"] = True
            (root / "preflight" / "controls.json").write_text(json.dumps(receipt))
            with patch("terminal_bench_pilot.run_configs", return_value=[]):
                execute(args, manifest, {})
                with self.assertRaises(FileExistsError):
                    execute(args, manifest, {})

    def test_frozen_bytes_cannot_change(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "adapter.py"
            frozen_write(path, "first")
            frozen_write(path, "first")
            with self.assertRaises(ValueError):
                frozen_write(path, "second")


class AdapterTests(unittest.TestCase):
    def test_setup_and_execution_share_runtime_and_execution_gets_actual_deadline(self):
        class Base:
            def __init__(self, logs_dir, **kwargs):
                self.logs_dir = logs_dir
                self.model_name = "openai/gpt-5.6-sol"
                self._REMOTE_CODEX_HOME = Path("/tmp/probe-codex")
                self._REMOTE_CODEX_SECRETS_DIR = Path("/tmp/probe-secrets")
                self.exec_as_agent = AsyncMock(side_effect=self.respond)
                self.exec_as_root = AsyncMock()

            async def respond(self, environment, command, **kwargs):
                stdout = "/workspace\n" if command.endswith("pwd") else ""
                if "codex --version" in command:
                    stdout = "codex-cli 0.155.1\nv22.0.0\n10.0.0\n"
                return types.SimpleNamespace(return_code=0, stdout=stdout, stderr="")

            async def setup(self, environment):
                pass

            def _resolve_auth_json_path(self):
                return Path("/unused-synthetic-auth")

            def build_cli_flags(self):
                return '-c model_reasoning_effort="high" -c web_search="disabled"'

        codex = types.ModuleType("harbor.agents.installed.codex")
        codex.Codex = Base
        context = types.ModuleType("harbor.models.agent.context")
        context.AgentContext = object
        with patch.dict(
            sys.modules,
            {
                "harbor.agents.installed.codex": codex,
                "harbor.models.agent.context": context,
            },
        ):
            spec = importlib.util.spec_from_file_location(
                "adapter_under_test", Path(__file__).with_name("harbor_paired.py")
            )
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            agent = module.PairedCodex(
                logs_dir=Path(directory),
                magistr_binary="/binary",
                pilot_timeout_sec=TIMEOUT_SECONDS,
            )
            environment = types.SimpleNamespace(
                upload_file=AsyncMock(), default_user=None
            )

            async def exercise():
                await agent.setup(environment)
                await agent.run(
                    "Do the task.\n" + ORIGINAL_DEADLINE, environment, object()
                )

            asyncio.run(exercise())
            commands = [c.kwargs["command"] for c in agent.exec_as_agent.call_args_list]
            init = next(c for c in commands if "magistr init" in c)
            execution = next(c for c in commands if "codex-launch" in c)
            self.assertTrue(init.startswith(runtime_command("")))
            self.assertTrue(execution.startswith(runtime_command("")))
            self.assertIn("You have 1800 seconds", execution)
            self.assertNotIn("28800", execution)


@unittest.skipUnless(
    importlib.util.find_spec("harbor"),
    "Run with pinned Harbor Python to check job schemas",
)
class HarborConfigTests(unittest.TestCase):
    def test_frozen_jobs_enforce_the_communicated_budget_and_no_retries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkout = root / "checkout"
            for task in TASKS:
                path = checkout / "tasks" / task
                path.mkdir(parents=True)
                (path / "task.toml").write_text(
                    '[metadata]\ncategory="Software"\n[environment]\nmemory_mb=2048\ncpus=2\n'
                )
            binary = root / "binary"
            binary.write_bytes(b"fixture")
            args = types.SimpleNamespace(
                checkout=checkout,
                output=root / "out",
                binary=binary,
                magistr_revision="fixture",
                model="openai/gpt-5.6-sol",
            )
            with patch(
                "terminal_bench_pilot.subprocess.check_output",
                side_effect=["452bf305c6daa62fc59061d22133a7cbc7c1572e\n", ""],
            ):
                manifest = prepare(args)
            for job in manifest["scored_jobs"]:
                config = json.loads((args.output / job["config"]).read_text())
                self.assertEqual(
                    config["agents"][0]["override_timeout_sec"], TIMEOUT_SECONDS
                )
                self.assertEqual(
                    config["agents"][0]["kwargs"]["pilot_timeout_sec"], TIMEOUT_SECONDS
                )
                self.assertEqual(config["retry"]["max_retries"], 0)
                self.assertEqual(len(config["tasks"]), 1)
                self.assertEqual(config["n_concurrent_trials"], 1)


if __name__ == "__main__":
    unittest.main()
