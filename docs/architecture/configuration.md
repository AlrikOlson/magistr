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
call the workflow's ambient loader again. The native publisher owns the
socket, so the think engine receives no broadcast path.

## Current project settings

Core's `config::read_project_settings` reads the full current
`ProjectConfig` at the path resolved by `paths::project_json`. Linked Git
working copies use the main project's declaration, including its backend,
commands and export policy. Validation, captured project inputs, CLI settings
views and setup preflight use that reader. Reading never runs stack detection
or writes settings; setup is responsible for detecting and declaring the stack.

An explicit `roadmap_backend` selects `native` or `yaml`. An omitted
field, or an absent settings file when only a backend is requested, selects
`native`. Other files and `spec.file` do not select a backend.
Commands that require configured projects still refuse missing settings.
Existing settings must satisfy the current typed contract: malformed JSON,
invalid values, unreadable files and invalid Git membership refuse with the
settings path or link error. Setup checks these before writing scaffold files.

Configuration and roadmap format migrations are absent. YAML reads use
`.magistr-roadmap.yml`; creation starts an empty roadmap instead of converting
surrounding files. Setup generates project and host configuration through its
administrative plan; it does not author a roadmap through an extra file write.
The default project template selects the native roadmap and omits a spec file.

## Current input precedence

These names and on-disk locations describe the current resolver. The
[pre-release policy](../../AGENTS.md#before-public-release) permits replacing them directly and
updating current callers; old settings and formats impose no preservation or
migration requirement.

| Setting | Resolution |
| --- | --- |
| Project root | Explicit host input. Hosts should supply an absolute root. |
| Execution | Captured `MAGISTR_EXECUTION_ID`, default `default`, or an explicit `with_execution_id` input. Ship state and receipts are partitioned by canonical source root plus execution id. |
| Project identity | Linked Git worktrees resolve the corresponding main project directory. Captured, sanitized `MAGISTR_PROJECT_ID`, then `project.id` in the nearest `.magistr/project.json`, then the canonical path-derived ID. An explicitly admitted project ID remains authoritative. |
| Store root | First nonblank value: `MAGISTR_WORKFLOW_DATA_DIR`; `XDG_DATA_HOME/magistr/workflow`; `HOME/.local/share/magistr/workflow`; `APPDATA/magistr/workflow`; `USERPROFILE/AppData/Roaming/magistr/workflow`; then the captured platform temporary directory plus `magistr/workflow`. |
| Native persistence | On by default. Trimmed `MAGISTR_WORKFLOW_PERSIST=false` (case insensitive) or `0` disables it. |
| Publisher socket | Nonblank, trimmed `MAGISTR_WORKFLOW_BROADCAST_PATH`, otherwise the existing project socket default when requested. |
| Short-socket directory | Trimmed `XDG_RUNTIME_DIR`, then `TMPDIR`, then `/tmp`. Existing Unix socket length limits and final short-path fallback remain. |
| Strict validation, no color, sessions | Existing switches require exact `true`. `MAGISTR_WORKFLOW_AUTO_SESSION` currently also enables sessions; its old name imposes no retention requirement. |
| History, branch, timeout and rollup limits | Existing integer variables accept trimmed values of at least one. Invalid, zero and out-of-range values keep defaults. Rollup pinned-floor enforcement remains in its owning engine. |
| Think output format | `MAGISTR_WORKFLOW_OUTPUT_FORMAT` accepts console, JSON or Markdown case insensitively. |
| Default think session | A valid explicit `MAGISTR_WORKFLOW_DEFAULT_SESSION_ID` wins and enables sessions. Otherwise enabled sessions use the admitted project ID. Session namespacing remains the workflow engine's responsibility. |

The injected resolver never asks the host for a temporary directory. Its last
fallback is an injected `TMPDIR` on Unix (or `TMP`, `TEMP`, `USERPROFILE` on
Windows), then `/tmp/magistr`. Production hosts use `from_env` so the
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
`load_config()` and `ThinkConfig::default()` currently read ambient inputs;
native engine composition uses the explicit path. These entry points may be
removed with their callers; they are not protected interfaces.

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
setup, repair and removal plan and its
[application](#application-and-receipts) with revalidation and per-change
receipts, and the [configuration evidence](#configuration-evidence) below
binds the effective external policy to verification applicability. This
configuration provenance is not a source digest, workflow revision, owner
generation or receipt. Protected workspace edits and human-owned command
admission remain enforced by their existing owners.

## Configuration evidence

A check's receipt certifies its inputs only while every identity it was
taken under still applies. The external configuration is one of them: the
workspace's organization policy and user layer (the paths
`ResolveOptions::external_layer_paths` resolves from the captured
[workspace inputs](#project-inputs) — `MAGISTR_WORKSPACE_ORG_POLICY`,
`MAGISTR_WORKSPACE_USER_CONFIG` or the platform defaults) govern command admission
and policy without touching a byte inside the tree. `WorkspaceEvidence` also
observes the shared project's `.magistr/project.json` as workspace configuration,
resolved through `ProjectLocation` by the central settings path owner. A linked
copy uses the main project's settings, including its gate commands and export
policy. Settings readers, hook writes and setup use that same path. Invalid Git
membership refuses resolution. `.magistr/workspace.toml` remains specific to each source
copy. The snapshot also selects either configuration file when it is inside
that copy.

[ConfigDigest](../../crates/magistr-workspace/src/config/config_digest.rs)
observes those layers lock-free: per layer, whether a file stood at its
resolved path and the SHA-256 of its bytes (absent and empty are different
observations; a layer that exists but cannot be read refuses the
observation), combined length-framed into one digest. Neither a path nor a
byte of content is recorded — two hosts resolving the same policy share the
digest, and nothing a layer says or where it lives enters a receipt.
`WorkspaceEvidence` (built by `writer_config` / `writer_config_from_env`
from the captured `ProjectInputs`) takes the digest beside every
`SourceSnapshot`, so the `SourceObservation` a receipt records carries the
identities below side by side; `SnapshotWitness::validate` re-reads the
source first and then the same layers, answering `Reconfigured` with the
layer names when a layer moved while the bytes did not.

| Identity | Owner and meaning | Moves when | Judged at completion |
| --- | --- | --- | --- |
| Source revision (`SourceObservation::revision`) | The workspace snapshot's name for an observation: scheme, canonical root, owner generation, selection-policy digest and manifest digest. | Any of its parts moves, including a new owner of the same bytes. | Recorded and displayed, not compared: its parts are judged one by one. |
| Source digest (`source_digest`) | The selected bytes under the policy (the manifest digest). | A selected file is added, removed or modified — `.magistr/project.json` and `.magistr/workspace.toml` included. | Yes — "the source changed since the gate ran". |
| Selection policy (`policy_digest`) | What the snapshot selects: version, ceiling, explicit inputs. | The policy is changed. | Yes — "the selection policy changed". |
| Configuration digest (`config_digest`) | Applicable policy layers and shared project settings by content. | Organization policy, user configuration or shared project settings appear, disappear or change. | Yes — the applicable configuration changed. |
| Acceptance revision (`CheckReceipt::acceptance_revision`) | The objective's description, acceptance criteria, constraints and scope. | An in-flight `ship_set_objective`. | Yes — "the objective's acceptance changed". |
| Owner generation (`workspace_generation`) | Which authority incarnation was durable ([workspace owner](workspace-owner.md)). | Every admission of a workspace owner; never a write. | No — a new owner of the same bytes is not a change of source. |
| Workflow store stamp (`StoreStamp`) | Where a family's store file stands on disk (inode, mtime, length), a cache-freshness hint. | Any writer lands a store file. | No — it orders nothing and names no content. |
| Applied mutation revision (`MutationReceipt::revision`) | What one journaled workspace mutation did to the files it touched. | Only for an applied mutation; indeterminate otherwise. | No — it names a mutation's effect, not the tree's state. |

`SourceObservation::drift_to` judges a later observation in that order —
workspace, scheme, selection policy, source, configuration — and names the
first identity that differs as a typed `ObservationDrift`; a receipt
recorded before a digest was bound is `Unbound` and refused by name. The
completion refusal prints the drift verbatim beside the gate, the
cross-process settlement path (no witness in the settling process) maps it
to `Unchanged`, `Diverged` (source digests) or `Reconfigured` (layers
unknown), and the latest attempt of each gate still governs: re-running the
gate under the current configuration completes.

Limits: the digest covers the workspace's two external layers; Magistr's
own per-user `config.toml`, presets, models and plugin settings do not
change what a gate certifies and are not bound. The effective layer is
digested by bytes, so a comment-only edit moves the evidence.

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
captures when the tool is composed, and the unified MCP launcher captures
for the endpoint it serves. `from_lookup` accepts injected inputs for tests;
each key is asked for at most once, and blank values count as unset, as the
[XDG specification](https://specifications.freedesktop.org/basedir/latest/)
prescribes.

| Input | Resolution |
| --- | --- |
| User configuration directory | `XDG_CONFIG_HOME/magistr`, else `HOME/.config/magistr`, else unlocated. `config.toml`, `presets/`, `models.toml`, `plugins/` and `org-plugins.json` derive from it. |
| Project settings | `.magistr/project.json` under the corresponding main project directory; linked copies share its reader and writer path. |
| Project presets | `<source-root>/.magistr/presets`, versioned with that source copy. |
| Provider | `MAGISTR_AGENT_BACKEND` (trimmed); `ANTHROPIC_API_KEY` recorded as present or absent only. |
| Plugin index | `MAGISTR_PLUGIN_INDEX_URL` overrides the index URL in `org-plugins.json`. |
| Workspace | The six keys the workspace's `ResolveOptions` reads (`MAGISTR_WORKSPACE_ORG_POLICY`, `MAGISTR_WORKSPACE_USER_CONFIG`, `ProgramData`, `XDG_CONFIG_HOME`, `HOME`, `USERPROFILE`), `MAGISTR_WORKSPACE_NETWORK_FS` and `MAGISTR_WORKSPACE_SESSION_ID`; the host builds `WorkspaceOpenOptions` from them. |

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

Tests cover lookup isolation, current input parsing, default/builder
provenance, rejected diagnostic payloads, admitted session identity, real engine
opens and reader/writer identity precedence. They use injected inputs,
temporary projects/stores and child-process environments rather than mutating
the test runner's process environment. The
[Rust 2024 environment guidance](https://doc.rust-lang.org/stable/edition-guide/rust-2024/newly-unsafe-functions.html)
explains why process-wide mutation is unsuitable for concurrent tests.

## Personal appearance settings

Desktop appearance uses the user's captured `config.toml` location. The same
[UserConfigDocument](../../crates/magistr-core/src/config/user_config_document.rs)
reads settings for `ProjectInputs`, saves desktop choices and saves personal
agent preferences from `magistr config set`. A read creates no directories or
files. A missing file uses defaults; unreadable files, directories in the file's
place and invalid TOML produce an error.

```toml
[theme]
default = "system"

[accessibility]
reduce_motion = false
```

`theme.default` accepts `system`, `light` or `dark`. The default is `system`,
which follows the operating system's appearance. Unsupported values are errors.
`accessibility.reduce_motion` defaults to `false`. Reduced motion is enabled
when any of these requests it: the explicit `--reduce-motion` flag, the saved
preference, or the operating system. Saving `false` cannot cancel an active
command-line or operating-system request.

[DesktopPreferencesService](../../crates/magistr-cli/src/application/desktop_preferences.rs)
captures the settings path from `ProjectInputs` and performs filesystem work on
a blocking worker. Each successful load or save returns the accepted choices
and a new service revision. A save must name the displayed revision; rejected
writes retain the previous accepted state, so the interface can keep the user's
edits available for retry.

The configuration writer preserves TOML comments and unrelated settings.
All personal preference writes share a stable sibling file lock. While holding
that lock, the writer checks the exact file bytes against the observed document,
validates the updated document and replaces the file through the core
[durable writer](../../crates/magistr-core/src/durable_write.rs), retaining the
existing file permissions. A changed file
requires a fresh load before saving; a busy writer or failed write reports an
error. The operating system releases the lock when the writer exits, and the
lock file remains in place for later writers.

## Administrative change plan

Project administration — setup, re-initialization, preset import, template
plugins, explicit repair and integration removal — is computed before writing,
as one [ChangePlan](../../crates/magistr-cli/src/scaffold/change_plan.rs).
`magistr init --dry-run`, `magistr repair --dry-run` and
`magistr remove --dry-run` show the corresponding plan. The guided interview
applies exactly its reviewed PreparedSetup through AdminService; it does not
recompute the plan after approval. MCP startup never repairs project files.
Every rendering contains one
object per path with its action, intent, ownership, expected state and
reason, plus the per-intent summary; the CLI adds an overwrite's diff and a
tool answer omits it. `generate_with_result`, `apply_approved`,
`PreparedSetup::apply` and `integrations::apply_removal` execute the plan they just
computed through the one
[administrative service](#application-and-receipts). Explicit destructive
reset (`magistr reset`, `remove_scaffold`) is deliberately not planned here
and stays a separate act.

Each [PlannedChange](../../crates/magistr-cli/src/scaffold/planned_change.rs)
names one path with:

| Field | Meaning |
| --- | --- |
| Expected state | `absent`, or the manifest content hash of the bytes read while planning (`expected()`); an application step revalidates against it. |
| Ownership | Judged from the file's update policy and the scaffold manifest, never from a new persisted layer: `unclaimed` (nothing there), `internal` (magistr internals every setup rewrites), `generated` (magistr wrote it, unchanged), `customized` (magistr wrote it, edited since), `user` (never overwritten, or present without a record), `shared` (a merged file such as `.mcp.json`, `.gitignore`, `.magistr/workspace.toml`, `AGENTS.md`, `.codex/config.toml` or the magistr section of `CLAUDE.md`), `canonical` (the target repository is the file's home). |
| Action and intent | `create`, `overwrite` or `remove` are applied; `unchanged` and `preserved` are skipped with a reason; `refused` carries why the plan will not act: a target that is not a regular file or cannot be read, a canonical input that is missing or unreadable, a missing runtime companion, or an unreadable plugin source. |

The setup plan covers everything generation writes: the regular scaffold
files under their policies, `.mcp.json` merged with only Magistr's entry updated, the `.gitignore` section, the `.magistr/workspace.toml` seed (computed from
the native command detection without writing), the retired rule files it
removes, the shared Codex components, the template plugins the captured
inputs name, and the seeds of a first init. A `.magistr/project.json` the
plan creates already carries the lifecycle hooks the interview or the
language preset chose and, for a polyglot tree, its archetypes, topology and
relationships ([SetupSeeds](../../crates/magistr-cli/src/scaffold/setup_seeds.rs),
built by the pipeline and handed to the plan); an existing configuration is
preserved as the user's and re-detection never rewrites it. `CLAUDE.md` stays
the user's file, but its marked sections — magistr, and the archetype
table planted beside a fresh configuration — are magistr's, so the plan shows
the merged document as an overwrite with its diff rather than a preserved
file that is then rewritten. First setup includes the resolved identity as
`project.id` in that same project configuration. Workflow admission reads it;
there is no separate identity file or runtime identity writer. Existing project
settings remain owned by the user.
The pipeline writes nothing after generation, so a first-init preview is
complete and a second plan over a freshly generated project applies nothing.

`magistr init --ci` plans its one file the same way
([ci.rs](../../crates/magistr-cli/src/scaffold/ci.rs)): the rendered
`.github/workflows/quality-gates.yml` is a create when nothing is there, the
user's preserved file when something is and `--force` is not given, an
overwrite with its diff (or unchanged) under `--force`, and refused when the
target is not a readable regular file. The plan is applied through the
administrative service and the command answers its receipt, so a workflow
that appeared or changed between planning and the write is reported failed
and left as found. The file is never among the scaffold's ownership records:
a setup neither repairs nor removes it.

Canonical inputs are read once per plan by
[CanonicalSource](../../crates/magistr-cli/src/scaffold/canonical.rs). The
Magistr source checkout — one that
carries `crates/magistr-cli/templates/workflows` — supplies every workflow
skill, the shared runtime with its host adapter and the workspace routing skill
from the live templates, through the same rendering the embedded copies use;
an installed binary older than the checkout therefore follows each later edit
instead of downgrading the skills during explicit repair. A canonical input
that is missing or unreadable refuses its paths; the embedded copy is never
written in its place. Ordinary projects carry no canonical inputs and keep
the embedded upgrades.

### Application and receipts

Every plan is applied by
[AdminService](../../crates/magistr-cli/src/scaffold/admin_service.rs), one
change at a time, and answers an
[ApplyReceipt](../../crates/magistr-cli/src/scaffold/apply_receipt.rs). An
administrative write is therefore always an explicit act for a named
operation (setup, repair or removal) on a plan that was computed first; an
agent session's source writes never reach it, because they take the
admitted workspace's [source route](clients.md) or are refused, and the
`native-write-path` gate (rule 13) keeps every other file under the scaffold
module from writing the filesystem at all. Human-owned `.magistr/workspace.toml`
`[commands]` tables, source policy, the `.magistr` state
namespaces and the roadmap export denials are outside every plan and stay
untouched.

Immediately before each write the service revalidates the target against
the plan's expected state: a create opens the file with `create_new`, so a
file that appeared since planning fails rather than being overwritten; an
overwrite re-reads the target, writes a temporary file beside it and renames
it into place, keeping the target's mode (or 0755 by the executable path
rule), so a write that fails part-way leaves no half-written file; a removal
re-reads the target first. A path a legacy Codex skill link covers is
checked to hold nothing once the link is detached, since the Claude source
the plan read it through may have been rewritten by the same plan.

Each planned change gets one outcome — `applied`, `skipped` with the plan's
reason, `refused` with the plan's reason, or `failed` with its cause: the
target changed after planning (expected and found state), the target could
no longer be revalidated (not a regular file, unreadable), or the write
itself failed — and a failed change is left as found while the remaining
changes are still applied. Ownership records are saved after every change
that needs one; a save that fails is the receipt's manifest outcome, reported
beside the file outcomes rather than lost. Nothing claims the batch was
atomic: `generate`, the interactive approval and Codex removal answer a
partial receipt as an error that names what failed, what the manifest did
and what was applied before that; `generate_with_result` carries the same in
its `failed` and `manifest` fields; explicit repair reports partial failures. Selective apply puts every change the plan applies — a create, an
overwrite or a removal such as a retired rule file — to the reviewer the same
way, checks the approved batch as a whole before the first write (a stale
selected target, including a retired file edited or already gone since the
preview, sends the reviewer back to a fresh preview rather than applying part
of what they approved) and then applies through the same service; a removal
the reviewer skipped stays in place.
