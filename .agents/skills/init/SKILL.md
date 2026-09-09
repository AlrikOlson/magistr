---
name: init
description: Interview the user and tailor Magistr configuration to the detected project stack and workflow. Use for $init or requests to initialize or customize project setup.
---

# Tailor Magistr project setup

Read [the runtime contract](../craft/references/runtime.md), its host adapter,
root project instructions, `.magistr/project.json`, and existing workbench
configuration. Ground stack detection in manifests and declared commands.
Summarize existing setup, then ask only missing project-specific questions.
Honor previous answers; do not restart the interview on a repeated invocation.

Useful questions concern project purpose, references, unusual conventions,
required quality gates, and explicit exclusions. Use the host's available
question mechanism. Research current primary documentation where external
tooling choices affect the configuration.

Update only instructions and configuration needed for the requested setup.
Keep `.magistr/project.json` authoritative for gates and roadmap policy.
Preserve the `.think-and-ship/project.json` identity, the **magistr** state
endpoint, unrelated host settings, and user customization. Keep project entry
instructions concise and link shared conventions. Never copy secrets into
new configuration or reset native project state.

Do not run `magistr init` wholesale over established setup merely to refresh
instructions. When scaffolding is requested, inspect its supported targets
and preview first. Follow the host adapter for configuration paths and host
verification. Validate syntax, skill discovery, and read-only MCP connectivity;
report changes and any reload required. Do not invoke `$next`, run a loop,
or mutate roadmap work as a setup test.
