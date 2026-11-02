# Vocabulary

1 November 2026. This page owns every public word in Magistr. The desktop, MCP
tools and their arguments, the CLI, generated skills and agent instructions,
docs, the website, saved records and Rust identifiers all use the words here.
The [vocabulary cutover](#cutover) applies the whole page in one change. After
that change, an old word is a defect wherever it appears, not an alias.

The words come from the [fleet design](../design/fleet-scale-roadmap.md). The
owner decided the storage reach and the collision names on 1 November 2026.

## Terms

| Internal term today | Public word | Meaning |
|---|---|---|
| roadmap | **plan** | Every task of a project, with dependencies, priority and status. One plan per project; the company plan is the plans of every project together. |
| chunk | **task** | One unit of plan work with a definition of done (its acceptance). |
| group, track, workstream | **feature** | A named set of tasks that people approve, limit and fund together. |
| lane | **team** | A set of agents and people bound to one or more features. |
| lease, claim | **checkout** | A holder's exclusive, expiring right to work a task, with a checkout number. |
| signal | **request** | Something a person raised: a question, suggestion, concern, bug or feedback. |
| think trace | **decision log** | The recorded reasoning for a project: notes, threads and their links. |
| think step | **note** | One numbered entry in the decision log. |
| think branch | **thread** | A side question in the decision log, opened from a note and merged or abandoned. |
| think session | **log** | One saved sequence of notes; a project can keep several. |
| ship trace, execution trace | **run history** | Every run of a project with its steps, progress, checks and results. |
| ship objective | **run** | One attempt at a goal (usually one task), with its steps and verification. |
| objective description | **goal** | What a run sets out to do. |
| ship task | **step** | One planned unit inside a run. |
| ship action | **progress** | A record of what was done during a step. |
| quality gate, gate (`magistr_gate`) | **check** | A configured command whose passing result is required. |
| receipt | **check result** | The recorded outcome of one check: command, exit code, time, inputs. |
| approval gate (`ship_gate_*`) | **approval** | A question that needs a person's yes before work continues. |
| backref, cross-ref | **reference** | A typed link between records, such as `task:x` or `decision:1805`. |
| blocked, blocker | **waiting** | A task or step that cannot move until something named happens. |
| backlog | **ideas** | Tasks nobody has prioritized yet. |
| obsoleted | **retired** | Overtaken by events, kept for history, never worked. |
| promote (request to chunk) | **promote** | Unchanged: a request becomes an idea in the plan. |
| focus | **focus** | Unchanged: the feature and mode a team is working in. |
| phase | **phase** | Unchanged: research, plan, execute, verify, done. |
| priority, band | **priority** | Unchanged. A smaller number is more urgent. |
| (new) | **responsible person** | The person who answers for a task while an agent works it: by default the approver of its feature. Questions, stalled work and the done packet go to them. |
| (new) | **done packet** | A finished task's result: each definition-of-done item tied to its proof, the change and its requests, approvals, and what is still not verified. "Checked by machines" and "approved by a person" are separate marks. |
| (new) | **code location** | Where part of a project's code lives: a code host, a repository, a default branch and a folder. A project has one or more; several projects may share a repository. |
| (new) | **code host** | The service that hosts a repository, such as GitHub, GitLab, Bitbucket, Gitea, Forgejo or Azure DevOps. Magistr reaches every code host through one port. |
| (new) | **pull request**, **merge request** | A proposed change on a code host, named the way that host names it: merge request on GitLab, pull request everywhere else. "Request" alone means a [request](#terms) someone raised. |

### Collisions this table settles

- **Task and step.** A plan holds tasks, and a run holds steps. Before the
  cutover, `task` meant a run step and `chunk` meant a plan task. The reference
  prefix `task:` therefore changes meaning, so the storage upgrade rewrites
  `task:` to `step:` before it rewrites `chunk:` to `task:`.
- **Plan and planning steps.** `plan_work` edits a run's steps, so it becomes
  `arrange_steps`. Every `plan_*` name refers to the plan only.
- **Run.** A run is an attempt at a goal. The workspace tool `run`, which runs a
  declared command, becomes `command`. The CLI `magistr run`, which starts an
  agent session, becomes `magistr start`. The loop flag `--run` becomes `--loop`.
  `start` was chosen over `session` because `magistr sessions` already exists
  one letter away.
- **Team.** A team is a group of agents and people. The people roster command
  `magistr team` becomes `magistr people`, and `.magistr/team.json` becomes
  `.magistr/people.json`.
- **Note and step.** Decision-log entries are notes, so "step" means only a run
  step. `step_number` on the decision log becomes `note`.
- **Thread.** In the decision log a thread is a side question. Integration copy
  for Slack says "reply in Slack", never "thread", so the two do not meet.
- **Ideas and suggestion.** The ideas list holds unprioritized tasks, so the
  request kind `idea` becomes `suggestion`.
- **Check.** `check` is the configured command, and `check_step` records a
  check result on a step. "Gate" is not a public word.

## States

The legal moves do not change. Only names change. `TaskStatus::allows` is
today's `ChunkStatus::allows`.

### Task

| Stored today | Stored after | Shown as |
|---|---|---|
| `backlog` | `idea` | Ideas |
| `pending` | `to_do` | To do |
| `in_progress` | `in_progress` | In progress |
| `blocked` | `waiting` | Waiting on … (the reason below) |
| `done` | `done` | Done |
| `obsoleted` | `retired` | Retired |

A waiting task always has one reason. The field `blocked_by` becomes
`waiting_on`.

| Stored today (`blocked_by`) | Stored after (`waiting_on`) | Shown as |
|---|---|---|
| `awaiting_human` | `person` | Waiting on a person |
| `external` | `outside` | Waiting on something outside the project |
| `premise_unmet` | `condition` | Waiting until *condition* |
| `premise_refuted` | `disproved` | Disproved, needs rethinking |

"Waiting on another team" is derived, not stored. It applies to a to-do task
that has a dependency on another project's task that is not done yet. "Ready"
stays derived: to do, no waiting reason, every dependency done, its feature's
plan approved, and its feature under its limit.

### Step, run, request and thread

| Record | Stored today → after | Shown as |
|---|---|---|
| Step | `planned`, `active`, `blocked` → `waiting`, `completed` → `done`, `skipped` | Planned, Active, Waiting, Done, Skipped |
| Run | `defined` → `open`, `active`, `completed` → `finished`, `abandoned` | Open, Active, Finished, Abandoned |
| Request | `new`, `triaged`, `researched`, `surfaced` → `raised`, `promoted`, `dismissed` | New, Triaged, Researched, Raised, Promoted, Dismissed |
| Request kind | `question`, `idea` → `suggestion`, `concern`, `bug`, `feedback` | Question, Suggestion, Concern, Bug, Feedback |
| Thread | `active` → `open`, `merged`, `abandoned` | Open, Merged, Abandoned |
| Focus mode | `shape`, `build`, `listen` | Shape, Build, Listen |

## References

| Prefix today | Prefix after | Points at |
|---|---|---|
| `chunk:<id>` | `task:<id>` | A plan task |
| `task:<id>` | `step:<id>` | A run step |
| `think:<n>` | `decision:<n>` | A decision-log note |
| `action:<n>` | `progress:<n>` | A progress record |
| `check:<name>` | `check:<name>` | A check result (unchanged) |
| `signal:<id>` | `request:<id>` | A request |

Record fields follow the same table: `cross_refs` becomes `refs`, `ship_ref`
becomes `step_ref`, `execution_ref` becomes `run_ref`, `think_step` becomes
`note`, and `group` becomes `feature`.

## MCP tools

Every operation the `magistr` MCP connection serves, as of 1.0.0-beta.2.
Direct tools are marked with ★. Argument names follow the terms and reference
tables above. For example, `task_id` on a step tool becomes `step_id`, and
`branch_from` / `branch_id` / `branch_name` become `thread_from` /
`thread_id` / `thread_name`.

### Run and source (direct)

| Today | After |
|---|---|
| `context` ★ | `context` |
| `search` ★ | `search` |
| `read` ★ | `read` |
| `edit` ★ | `edit` |
| `write` ★ | `write` |
| `plan_work` ★ | `arrange_steps` |
| `start_work` ★ | `start_step` |
| `record_progress` ★ | `record_progress` |
| `check_work` ★ | `check_step` |
| `finish_work` ★ | `finish_step` |

### Plan

| Today | After |
|---|---|
| `roadmap_status` | `plan_status` |
| `roadmap_next` | `plan_next` |
| `roadmap_get` | `plan_get` |
| `roadmap_export` | `plan_export` |
| `roadmap_add_chunk` | `plan_add_task` |
| `roadmap_update_chunk` | `plan_update_task` |
| `roadmap_start_chunk` | `plan_start_task` |
| `roadmap_complete_chunk` | `plan_complete_task` |
| `roadmap_obsolete_chunk` | `plan_retire_task` |
| `roadmap_set_status` | `plan_set_status` |
| `roadmap_set_group` | `plan_set_feature` |
| `roadmap_propose_groups` | `plan_propose_features` |
| `roadmap_link` | `plan_link` |
| `roadmap_reprioritize` | `plan_reprioritize` |
| `roadmap_record_refresh` | `plan_record_refresh` |
| `roadmap_focus_get` | `plan_focus_get` |
| `roadmap_focus_set` | `plan_focus_set` |

### Run

| Today | After |
|---|---|
| `ship_set_objective` | `run_open` |
| `ship_status` | `run_status` |
| `ship_block` | `run_block_step` |
| `ship_finalize` | `run_finish` |
| `ship_export` | `run_export` |
| `ship_reset` | `run_reset` |
| `ship_compact_receipts` | `run_compact_checks` |
| `ship_gate_open` | `approval_open` |
| `ship_gate_wait` | `approval_wait` |

### Decision log

| Today | After |
|---|---|
| `think_record_step` | `decision_record` |
| `think_get_step` | `decision_get` |
| `think_search_trace` | `decision_search` |
| `think_pin_step` | `decision_pin` |
| `think_revise_estimate` | `decision_revise_estimate` |
| `think_set_branch_status` | `decision_set_thread_status` |
| `think_step_impact` | `decision_impact` |
| `think_trace_checkpoint` | `decision_review` |
| `think_session` | `decision_log` |
| `think_sessions` | `decision_logs` |
| `think_export_trace` | `decision_export` |
| `think_engine_status` | `decision_status` |
| `think_wipe_trace` | `decision_wipe` |

### Requests

| Today | After |
|---|---|
| `signal_capture` | `request_capture` |
| `signal_get` | `request_get` |
| `signal_status` | `request_status` |
| `signal_research` | `request_research` |
| `signal_pending` | `request_ready` |
| `signal_surface` | `request_raise` |
| `signal_snooze` | `request_snooze` |
| `signal_ignore` | `request_dismiss` |
| `signal_link` | `request_link` |
| `signal_promote` | `request_promote` |

### Project, workspace and trackers

| Today | After |
|---|---|
| `magistr_gate` | `magistr_check` |
| `magistr_format` | `magistr_format` |
| `magistr_set_phase` | `magistr_set_phase` |
| `magistr_ask` | `magistr_ask` |
| `magistr_init` | `magistr_init` |
| `magistr_recommend` | `magistr_recommend` |
| `magistr_hooks_add` | `magistr_hooks_add` |
| `magistr_hooks_list` | `magistr_hooks_list` |
| `magistr_hooks_remove` | `magistr_hooks_remove` |
| `run` | `command` |
| `files` | `files` |
| `map` | `map` |
| `script` | `script` |
| `undo` | `undo` |
| `workspace_status` | `workspace_status` |
| `tracker_setup` | `tracker_setup` |
| `tracker_status` | `tracker_status` |

No two names after the cutover are equal. The prefixes `plan_`, `run_`,
`decision_`, `request_`, `approval_`, `magistr_`, `tracker_` and `workspace_`
each belong to one family. The direct verbs (`arrange_steps`, `start_step`,
`record_progress`, `check_step`, `finish_step`) act on steps only.

## CLI

Every command path in 1.0.0-beta.2. The global options `--json`, `--budget`,
`--budget-warn`, `--trust`, `--verbose`, `--help` and `--version` do not change.
In the flags column, "same" means the command keeps every flag it has today.

| Today | After | Flags |
|---|---|---|
| `magistr init` | `magistr init` | same |
| `magistr detect` | `magistr detect` | same |
| `magistr repair` | `magistr repair` | same |
| `magistr remove` | `magistr remove` | same |
| `magistr reset` | `magistr reset` | same |
| `magistr validate` | `magistr validate` | same; help says "checks", not "quality gates" |
| `magistr run` | `magistr start` | same |
| `magistr next` | `magistr next` | same; help says "the next ready task" |
| `magistr next-loop` | `magistr next-loop` | `--track` → `--feature`; others same |
| `magistr resolve-dispatch` | `magistr resolve-dispatch` | `--run` → `--loop`; others same |
| `magistr completions` | `magistr completions` | same |
| `magistr man-pages` | `magistr man-pages` | same |
| `magistr config` (`set`, `check`, `show`) | `magistr config` (`set`, `check`, `show`) | same |
| `magistr lint` | `magistr lint` | same |
| `magistr archetypes` | `magistr archetypes` | same |
| `magistr doctor` | `magistr doctor` | same |
| `magistr guard` | `magistr guard` | same; reads run and plan state |
| `magistr plans` | `magistr runs` | same |
| `magistr roadmap` | `magistr plan` | — |
| `magistr roadmap show` | `magistr plan show` | same |
| `magistr roadmap generate` | `magistr plan generate` | same |
| `magistr roadmap next` | `magistr plan next` | same |
| `magistr roadmap status` | `magistr plan status` | same |
| `magistr roadmap export` | `magistr plan export` | same; writes `PLAN.md` / `PLAN.json` |
| `magistr export` | `magistr export` | same |
| `magistr costs` | `magistr costs` | same |
| `magistr import` | `magistr import` | same |
| `magistr sessions` | `magistr sessions` | same |
| `magistr login` | `magistr login` | same |
| `magistr logout` | `magistr logout` | same |
| `magistr team` | `magistr people` | — |
| `magistr team show` | `magistr people show` | same |
| `magistr team init` | `magistr people init` | same |
| `magistr team add` | `magistr people add` | same |
| `magistr team remove` | `magistr people remove` | same |
| `magistr team set-role` | `magistr people set-role` | same |
| `magistr policy` (`show`, `check`, `report`) | `magistr policy` (`show`, `check`, `report`) | same |
| `magistr permissions` | `magistr permissions` | same |
| `magistr models` | `magistr models` | same |
| `magistr learn` | `magistr learn` | same |
| `magistr plugins` (`list`, `add`, `remove`, `install`, `search`, `validate`) | `magistr plugins` (same subcommands) | same |
| `magistr preset` (`list`, `create`, `test`) | `magistr preset` (same subcommands) | same |
| `magistr mcp` | `magistr mcp` | same |
| `magistr codex-launch` | `magistr codex-launch` | same |
| `magistr codex-remove` | `magistr codex-remove` | same |
| `magistr memory` (`show`, `add`, `reset`) | `magistr memory` (same subcommands) | same |
| `magistr workspace` | `magistr workspace` | — |
| `magistr workspace working-copy` (`list`, `create`, `open`, `archive`) | `magistr workspace working-copy` (same subcommands) | same |
| `magistr workspace doctor` (`bundle`, `config`) | `magistr workspace doctor` (same subcommands) | same |
| `magistr workspace journal` | `magistr workspace journal` | same |
| `magistr workspace key` (`rotate`) | `magistr workspace key` (`rotate`) | same |
| `magistr workspace init` | `magistr workspace init` | same |
| `magistr workspace snapshot` | `magistr workspace snapshot` | same |

## Skills, configuration and generated files

The owner chose the skill set on 2 November 2026. The
[skills design](../design/skills.md) gives each skill's job and what it replaces.

| Today | After |
|---|---|
| Skills `roadmap`, `roadmap-refresh` | `plan`: status, research and shaping, never building |
| Skill `signals` | `requests` |
| User skills `switch-work`, `advance-work` | `focus` sets the feature and mode; `next` does the one unit |
| User skill `handoff` | Part of `next-loop` |
| User skills `roadmap-run`, `roadmap-sk`, `roadmap-refresh-sk`, `roadmap-run-sk`, `business-intel` | Retired |
| Skills `next`, `next-loop`, `craft`, `validate`, `init`, `workspace-routing` | Names unchanged; their text uses this page's words |
| `craft/references/runtime.md` | `craft/contract.md`, the one workflow contract |
| `.claude/rules/workflow.md`, `.claude/rules/tools.md` | Retired; the generated section and the MCP server instructions carry what every session needs |
| `.magistr/project.json` `quality_gate` | `checks` |
| `.magistr/project.json` `custom_gates` | `custom_checks` |
| `.magistr/project.json` `verification.gates` | `verification.checks` |
| `.magistr/project.json` `roadmap_exports` | `plan_exports` |
| `.magistr/team.json` | `.magistr/people.json` |
| `ROADMAP.md`, `ROADMAP.json` | `PLAN.md`, `PLAN.json` |
| Hook guard reasons, `CLAUDE.md` / `AGENTS.md` generated sections, MCP server instructions | This page's words only |

## Storage, wire and code

The owner decided on 1 November 2026 that saved records, the MCP wire and Rust
identifiers all use this page's words. There is one vocabulary from disk to
screen, and no translation layer.

- **Wire.** Tool names, argument names, result fields, status values and
  reference prefixes are the new words. A request that uses an old name is
  refused like any other unknown name. Nothing accepts both names.
- **Storage.** The workflow store directories become `plan/`, `run/`,
  `decisions/` and `requests/` (today: `roadmap/`, `ship/`, `think/`,
  `signal/`). Record keys, status values and stored references take the new
  spellings.
- **Upgrade.** The [release policy](../../AGENTS.md) requires existing
  installations to upgrade automatically, so each store's owner converts its
  records once, when it opens them. The conversion keeps the original as a
  recoverable backup, is idempotent and safe after interruption, and rewrites
  references in a fixed order: `task:` to `step:` first, then `chunk:` to
  `task:`. Real old-to-new fixtures cover it. After the conversion, readers
  accept only the new spellings. This is the required upgrade, not a
  compatibility path.
- **Code.** Modules and types follow the term table: `roadmap` → `plan`,
  `ship` → `run`, `think` → `decision`, `signal` → `request`; `Chunk` →
  `Task`, `ChunkStatus` → `TaskStatus`, ship `Task` → `Step`, `Objective` →
  `Goal`, `ShipRun` → `Run`, `ThinkStep` → `Note`, `Branch` → `Thread`,
  `CheckReceipt` → `CheckResult`, `Signal` → `Request`, `Lane` → `Team`,
  `Workstream*` and `Group*` → `Feature*`, approval `Gate*` → `Approval*`.
  Rename the ship `Task` types to `Step` before the plan `Chunk` types take
  `Task`. Crate names do not change.

## Copy

### Words the interface never shows

chunk, roadmap, group, track, workstream, lane, signal, think, trace, ship,
objective, receipt, backref, cross-ref, gate, lease, claim, backlog,
obsoleted, blocked, branch (in the decision-log sense).

This list covers user-visible strings in the desktop, the website, CLI output
and help, and agent-facing tool descriptions. Code identifiers and test names
follow the code rules above.

### State

- Use the "Shown as" words exactly, in sentence case: "To do", not "TODO" or
  "Pending".
- Show a state with its color and shape together, never color alone.
- "Done" means its checks passed. Do not show "Done" for a task whose checks
  have not passed. Show "In progress" with the failing check instead.

### Waiting

- Always say what something is waiting on, and for how long: "Waiting on a
  person · 2 days", "Waiting on Billing's *Invoice export* · 5 hours".
- Name the person or team when it is known. Add "Release" or "Answer" when the
  viewer can act on it.
- Say how much work the wait holds up: "blocks 3 tasks".

### Errors

- Say what happened, in the user's words, and the next action: "This task's
  checkout expired, so another agent has it now. Your changes are on branch
  *x*."
- Do not show error codes, type names, file paths into the store, or stack
  traces in the interface. Write them to the log.
- Do not tell a user to delete saved data to recover.

## Cutover

The [vocabulary cutover](../design/fleet-scale-roadmap.md#delivery) applies
this page one family at a time. The owner chose this split on 2 November 2026.
Each family task renames its words completely in one change: code, storage
upgrade, wire, CLI, skills, generated instructions, docs, fixtures and
snapshots. No family keeps an alias, and nothing accepts both names. The tasks
run in this order:

1. `vocab-commands`: workspace `run` → `command`, `magistr run` →
   `magistr start`, `--run` → `--loop`, `--track` → `--feature`, and
   `magistr team` / `.magistr/team.json` → `magistr people` /
   `.magistr/people.json`. This frees the words run and team.
2. `vocab-run-family`: ship, objective, ship task, action, receipt and approval
   gates, and `magistr plans` → `magistr runs`. The run store moves from
   `ship/` to `run/` (its check results from `receipts/` to `check_results/`)
   and every store rewrites its stored run references here: `step:<n>` (the
   retired spelling of a reasoning step) to `think:<n>` first, then `task:` to
   `step:` and `action:` to `progress:`. Admission does the move once, before
   any engine opens, and keeps the originals under the store's `backups/`;
   readers refuse until it is done.
3. `vocab-plan-family`: roadmap, chunk, group and the task states. The plan
   store moves from `roadmap/` to `plan/` after the run store has moved, and
   every store rewrites `chunk:` to `task:`. One admission-time owner,
   `StoreWordsUpgrade`, keeps the order of the moves; each family's codec
   converts its own documents and decides what is old from the document's
   saved version, never from its references, because `task:` meant a run step
   before the run words and means a plan task after the plan words. Plan and
   signal documents gain a leading `schema_version`. `project.json`
   `roadmap_backend` and `roadmap_exports` and the YAML plan
   `.magistr-roadmap.yml` are converted by their configuration owner. A
   readiness that waits on dependencies is `dependent`; `waiting` is reserved
   for a named reason.
4. `vocab-decision-family`: think, step, branch and session in the decision
   log. The decision store moves from `think/` to `decisions/` after the plan
   store has moved; every store rewrites `think:` to `decision:` and the
   decision fields (`think_step` to `note`, a plan refresh's `think_steps` to
   `notes`). Saved logs hold `notes` and `threads` keyed by `log_id`; a note's
   number is `note`, and an open thread is stored as `open`. Every family's
   codec moves one version for it. Owners older than these words lock in the
   plan directory, so current owners hold their project lease under
   `owners/decision-words/` and the move waits while an older lease is held.
   The cloud record kind for a note is `note`, and the family id is
   `decision`.
5. `vocab-request-family`: signal. The request store moves from `signal/` to
   `requests/` after the decision store has moved; every store rewrites
   `signal:` to `request:`, and request documents rename their list to
   `requests`, kind `idea` to `suggestion`, status `surfaced` to `raised` and
   its stamp `surfaced_at` to `raised_at`. Every family's codec moves one
   version for it. Owners older than these words lock under
   `owners/decision-words/`, so current owners hold their project lease under
   `owners/request-words/` and the move waits while an older lease is held.
   The cloud family id is `request`, and a promoted request's new task is
   `request-<id>`.
6. `vocab-teams-checks`: lane, workstream and gate.
7. `vocab-copy-gate`: desktop, website and docs copy, plus a check that fails
   on a word from [Words the interface never shows](#words-the-interface-never-shows).
8. `vocabulary-cutover`: ship the staged skills, then prove an end-to-end agent
   run that uses only the new names.

While the tasks land, a family that is not done yet still uses its old words.
Use this page to translate them.

These keep their spelling on purpose:

- Paths and ids that carry an old word are identifiers, such as
  `docs/design/fleet-scale-roadmap.md` and task ids like `chunk-claim-lease`.
  The copy task renames doc paths. Task ids keep their spelling.
- Recorded evidence under `docs/evidence/` is history and is not rewritten.
- The retained workspace reference tree under `subsystems/workspace/reference`
  is pinned upstream code. The native tools own the public names.
- The workspace error code `RUN_REDIRECTED` and the runner plugin interface
  (`magistr:runner`) describe running a process, not a run.
- The step verbs (`arrange_steps`, `start_step`, `record_progress`,
  `check_step`, `finish_step`) carry no family prefix, so the run family claims
  them by exact name; it also claims the `approval_` prefix.
- Real old-format fixtures, the upgrade code that converts them, and the tests
  that prove old names are refused keep the old words: they name what is
  converted.
- `receipt` in workspace writes (apply, mutation, source-write receipts) and in
  `execute` composition results is a different word from a check result and
  keeps its spelling here; `vocab-copy-gate` decides its public copy.
- Words other families own stay until their task lands: the quality-gate words
  `gate`, `magistr_gate` and `custom_gates`, and `workstream`, `track` and
  `lane` (teams and checks).
- `thinking` is the model's own reasoning (Claude's extended thinking, the
  ACP tool kind `think`), not the decision log, and keeps its words. So do a
  git branch, an agent or conversation session, and the
  `think-and-ship` backend name and directory that older installations
  upgrade from.
- Repository mirroring writes the Agent Trace format, whose own words
  (`.magistr/traces/`, its `sessions/` files and `session_id`) belong to that
  format.
- The recorded build session under `apps/website/public/demo` uses the words
  of its day, so the website code that replays it reads those words.
- The desktop's internal names and shown labels for the decision log
  ("Reasoning", "Step 41", "Investigation") are copy; `vocab-copy-gate`
  brings them to the glossary. The fields they read already use the stored
  words.
- The skill names `roadmap` and `roadmap-refresh` and their template files stay
  until `vocabulary-cutover` ships the staged `plan` skill; their text already
  uses the plan tools.
- `signal` in its other senses keeps its spelling: operating-system signals
  (`SIGINT`, `tokio::signal`, process groups), the workspace's search ranking
  signals and their plugin interface, OpenTelemetry signals (traces and logs),
  and plain English such as "a liveness signal".
- The skill name `signals`, its template `templates/workflows/signals.md` and
  the `/signals` and `$signals` invocations stay until `vocabulary-cutover`
  ships the staged `requests` skill; the skill's text already uses the request
  tools. Task ids minted by earlier promotions (`signal-<id>`) keep their
  spelling.
- The plan importer reads people's own documents, so it still recognizes their
  words (`pending`, `backlog`, `blocked`, `obsoleted`) as input. External
  tracker vocabularies (Linear's `backlog` state type, GitHub Projects column
  names) belong to those services.
- Shown state labels in the desktop ("Backlog", "Planned", "Blocked",
  "Completed") are copy; `vocab-copy-gate` brings them to the "Shown as" words.
  The keys they label already use the stored words.
- The imported upstream documentation under `subsystems/workflow/docs` and the
  recorded demo session under `apps/website/public/demo` are history and are
  not rewritten.
