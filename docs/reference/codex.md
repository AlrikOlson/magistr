# Codex integration contract

For project setup, use [Connect an agent](https://magistr.dev/docs/agents#codex).
For everyday work and recovery, use [Skills](https://magistr.dev/docs/skills)
and [Sessions](https://magistr.dev/docs/sessions). This page is the contributor
reference for launcher, selection, and generated-file ownership.

Magistr provides two Codex integrations. The project launcher runs the Codex CLI
directly with project skills and MCP configuration. The
[session provider](../architecture/sessions.md#codex) carries Codex coding sessions
through ACP into Magistr's interfaces. Launching Codex does not select that provider.

## Setup

With Magistr and Codex on `PATH`:

```sh
magistr init . --agent codex --yes
./.codex/run
```

Setup adds the following artifacts:

| Path | Purpose |
| --- | --- |
| `.codex/config.toml` | Project MCP settings and skill selections |
| `AGENTS.md` | Managed `magistr:codex` instruction block |
| `.codex/run` | Shell entry point for the native launcher |
| `.agents/skills/*/SKILL.md` | Nine project skills |
| `.agents/skills/craft/references/runtime.md` | Workflow contract and Codex adapter |

The skills are `craft`, `init`, `next`, `next-loop`, `roadmap`,
`roadmap-refresh`, `signals`, `validate` and `workspace-routing`.
One `magistr` connection serves workflow and workspace operations. Cargo
installations use `magistr mcp --dir .`; npm installations use a pinned npx
invocation. Source tools are direct; `discover` and `execute` provide advanced
operations. Shared conventions live in `.magistr/conventions.md`.

Start a new trusted Codex session after setup. Inspect `context`, then discover
`ship_status` and `workspace_status` as needed. Concurrent connections attach to
the existing owners; each working copy keeps its own workspace journal.

For native `magistr next` and `magistr next-loop`, see
[agent settings](agent-settings.md). Those preferences select sessions Magistr
launches; they do not change the model in an existing Codex host session.

## Launcher

```sh
magistr codex-launch --project-root . --
magistr codex-launch --project-root . -- --model YOUR_MODEL
./.codex/run -C packages/api
```

The native entry point also works without a POSIX shell. Arguments after `--`
are forwarded literally, including spaces, metacharacters, and OS-supported
non-UTF-8 arguments. A forwarded `-C`/`--cd` changes the working directory;
relative paths resolve from the wrapper's project root. Configuration and skill
selection follow that directory's project layers. A forwarded `--` ends launcher
inspection, so prompt text is not parsed as flags.

On Unix the launcher replaces its process with Codex, preserving stdio, signals,
and exit status. Elsewhere it waits and returns Codex's exit code. It reads
`CODEX_HOME`, defaulting to the user's `.codex` directory, and does not edit
personal configuration, skills, or the trust database.

## Skill selection

Run `magistr codex-launch --project-root . --diagnose` to see the installed and
verified Codex versions. The launcher supplies process-local
`-c skills.config=...` overrides so `config/read` and `skills/list` observe the
same project selections. The installed-runtime acceptance check covers both
views, relocated projects, nested directories and the generated MCP connection.

Selections merge by skill path in this order:

1. `/etc/codex/config.toml` and `$CODEX_HOME/config.toml`.
2. The `$CODEX_HOME/<profile>.config.toml` selected by `-p`/`--profile`.
3. Suppression of personal skills whose metadata names match installed Magistr
   project skills. Discovery includes `$CODEX_HOME/skills` and `$HOME/.agents/skills`.
4. Project `.codex/config.toml` layers from repository root to working directory.
   Paths resolve relative to their config file; explicit choices can re-enable
   personal skills.
5. Forwarded Codex `-c` overrides.

Unrelated personal selections survive. Skill symlinks have cycle protection.
Missing config files are optional; malformed files fail with a diagnostic.
Other configuration and project-trust behavior belong to Codex itself.

At the baseline version, inline `[profiles.<name>]` and top-level `profile` are
legacy formats rejected with sidecar-migration guidance. The launcher does not
translate them. Profiles apply to Codex runtime commands and `codex mcp`;
`codex app-server` rejects `--profile`.

The override is applied on every launch. The diagnostic report marks versions
other than the baseline as unverified. The launcher handles local skill
discovery; remote discovery and experimental extra roots are not merged.

## Diagnostics

```sh
magistr codex-launch --project-root . --diagnose
magistr codex-launch --project-root . --diagnose -- -C packages/api
```

The JSON report names the CLI path/version, baseline match, working directory,
`CODEX_HOME`, loaded selection layers, and suppressed skill paths. It invokes
only `codex --version`: no App Server, MCP connection, thread, or model turn.
Authentication values, config contents, and forwarded prompts are not printed.

Default tests use fake executables. The opt-in runtime test uses installed Codex
and Workspace in disposable relocated projects for read-only RPCs and handshakes:

```sh
cargo test -p magistr-cli --test codex_runtime_live -- --ignored --nocapture
```

It does not invoke a model or acquire the real project's Workspace lock.

## Refresh and ownership

`magistr init . --agent codex --force --yes` refreshes eligible generated files.
It preserves explicit transports, unrelated TOML, comments, project state,
customized skills, and instructions outside the managed block. Malformed TOML
or instruction markers stop setup before writes.

Use `--dry-run --json` to inspect changes. A terminal-only `--dry-run` can enter
selective application; it cannot be combined with `--force` or `--yes`.
Legacy links from matching `.agents/skills/<name>` directories to this project's
`.claude/skills/<name>` migrate to Codex-specific files. Unrelated links and
unrecorded custom files are preserved. The preview lists companion replacements.

## Removal

```sh
magistr codex-remove .
```

Removal deletes recorded launcher/skill files only when unchanged. It restores
previous config values and managed instructions only where current values still
match the generated version. Customized and unrecorded content survives; shared
files and empty tables can remain. Project configuration and native workflow
state are retained.

`magistr codex-remove . --dry-run` prints the removal plan without writing:
every path with its action (remove, overwrite, preserved, refused), who owns
it and why it is kept when it is. `--json` prints the same plan as one
object. `magistr init --check` likewise prints the explicit repair plan the
explicit `magistr repair` applies, including the Codex target's repair.

`magistr reset` deletes local project configuration and workflow state, including
legacy state directories, after confirmation.

## Workflow

`$next` resumes an interrupted item or starts one ready item in the current
session. `$next-loop` carries a single-item handoff: one item completes, its
handoff is written, and the current session stops before another item is
selected. Invoke `$next-loop`
again only from a fresh Codex session; the bound defaults to ten iterations
when none is supplied. Native `magistr next-loop`
is the automatic host-owned loop; it starts a fresh provider session per
iteration. After an interruption, run the same command again: in-progress
work takes precedence over pending work. The matching objective, saved plan,
task statuses, actions, and check results are retained. The agent re-reads the
code and remaining tasks before continuing; it does not start the item again
or replace another session's objective.

When `roadmap_exports` is false, setup adds native-only instructions and disables
the export tool. Codex skills perform each workflow step directly. Reusable
workflow sources live in
`crates/magistr-cli/templates/workflows`, with small host adapters. Workspace's
routing skill has its own pinned canonical source.
