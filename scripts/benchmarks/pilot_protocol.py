"""Shared, testable rules for the fixed paired pilot."""

import math
import subprocess
from pathlib import Path

TIMEOUT_SECONDS = 1800
ORIGINAL_DEADLINE = "You have 28800 seconds to complete this task."
TASKS = ("bun-sourcemap-leak", "session-window-debug", "ontology-kg-querying")
ARMS = ("baseline", "magistr")


def pilot_instruction(instruction, seconds):
    if not math.isfinite(seconds) or seconds <= 0:
        raise ValueError("A finite positive deadline is required")
    if instruction.count(ORIGINAL_DEADLINE) != 1:
        raise ValueError("Expected exactly one pinned upstream deadline notice")
    return instruction.replace(
        ORIGINAL_DEADLINE, f"You have {seconds:g} seconds to complete this task.", 1
    )


def runtime_command(command):
    # Codex installation places Node/npm in NVM, outside a non-login shell's PATH.
    return (
        "set -o pipefail; "
        'if [ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]; then '
        '. "${NVM_DIR:-$HOME/.nvm}/nvm.sh"; fi; ' + command
    )


def trial_order():
    return [
        {"position": pair * 2 + offset + 1, "task": task, "arm": arm}
        for pair, task in enumerate(TASKS)
        for offset, arm in enumerate(ARMS if pair % 2 == 0 else ARMS[::-1])
    ]


def run_configs(configs, output, python, env):
    """Run every scheduled job once, in order, even if a job reports an error."""
    results = []
    for config in configs:
        log_path = output / (Path(config).stem + ".log")
        with log_path.open("x") as log:
            result = subprocess.run(
                [python, "-m", "harbor.cli.main", "run", "--config", str(config)],
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                check=False,
            )
        results.append({"config": str(config), "exit_code": result.returncode})
    return results
