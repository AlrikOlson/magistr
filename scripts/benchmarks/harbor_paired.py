"""Paired Codex / Magistr integration for Harbor 0.18.0.

Only the treatment installs Magistr's generated project setup and native tools.
Harbor retains ownership of task isolation, hidden verification and results.
"""

import json
import shlex
from pathlib import Path

from harbor.agents.installed.codex import Codex
from harbor.models.agent.context import AgentContext
from pilot_protocol import pilot_instruction, runtime_command

COMMON_INSTRUCTION = (
    "\n\nWork independently on the task. Do not retrieve benchmark reference "
    "solutions or hidden tests, and do not modify the verifier or its logs."
)


class PairedCodex(Codex):
    def __init__(self, *args, magistr_binary=None, pilot_timeout_sec=1800, **kwargs):
        super().__init__(*args, **kwargs)
        self.magistr_binary = Path(magistr_binary) if magistr_binary else None
        self.project_root = None
        self.pilot_timeout_sec = pilot_timeout_sec

    @staticmethod
    def name():
        return "paired-codex"

    async def checked(self, environment, command, **kwargs):
        result = await self.exec_as_agent(
            environment, command=runtime_command(command), **kwargs
        )
        if result.return_code != 0:
            raise RuntimeError(
                f"Agent command failed ({result.return_code}): "
                f"{(result.stderr or result.stdout or '')[-3000:]}"
            )
        return result

    async def setup(self, environment):
        await super().setup(environment)
        root = await self.checked(environment, "pwd")
        self.project_root = root.stdout.strip()
        if not self.project_root.startswith("/") or self.project_root == "/":
            raise RuntimeError("Benchmark requires a dedicated task working directory")
        await self.exec_as_root(
            environment,
            command="command -v git >/dev/null || "
            "(apt-get update -qq && apt-get install -y -qq git)",
        )
        await self.checked(environment, "git init -q")
        versions = await self.checked(
            environment, "codex --version && node --version && npm --version"
        )
        (self.logs_dir / "runtime-versions.txt").write_text(versions.stdout or "")
        if "codex-cli 0.155.1" not in (versions.stdout or ""):
            raise RuntimeError("Installed Codex does not match the pinned version")
        if self.magistr_binary:
            await environment.upload_file(self.magistr_binary, "/usr/local/bin/magistr")
            await self.exec_as_root(
                environment, command="chmod 755 /usr/local/bin/magistr"
            )
            result = await self.checked(
                environment, "magistr init . --agent codex --yes"
            )
            (self.logs_dir / "magistr-setup.txt").write_text(
                (result.stdout or "") + (result.stderr or "")
            )

    async def run(self, instruction, environment, context: AgentContext):
        if not self.model_name or not self.project_root:
            raise ValueError("Model and completed setup are required")
        auth_path = self._resolve_auth_json_path()
        if not auth_path:
            raise ValueError("Set CODEX_AUTH_JSON_PATH to an existing Codex sign-in")
        home = self._REMOTE_CODEX_HOME.as_posix()
        secret_dir = self._REMOTE_CODEX_SECRETS_DIR.as_posix()
        remote_auth = secret_dir + "/auth.json"
        logs = "/logs/agent"
        env = {"CODEX_HOME": home}
        await self.checked(
            environment,
            shlex.join(["mkdir", "-p", home, secret_dir, logs]),
        )
        await environment.upload_file(auth_path, remote_auth)
        await self.exec_as_root(
            environment, command=shlex.join(["chmod", "600", remote_auth])
        )
        if environment.default_user is not None:
            await self.exec_as_root(
                environment,
                command=shlex.join(["chown", environment.default_user, remote_auth]),
            )
        await self.checked(
            environment,
            shlex.join(["ln", "-sf", remote_auth, home + "/auth.json"]),
        )
        config = (
            f'[projects.{json.dumps(self.project_root)}]\ntrust_level = "trusted"\n'
        )
        local_config = self.logs_dir / "codex-config.toml"
        local_config.write_text(config)
        await environment.upload_file(local_config, home + "/config.toml")
        command = ["codex"]
        if self.magistr_binary:
            command = [
                "magistr",
                "codex-launch",
                "--project-root",
                self.project_root,
                "--",
            ]
        command += [
            "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "--skip-git-repo-check",
            "--model",
            self.model_name.split("/")[-1],
            "--json",
            "--enable",
            "unified_exec",
        ]
        instruction = pilot_instruction(instruction, self.pilot_timeout_sec)
        command_text = (
            shlex.join(command)
            + " "
            + self.build_cli_flags()
            + " -- "
            + shlex.quote(instruction + COMMON_INSTRUCTION)
            + " 2>&1 </dev/null | tee /logs/agent/codex.txt"
        )
        try:
            await self.checked(environment, command_text, env=env)
        finally:
            # Never collect the Codex home: it contains credentials.
            await self.exec_as_agent(
                environment,
                command=(
                    'if [ -d "$CODEX_HOME/sessions" ]; then '
                    'cp -R "$CODEX_HOME/sessions" /logs/agent/sessions; fi'
                ),
                env=env,
            )
            if self.magistr_binary:
                await self.exec_as_agent(
                    environment,
                    command=(
                        "mkdir -p /logs/agent/magistr-evidence; "
                        "for p in .magistr .think-and-ship .codex/config.toml "
                        ".workbench/trace.jsonl; do "
                        'if [ -e "$p" ]; then cp -R --parents "$p" '
                        "/logs/agent/magistr-evidence/; fi; done"
                    ),
                )
            await self.exec_as_agent(
                environment,
                command=shlex.join(["rm", "-rf", secret_dir, home]),
            )
