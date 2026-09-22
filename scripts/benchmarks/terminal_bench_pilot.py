"""Prepare, preflight or execute the fixed Terminal-Bench 4.0 paired pilot."""

import argparse
import hashlib
import importlib.metadata
import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path

from pilot_protocol import ARMS, TASKS, TIMEOUT_SECONDS, run_configs, trial_order

BENCHMARK_COMMIT = "452bf305c6daa62fc59061d22133a7cbc7c1572e"
SEED = "magistr-pilot-2026-09-21:"
CODEX_VERSION = "0.155.1"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def frozen_write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    data = content.encode() if isinstance(content, str) else content
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"Frozen artifact differs: {path}")
    else:
        path.write_bytes(data)


def prepare(args):
    if importlib.metadata.version("harbor") != "0.18.0":
        raise ValueError("This adapter was verified with Harbor 0.18.0")
    revision = subprocess.check_output(
        ["git", "-C", str(args.checkout), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != BENCHMARK_COMMIT:
        raise ValueError("Checkout must be Terminal-Bench v4.0.0")
    if subprocess.check_output(
        ["git", "-C", str(args.checkout), "status", "--porcelain"], text=True
    ).strip():
        raise ValueError("Benchmark checkout must be unchanged")
    candidates = []
    for path in (args.checkout / "tasks").glob("*/task.toml"):
        data = tomllib.loads(path.read_text())
        env = data["environment"]
        if (
            data.get("metadata", {}).get("category") == "Software"
            and not env.get("gpus", 0)
            and env.get("memory_mb", 0) <= 4096
        ):
            candidates.append(path.parent.name)
    candidates.sort(key=lambda name: hashlib.sha256((SEED + name).encode()).hexdigest())
    if candidates[:3] != list(TASKS):
        raise ValueError("The pinned task selection changed")
    files = {}
    resources = {}
    for name in TASKS:
        task = args.checkout / "tasks" / name
        resources[name] = tomllib.loads((task / "task.toml").read_text())["environment"]
        for path in sorted(task.rglob("*")):
            if path.is_file():
                files[str(path.relative_to(args.checkout))] = digest(path)
    scripts = {}
    for name in (
        "harbor_paired.py",
        "pilot_protocol.py",
        "terminal_bench_pilot.py",
        "summarize_pilot.py",
        "integration_probe.py",
    ):
        source = Path(__file__).with_name(name)
        snapshot = args.output / "adapter" / name
        frozen_write(snapshot, source.read_bytes())
        scripts[name] = digest(snapshot)

    from harbor.models.job.config import JobConfig
    from harbor.models.trial.config import AgentConfig, EnvironmentConfig, TaskConfig

    def config(task, arm, job_name, jobs_dir, install_only=False):
        kwargs = {
            "version": CODEX_VERSION,
            "reasoning_effort": "high",
            "web_search": "disabled",
            "pilot_timeout_sec": TIMEOUT_SECONDS,
        }
        if arm == "magistr":
            kwargs["magistr_binary"] = str(args.binary)
        agent = (
            AgentConfig(name="oracle")
            if arm == "reference"
            else AgentConfig(
                import_path="harbor_paired:PairedCodex",
                model_name=args.model,
                override_timeout_sec=TIMEOUT_SECONDS,
                kwargs=kwargs,
            )
        )
        return JobConfig(
            job_name=job_name,
            jobs_dir=jobs_dir,
            install_only=install_only,
            n_attempts=1,
            n_concurrent_trials=1,
            environment=EnvironmentConfig(type="docker"),
            agents=[agent],
            tasks=[TaskConfig(path=args.checkout / "tasks" / task)],
        )

    jobs = []
    configs = {}
    for trial in trial_order():
        name = f"{trial['position']:02d}-{trial['arm']}-{trial['task']}"
        path = args.output / "configs" / (name + ".json")
        job = config(
            trial["task"], trial["arm"], name, args.output / "jobs" / trial["arm"]
        )
        frozen_write(path, job.model_dump_json(indent=2))
        configs[str(path.relative_to(args.output))] = digest(path)
        jobs.append(
            {
                **trial,
                "config": str(path.relative_to(args.output)),
                "result_glob": f"jobs/{trial['arm']}/{name}/*/result.json",
            }
        )
    controls = []
    for arm in ("reference", *ARMS):
        for task in TASKS:
            name = f"{arm}-{task}"
            path = args.output / "configs" / ("control-" + name + ".json")
            job = config(
                task,
                arm,
                name,
                args.output / "preflight" / "jobs",
                install_only=arm != "reference",
            )
            frozen_write(path, job.model_dump_json(indent=2))
            configs[str(path.relative_to(args.output))] = digest(path)
            controls.append(
                {
                    "arm": arm,
                    "task": task,
                    "config": str(path.relative_to(args.output)),
                    "result_glob": f"preflight/jobs/{name}/*/result.json",
                }
            )
    manifest = {
        "protocol": "paired-pilot-1800-v1",
        "benchmark": "terminal-bench@4.0.0",
        "benchmark_commit": revision,
        "selection_seed": SEED,
        "eligible_tasks_in_order": candidates,
        "selected_tasks": list(TASKS),
        "task_file_sha256": files,
        "task_resources": resources,
        "magistr_revision": args.magistr_revision,
        "magistr_binary_sha256": digest(args.binary),
        "script_sha256": scripts,
        "config_sha256": configs,
        "harbor_version": "0.18.0",
        "codex_version": CODEX_VERSION,
        "model": args.model,
        "reasoning_effort": "high",
        "web_search": "disabled",
        "agent_timeout_sec": TIMEOUT_SECONDS,
        "communicated_timeout_sec": TIMEOUT_SECONDS,
        "official_agent_timeout_sec": 28800,
        "attempts_per_arm_per_task": 1,
        "parallel_trials": 1,
        "retries": 0,
        "scored_jobs": jobs,
        "controls": controls,
        "leaderboard_comparable": False,
    }
    frozen_write(args.output / "manifest.json", json.dumps(manifest, indent=2) + "\n")
    return manifest


def verify_controls(output, manifest):
    outcomes = []
    for control in manifest["controls"]:
        paths = list(output.glob(control["result_glob"]))
        result = json.loads(paths[0].read_text()) if len(paths) == 1 else {}
        passed = len(paths) == 1 and not result.get("exception_info")
        if control["arm"] == "reference":
            passed = (
                passed
                and (result.get("verifier_result") or {})
                .get("rewards", {})
                .get("reward")
                == 1
            )
        outcomes.append(
            {**control, "passed": bool(passed), "results": [str(p) for p in paths]}
        )
    return outcomes


def run_controls(args, manifest, env):
    if (args.output / "scoring-started.json").exists():
        raise ValueError("Controls cannot overlap or follow scored execution")
    logs = args.output / "preflight" / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    runs = run_configs(
        [args.output / c["config"] for c in manifest["controls"]],
        logs,
        sys.executable,
        env,
    )
    outcomes = verify_controls(args.output, manifest)
    probe = subprocess.run(
        [
            sys.executable,
            str(args.output / "adapter" / "integration_probe.py"),
            "--binary",
            str(args.binary),
            "--output",
            str(args.output / "preflight" / "integration"),
        ],
        env=env,
        check=False,
    )
    receipt = {
        "passed": all(o["passed"] for o in outcomes)
        and all(r["exit_code"] == 0 for r in runs)
        and probe.returncode == 0,
        "manifest_sha256": digest(args.output / "manifest.json"),
        "controls": outcomes,
        "jobs": runs,
        "integration_exit_code": probe.returncode,
    }
    frozen_write(
        args.output / "preflight" / "controls.json",
        json.dumps(receipt, indent=2) + "\n",
    )
    if not receipt["passed"]:
        raise ValueError("Preflight failed; no scored trials may start")


def execute(args, manifest, env):
    receipt = json.loads((args.output / "preflight" / "controls.json").read_text())
    if not receipt["passed"] or receipt["manifest_sha256"] != digest(
        args.output / "manifest.json"
    ):
        raise ValueError("Current successful controls are required")
    marker = args.output / "scoring-started.json"
    with marker.open("x") as handle:
        json.dump({"order": manifest["scored_jobs"]}, handle, indent=2)
    logs = args.output / "logs"
    logs.mkdir(exist_ok=True)
    results = run_configs(
        [args.output / j["config"] for j in manifest["scored_jobs"]],
        logs,
        sys.executable,
        env,
    )
    frozen_write(args.output / "execution.json", json.dumps(results, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkout", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--magistr-revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--model", default="openai/gpt-5.6-sol", choices=["openai/gpt-5.6-sol"]
    )
    phase = parser.add_mutually_exclusive_group()
    phase.add_argument("--controls", action="store_true")
    phase.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    for field in ("checkout", "binary", "output"):
        setattr(args, field, getattr(args, field).resolve())
    manifest = prepare(args)
    print(json.dumps({"tasks": list(TASKS), "output": str(args.output)}), flush=True)
    env = os.environ.copy()
    env["PYTHONPATH"] = str(args.output / "adapter")
    if (args.controls or args.execute) and not env.get("CODEX_AUTH_JSON_PATH"):
        parser.error("Set CODEX_AUTH_JSON_PATH to your existing Codex auth.json")
    if args.controls:
        run_controls(args, manifest, env)
    if args.execute:
        execute(args, manifest, env)


if __name__ == "__main__":
    main()
