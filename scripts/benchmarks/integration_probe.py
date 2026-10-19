"""Exercise the generated Codex/MCP setup in a separate, unscored fixture."""

import argparse
import json
import os
import sys
from pathlib import Path

from pilot_protocol import ORIGINAL_DEADLINE, run_configs
from summarize_pilot import trial_row

INSTRUCTION = """
Verify the tools installed in this small project. Use the generated Magistr and
Workspace MCP tools, and follow their schemas and confirmation-token responses.
1. Inspect project checks. Run the test gate and report it as unconfigured.
2. Create probe.md using files(create), read its content hash, then populate it
   with edit(replace_text), old:"", new:"native writes work\\n" and expectedHash.
3. Repeat an empty-anchor edit against that populated file. It must refuse;
   preserve the original text.
4. Create queued.txt with files(create). Use script's edit.queue to replace its
   empty content with "queued writes work\\n"; review and confirm if requested.
Do not write either file using shell commands. Do not add test tools or commands.
Finish after these probes and report what succeeded or was refused.
"""

VERIFIER = """#!/bin/bash
set -eu
python3 - <<'PY'
import json
from pathlib import Path
root = Path('/workspace')
config = json.loads((root / '.magistr/project.json').read_text())
assert all(config['quality_gate'][name] == '' for name in ('check','test','lint'))
assert (root / 'probe.md').read_text() == 'native writes work\\n'
assert (root / 'queued.txt').read_text() == 'queued writes work\\n'
Path('/logs/verifier/reward.txt').write_text('1')
PY
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    task = args.output / "task"
    (task / "environment").mkdir(parents=True)
    (task / "tests").mkdir()
    (task / "instruction.md").write_text(INSTRUCTION + "\n" + ORIGINAL_DEADLINE)
    (task / "task.toml").write_text(
        'version = "1.0"\n[agent]\ntimeout_sec = 1800\n'
        "[verifier]\ntimeout_sec = 120\n[environment]\n"
        "cpus = 2\nmemory_mb = 2048\nstorage_mb = 10240\n"
    )
    (task / "environment" / "Dockerfile").write_text(
        "FROM python:3.13-slim\nRUN mkdir /workspace\n"
        "WORKDIR /workspace\nCOPY package.json index.ts ./\n"
    )
    (task / "environment" / "package.json").write_text(
        json.dumps(
            {
                "name": "native-integration-probe",
                "packageManager": "npm@10",
                "scripts": {"smoke": "node index.ts"},
            }
        )
    )
    (task / "environment" / "index.ts").write_text("export const value = 1;\n")
    (task / "tests" / "test.sh").write_text(VERIFIER)
    from harbor.models.job.config import JobConfig
    from harbor.models.trial.config import AgentConfig, EnvironmentConfig, TaskConfig

    config = JobConfig(
        job_name="generated-integration",
        jobs_dir=args.output / "jobs",
        n_attempts=1,
        n_concurrent_trials=1,
        environment=EnvironmentConfig(type="docker"),
        agents=[
            AgentConfig(
                import_path="harbor_paired:PairedCodex",
                model_name="openai/gpt-5.6-sol",
                override_timeout_sec=1800,
                kwargs={
                    "version": "0.155.1",
                    "reasoning_effort": "high",
                    "web_search": "disabled",
                    "pilot_timeout_sec": 1800,
                    "magistr_binary": str(args.binary.resolve()),
                },
            )
        ],
        tasks=[TaskConfig(path=task.resolve())],
    )
    config_path = args.output / "integration.json"
    config_path.write_text(config.model_dump_json(indent=2))
    runs = run_configs([config_path], args.output, sys.executable, os.environ.copy())
    paths = list((args.output / "jobs" / "generated-integration").glob("*/result.json"))
    if len(paths) != 1:
        raise RuntimeError("Integration did not produce exactly one result")
    row = trial_row(paths[0], "integration")
    calls = row["mcp_calls"]
    required = ("files", "read", "edit", "script", "magistr_gate")
    used = all(any(name.endswith("/" + tool) for name in calls) for tool in required)
    receipt = {
        "passed": runs[0]["exit_code"] == 0 and row["status"] == "passed" and used,
        "actual_required_tools_used": used,
        "trial": row,
    }
    (args.output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if not receipt["passed"]:
        raise RuntimeError("Generated Codex/MCP integration probe failed")


if __name__ == "__main__":
    main()
