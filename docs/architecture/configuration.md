# Configuration ownership

Native workflow configuration is captured by
[NativeConfig](../../crates/magistr-engine/src/native/native_config.rs)
before a reader, writer or attached client selects its project.
`from_env` is the production capture boundary; `from_lookup` accepts injected
inputs for tests and explicit callers. Each requested environment key is read
at most once during resolution. No environment map is retained after parsing.

The captured object supplies the store, socket policy, sanitized project-name
override and typed think settings. Reader/writer admission and attachment use
that same identity input. Opening engines materializes think settings from the
captured object, the admitted project ID and native persistence. It does not
call the workflow compatibility loader again. The native publisher owns the
socket, so the think engine receives no broadcast path.

## Compatibility and precedence

Existing names and on-disk locations remain compatible. These are input
compatibility rules inside Magistr, not separate configuration authorities.

| Setting | Resolution |
| --- | --- |
| Project root | Explicit host input. Hosts should supply an absolute root. |
| Project identity | Captured, sanitized `THINK_AND_SHIP_PROJECT_NAME`, then the nearest `.think-and-ship/project.json`, then the canonical path-derived ID. An explicitly admitted project ID remains authoritative. |
| Store root | First nonblank value: `THINK_AND_SHIP_DATA_DIR`; `XDG_DATA_HOME/think-and-ship`; `HOME/.local/share/think-and-ship`; `APPDATA/think-and-ship`; `USERPROFILE/AppData/Roaming/think-and-ship`; then the captured platform temporary directory plus `think-and-ship`. |
| Native persistence | On by default. Trimmed `THINK_AND_SHIP_PERSIST=false` (case insensitive) or `0` disables it. |
| Publisher socket | Nonblank, trimmed `THINK_AND_SHIP_BROADCAST_PATH`, otherwise the existing project socket default when requested. |
| Short-socket directory | Trimmed `XDG_RUNTIME_DIR`, then `TMPDIR`, then `/tmp`. Existing Unix socket length limits and final short-path fallback remain. |
| Strict validation, no color, sessions | Existing switches require exact `true`. `THINK_AND_SHIP_AUTO_SESSION` remains a compatibility alias enabling sessions. |
| History, branch, timeout and rollup limits | Existing integer variables accept trimmed values of at least one. Invalid, zero and out-of-range values keep defaults. Rollup pinned-floor enforcement remains in its owning engine. |
| Think output format | `THINK_AND_SHIP_OUTPUT_FORMAT` accepts console, JSON or Markdown case insensitively. |
| Default think session | A valid explicit `THINK_AND_SHIP_DEFAULT_SESSION_ID` wins and enables sessions. Otherwise enabled sessions use the admitted project ID. Session namespacing remains the workflow engine's responsibility. |

The injected resolver never asks the host for a temporary directory. Its last
fallback is an injected `TMPDIR` on Unix (or `TMP`, `TEMP`, `USERPROFILE` on
Windows), then `/tmp/think-and-ship`. Production hosts use `from_env` so the
platform fallback, including macOS per-user temporary directories, is preserved.

## Provenance

`configuration_sources()` reports field paths mapped to `ConfigSource`:
built-in/platform default, named environment variable, explicit host override
or admitted project identity. It exposes source names without environment
values, credentials or rejected payloads. Warnings for rejected think settings
also omit the supplied values. Builder overrides update their field's source;
configuration fields are read through accessors outside the native composition
module.

The shared
[ThinkConfigResolution](../../crates/magistr-workflow/src/think/config_resolution.rs)
parser uses explicit persistence and injected input lookup. Its
`ThinkConfig::with_persistence` initializer is independent of the environment.
The legacy `load_config()` and `ThinkConfig::default()` remain compatibility
entry points for existing library callers; native engine composition uses the
explicit path.

## Guarantees and remaining boundaries

Capture is per key, not an atomic snapshot of the entire process environment.
Project identity files are read at admission; they are not frozen source
snapshots. After admission, the owner/reader retains the selected identity.
Relative paths retain filesystem-relative semantics, so explicit integrations
should pass absolute roots. Engine trace metadata remains its own concern.

