# Codex integration

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

The skills are `craft`, `init`, `next`, `next-loop`, `roadmap`, `roadmap-refresh`,
`signals`, `validate`, and `workbench-routing`. Project MCP settings use the
`magistr` server, which embeds think-and-ship, and Workbench for workspace work.
The duplicate standalone `think-and-ship` server is disabled.

Both transports use portable `magistr mcp-launch` commands. The `workbench-mcp`
entry is the retained name of Magistr's native workspace bridge: the same
binary serves it, so no external executable, checkout or Bun runtime is
consulted, and a second Codex thread's launch attaches to the first thread's
workspace owner instead of being refused.

Start a new Codex session after setup. Project configuration depends on Codex's
project trust settings. Inspect `roadmap_status`, `ship_status`, and
`workbench_status` through existing connections. Every workspace launch reaches
the one admitted owner; a workspace whose `.workbench/server.lock` still holds
the retained legacy server's record needs `magistr workspace migrate` once,
with that server stopped.

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

The compatibility baseline is Codex CLI `0.153.4`. At that version, project
skill-disable entries appear in `config/read` but are not applied by `skills/list`.
The launcher supplies process-local `-c skills.config=...` overrides so both
views observe project selections.

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
and Workbench in disposable relocated projects for read-only RPCs and handshakes:

```sh
cargo test -p magistr-cli --test codex_runtime_live -- --ignored --nocapture
```

It does not invoke a model or acquire the real project's Workbench lock.

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

`magistr reset` deletes local project configuration and workflow state, including
legacy state directories, after confirmation.

## Workflow

`$next` completes one ready chunk in the current session. `$next-loop` carries a
single-chunk handoff: one chunk completes, its handoff is written, and the
current session stops before another chunk is selected. Invoke `$next-loop`
again only from a fresh Codex session; the bound defaults to ten iterations
when none is supplied. Native `magistr next-loop`
is the automatic host-owned loop; it starts a fresh provider session per
iteration. Skills recover existing work and preserve unrelated objectives.

When `roadmap_exports` is false, setup adds native-only instructions and disables
the export tool. Codex skills perform each workflow step directly. Reusable
workflow sources live in
`crates/magistr-cli/templates/workflows`, with small host adapters. Workbench's
routing skill has its own pinned canonical source.
