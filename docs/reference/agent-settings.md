# Choose your agent and model

Save your preferences once:

```sh
magistr config set agent codex
magistr config set model YOUR_MODEL
magistr config set effort high
```

These settings apply to native `magistr run`, `magistr next`, and
`magistr next-loop`. Models and reasoning effort are saved separately for each
agent. You can save another agent's preferences without switching:

```sh
magistr config set model YOUR_CLAUDE_MODEL --agent claude
```

Use `--project` to save a preference for the current project:

```sh
magistr config set agent codex --project
magistr config set model YOUR_MODEL --project
```

Project preferences live in `.magistr/agents.toml`. Commit this file to share
preferences with your team. For projects created before these settings were
added, allow `agents.toml` in `.magistr/.gitignore` or add it with `git add -f`.

Example:

```toml
agent = "codex"

[models]
codex = "YOUR_MODEL"

[efforts]
codex = "high"
```

Personal preferences live in `$XDG_CONFIG_HOME/magistr/config.toml`, or
`$HOME/.config/magistr/config.toml`. Existing `[session] default_provider` and
`[models] claude` / `copilot` settings continue to work. `[models]` also accepts
Codex and other named agents; `[efforts]` saves effort by agent.

Override a preference for one command:

```sh
magistr next --agent claude --model YOUR_MODEL --effort high
magistr next-loop --agent codex --model YOUR_MODEL --max-iterations 3
magistr next --show-config
```

On Windows, `run`, `next`, and `next-loop` use plain text automatically.
Run the same commands in PowerShell; `--no-tui` is optional. Agent preferences
and saved loop progress still apply.

`--provider` is an alias for `--agent`; `-m` is short for `--model`.
`--show-config` prints JSON with the selected values and their sources, then
exits without opening an agent session or starting work. Startup also prints
the selection. Runtime authentication and available model/effort choices are
checked when the session opens, before the prompt is sent.

Command options take precedence over project preferences, then personal
preferences, then the agent's defaults. `--agent auto` explicitly requests
automatic selection. Use `default` for a model or effort to let the agent
choose, including when overriding a saved preference. Project restrictions
still apply. An unavailable explicit choice fails rather than switching
to another agent or model.

An unfinished loop keeps its saved agent settings across fresh sessions and
recovery, even if preferences change. Conflicting command options are refused.
If project restrictions stop allowing the saved model, startup fails instead
of substituting another model.
A fully settled loop can start a new run with new preferences. Values left as
`agent default` remain the agent's choice rather than a pinned model or effort.

`next` and `next-loop` require an agent that can edit the project: Claude,
Copilot, or Codex. OpenAI, Gemini, and Ollama are available through `run`.

The `$next` / `/next` skills run inside your current host session. Choose that
session's agent and model in the host or its launcher. Native Magistr settings
do not change a running host session. A skill-based loop includes the host's
reported selection and any requested changes in its fresh-session handoff.