This delivers the frozen native workflow portion of the project-configuration
refactor; the [project inputs](#project-inputs) below deliver the
project/preset/provider/plugin/workspace portion and the
[administrative change plan](#administrative-change-plan) the reviewable
setup, repair and removal plan. Application-time revalidation with
partial-outcome receipts, and effective configuration/policy digests in
verification applicability, remain separate roadmap work. This configuration
provenance is not a source digest, workflow revision, owner generation or
receipt. Protected workspace edits and human-owned command admission remain
enforced by their existing owners.

## Project inputs

[ProjectInputs](../../crates/magistr-core/src/config/inputs/project_inputs.rs)
is the composition-time capture for everything outside the native workflow
store: the per-user configuration directory and the files under it, the
provider environment, the plugin environment and the workspace's configuration
environment. `from_env` is the production boundary — `main` captures once at
startup and threads the set to every command; a command that takes `--dir`
re-roots the same capture with `for_project`. `AgentConfig::new` captures for
the session it composes, and a host that already holds the process capture
hands it over with `with_inputs`, so session resolution, runtime detection and
backend selection never consult the environment. The `magistr_init` MCP tool
captures when the tool is composed, and the `workbench-mcp` launcher captures
for the endpoint it serves. `from_lookup` accepts injected inputs for tests;
each key is asked for at most once, and blank values count as unset, as the
[XDG specification](https://specifications.freedesktop.org/basedir/latest/)
prescribes.

| Input | Resolution |
| --- | --- |
| User configuration directory | `XDG_CONFIG_HOME/magistr`, else `HOME/.config/magistr`, else unlocated. `config.toml`, `presets/`, `models.toml`, `plugins/` and `org-plugins.json` derive from it. |
| Project files | `<root>/.magistr/project.json` and `<root>/.magistr/presets`. |
| Provider | `MAGISTR_AGENT_BACKEND` (trimmed); `ANTHROPIC_API_KEY` recorded as present or absent only. |
| Plugin index | `MAGISTR_PLUGIN_INDEX_URL` overrides the index URL in `org-plugins.json`. |
| Workspace | The six keys the workspace's `ResolveOptions` reads (`WORKBENCH_ORG_POLICY`, `WORKBENCH_USER_CONFIG`, `ProgramData`, `XDG_CONFIG_HOME`, `HOME`, `USERPROFILE`), `WORKBENCH_NETWORK_FS` and `WORKBENCH_SESSION_ID`; the host builds `WorkspaceOpenOptions` from them. |

Loaders on the captured set return a `CapturedInput` whose `InputOrigin` says
whether the file was loaded, is absent (defaults apply) or could not be located
(no configuration directory); an existing file that cannot be read or parsed is
an error, never a default. `sources()` maps each setting to an `InputSource`
(default, named environment variable or explicit override) without values. The
preset pipeline's `LayerOptions::user_preset_dir` is the user override layer as
the capture resolved it: composition never reads the environment, so a
pipeline built without inputs composes built-in layers only.

Ambient reads outside these families — `MAGISTR_*` flags in `MagistrEnv`,
budget exports, cost and session storage paths, team identity, Codex home —
are not captured here; they are tracked as follow-up work.

Tests cover lookup isolation, exact compatibility parsing, default/builder
provenance, rejected diagnostic payloads, admitted session identity, real engine
opens and reader/writer identity precedence. They use injected inputs,
temporary projects/stores and child-process environments rather than mutating
the test runner's process environment. The
[Rust 2024 environment guidance](https://doc.rust-lang.org/stable/edition-guide/rust-2024/newly-unsafe-functions.html)
explains why process-wide mutation is unsuitable for concurrent tests.

## Administrative change plan

Project administration — setup and re-initialization, presets and import,
selective apply from a dry run, template plugins, startup repair and owned
Codex removal — is computed before it is written, as one
[ChangePlan](../../crates/magistr-cli/src/scaffold/change_plan.rs). The
plan is what the preview shows and what the writer executes: `magistr init
--dry-run` (plain, JSON and the interactive approval), the `magistr_init`
tool's `dry_run` mode, `repair::plan` and `CodexSetup::removal_plan` all
render it, and `generate_with_result`, `repair::repair` and
`CodexSetup::remove_target` execute the plan they just computed. Explicit
destructive reset (`magistr reset`, `remove_scaffold`) is deliberately not
planned here and stays a separate act.

Each [PlannedChange](../../crates/magistr-cli/src/scaffold/planned_change.rs)
names one path with:

| Field | Meaning |
| --- | --- |
| Expected state | `absent`, or the manifest content hash of the bytes read while planning (`expected()`); an application step revalidates against it. |
| Ownership | Judged from the file's update policy and the scaffold manifest, never from a new persisted layer: `unclaimed` (nothing there), `internal` (magistr internals every setup rewrites), `generated` (magistr wrote it, unchanged), `customized` (magistr wrote it, edited since), `user` (never overwritten, or present without a record), `shared` (a merged file such as `.mcp.json`, `.gitignore`, `workbench.toml`, `AGENTS.md`, `.codex/config.toml` or the think-and-ship section of `CLAUDE.md`), `canonical` (the target repository is the file's home). |
| Action and intent | `create`, `overwrite` or `remove` are applied; `unchanged` and `preserved` are skipped with a reason; `refused` carries why the plan will not act: a target that is not a regular file or cannot be read, a canonical input that is missing or unreadable, a missing runtime companion, or an unreadable plugin source. |

The setup plan covers everything generation writes: the regular scaffold
files under their policies, `.mcp.json` merged with the retired servers
stripped, the `.gitignore` section, the `workbench.toml` seed (computed from
the native command detection without writing), the retired rule files it
removes, the shared Codex components and the template plugins the captured
inputs name. `CLAUDE.md` stays the user's file, but its marked think-and-ship
section is magistr's, so the plan shows the merged document as an overwrite
with its diff rather than a preserved file that is then rewritten. A second
plan over a freshly generated project applies nothing.

Canonical inputs are read once per plan by
[CanonicalSource](../../crates/magistr-cli/src/scaffold/canonical.rs). A
skill's home repository (`skills/workbench-routing/SKILL.md` in workbench-mcp)
supplies the vendored copy's bytes. Magistr's own source checkout — one that
carries `crates/magistr-cli/templates/workflows` — supplies every workflow
skill, the shared runtime with its host adapter and the vendored routing skill
from the live templates, through the same rendering the embedded copies use;
an installed binary older than the checkout therefore follows each later edit
instead of downgrading the skills at `mcp-serve` startup. A canonical input
that is missing or unreadable refuses its paths; the embedded copy is never
written in its place. Ordinary projects carry no canonical inputs and keep
the embedded upgrades.

What this chunk does not do: execution still preflights the plan once and
writes change by change without locking out concurrent writers or
revalidating each target's expected state at write time, and generation
reports created/updated/unchanged/preserved/removed/refused rather than a
per-outcome receipt. Application with expected-state checks and partial
outcome receipts is `audit-260912-16-admin-apply`.
