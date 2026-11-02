# Fleet coordination

1 November 2026. This contract covers one plan shared by every project in a
company and worked by many agents and a few people. It names the owner of each
new record and verb. Every fleet task builds against it:
`roadmap-group-record`, `roadmap-plan-approval`, `chunk-claim-lease`,
`person-hold-staleness`, `lane-scoped-next`, `cross-project-deps`,
`directory-server`, `owner-directory-client` and `fleet-metrics`. The
[fleet design](../design/fleet-scale-roadmap.md) explains why and shows the
journeys. This page states what must be true.

Words follow the [vocabulary](../reference/vocabulary.md). Tool and field names
are the names after the [vocabulary cutover](../reference/vocabulary.md#cutover).
Until the cutover lands, use the vocabulary page to translate them.

## Ownership

One project keeps one owner ([project authority](project-authority.md)). The
plan owner is that owner's plan use cases, and every record below that belongs
to a project lives in its plan store. No new record gets a second writer.

| Record or verb | Owner | Writers | Readers |
|---|---|---|---|
| Feature record (state, approver, limit, budget) | Plan owner | `plan_feature_update` (state, approver, limit, budget) | Readiness, desktop, integrations |
| Plan approval (approved task set) | Plan owner | `plan_propose`, `plan_amend`, `plan_approve`, `plan_refuse` | Readiness, GitHub projection |
| Team record (name, bound features, members) | Plan owner | `plan_team_update` | Checkout admission, `plan_next` |
| Checkout (agent holder) | Plan owner | `plan_start_task` issues it. Run writes renew it. `plan_release_task` and the expiry sweep end it. | Readiness, run owner, desktop |
| Hold (person holder) | Plan owner | `plan_start_task` issues it. Activity renews it. `plan_release_task` by a person ends it. | Staleness ladder, desktop |
| Checkout number sequence | Plan owner | Incremented only by `plan_start_task` | Fencing on run writes and `plan_complete_task` |
| Run binding to a checkout | Run owner | `run_open` with scope `task:<id>` | Run step writes, `plan_complete_task` |
| Cross-project dependency fact | Publishing project's plan owner | `plan_complete_task` and reopen publish it; the directory relays it | Depending project's readiness |
| Fact cache (relayed facts received) | Depending project's plan owner | Directory intake only; no verb edits it | Readiness |
| Directory registration | Directory | The registering owner, with its owner token | Owners, company board |
| Directory fact relay | Directory | Publishing owners, signed | Subscribed owners |

The directory owns no task and can change nothing in a project. Readiness,
checkouts, approvals and completion stay inside each project's owner.

## Qualified ids

A task id is unique inside its project. Across projects, a task is named by its
**qualified id** `<project_id>/<task_id>`, for example
`billing-1a2b3c/invoice-export`. `project_id` is the existing
`resolve_project_id()` value.

- A dependency on a task in the same project stays a bare id. A dependency on
  another project's task is a qualified id. The `/` marks it as one.
- References use the same form: `task:billing-1a2b3c/invoice-export`. The
  unified record envelope's `CrossRef` pattern already admits it.
- Task ids may not contain `/` or `:`. `cross-project-deps` adds this refusal to
  `plan_add_task`. The plan store's upgrade refuses to open a store with a
  conflicting id and names it, because no existing store has one.
- In the directory, every task is named by its qualified id, including the
  envelope `id`. `directory-server` widens the envelope `id` pattern to admit
  `/`. The change is additive, so every id valid today stays valid.

## Holders

A task in progress has exactly one holder. There are two kinds.

| Kind | Identity | Right | Ends by |
|---|---|---|---|
| `agent` | Session id, provider, model and team, supplied by the host from the admitted session, never by the agent | **Checkout**: expires unless renewed | Completion, release, waiting, or expiry |
| `person` | A member of `.magistr/people.json`, matched through the verified login ([authentication](../reference/authentication.md)) | **Hold**: never expires | Completion, release by a person, or waiting |

Both kinds take numbers from the same per-project checkout number sequence. The
sequence is persisted with the plan, never reused, and survives owner restarts
and upgrades. A task stores its current holder and number as one `holder`
field. A task with no holder in progress is a defect the owner refuses to write.

## Checkouts

### Taking one

`plan_start_task` is the only way into progress. In one serialized write it
checks that the task is ready ([readiness](#readiness)) and has no holder. It
also checks that the caller's team is bound to the task's feature. Then it
moves the task from to do to in progress, records the holder and issues the
next number. It returns the number and `expires_at`.

When several agents ask for the same task, the single project writer serializes
them. The first is admitted. Each of the others gets a refusal that names the
next ready task in its scope, so it can ask again at once.

### Expiry and renewal

- An agent checkout starts with a time to live of 15 minutes, configured in
  `.magistr/project.json` under `fleet.checkout_ttl_secs`.
- Every write under the checkout's run renews it to now plus the time to live:
  `start_step`, `record_progress`, `check_step`, `finish_step`. `decision_record`
  also renews it when its `run_ref` names a step of the run.
- The owner runs checks itself, so a check it admitted renews the checkout when
  it starts. The check keeps the checkout from expiring until it settles. A long
  check never costs an agent its task.
- `plan_renew_task` renews without other progress, for work that writes nothing
  for a long time. It is fenced like every other write.

### Return to the to-do list

- A checkout is expired once `now >= expires_at`. Readers compute this from the
  stored time, the same rule the approval record uses. Readiness treats an
  expired checkout's task as to do before anything is written.
- The owner's expiry sweep is the one writer that settles it. It runs on owner
  start, before each `plan_start_task` and `plan_next`, and on a timer. It moves
  the task from in progress to to do, clears the holder, keeps the last issued
  number and writes one log line naming the task, holder and number.
- The run bound to that checkout stays as it is, with its steps, progress and
  check results. The next holder opens a new run. Its first step can read the
  earlier run history.
- The sweep never touches a person's hold.

### Fencing

A run opened with scope `task:<id>` records the checkout number current at
`run_open`. `run_open` is refused if the caller is not the task's holder.

- The run owner refuses `start_step`, `record_progress`, `check_step` and
  `finish_step` on a run whose number is no longer the task's current number.
- The plan owner refuses `plan_complete_task` unless the completing run's
  number equals the task's current number. This holds even when every required
  check passed. The check results stay recorded, so a person can see the work
  was done, but they do not mark the task done.
- The refusal follows the [error copy rules](../reference/vocabulary.md#errors):
  "This task's checkout expired, so another agent has it now. Your changes are
  on branch *x*." The log records the stale and current numbers.

Fencing is checked where the record is written, not by the agent, so an agent
that resumes after a pause cannot write under an old number.

### Lifecycle

The [lifecycle table](../../crates/magistr-workflow-contracts/src/roadmap/records.rs)
(`ChunkStatus::allows`, after the cutover `TaskStatus::allows`) does not change.
Every move above is already legal:

| Event | Move | Holder after |
|---|---|---|
| Checkout or hold taken | to do → in progress | Set, new number |
| Expiry sweep, or release | in progress → to do | Cleared |
| Completion with current number | in progress → done | Cleared |
| Holder marks it waiting | in progress → waiting | Cleared |
| Waiting reason resolved | waiting → to do | None |

A task leaving waiting goes to to do and is taken again through
`plan_start_task`, so it always gets a fresh number.

## Holds and the staleness ladder

People do not get an expiring checkout. Taking a task back without asking can
throw away uncommitted work. A hold instead has an **activity clock**: the
latest of its last progress record, its last check result, and the newest
commit on the task's branch. The owner reads that commit time on demand from
the working copy and stores nothing about it.

The owner computes two thresholds from how many tasks the hold holds up.
`blocks` counts every to-do task that depends on it, directly or through other
tasks, including tasks in other projects whose facts name it:

```
nudge_after    = fleet.hold.nudge_secs    / (1 + blocks)
escalate_after = fleet.hold.escalate_secs / (1 + blocks)
```

The defaults are 2 days and 4 days. A leaf task escalates after 4 days, and a
task holding up three others escalates after 1 day. A floor of 1 hour applies.

1. **Nudge.** Idle past `nudge_after`, the owner emits a nudge event to the
   holder. Their tool shows it: desktop, editor or chat integration. Nothing else
   changes.
2. **Escalate.** Idle past `escalate_after`, the hold joins the feature
   approver's "Waiting on you" list. The entry shows its age, the holder, the
   tasks it holds up, and **Release**.
3. **Release.** `plan_release_task` from a person moves the task from in
   progress to to do. It clears the hold and records who released it, why and
   when. The holder's git branch and run history stay as they are, and the next
   holder can start from them.

Nothing releases a person's task unless a person decides to. Agents and the
expiry sweep cannot call `plan_release_task` on a hold. The owner refuses an
agent identity for that verb. Any activity resets the ladder.

## Features and plan approval

A feature record is stored in the plan store, keyed by its name. It holds
`state` (`draft`, `proposed`, `approved`, `refused`), `approver` (a
`people.json` member), `limit` (the most tasks of the feature in progress at
once; none means no limit) and `budget` (compared with recorded spending by cost governance in
`magistr_domain::policy`. The plan owner never enforces it).

Approval covers the exact tasks that were reviewed. `plan_approve` records
the approved task set: each task id with a digest of its title, acceptance,
dependencies and feature. It also records the approver and the time.

- A task is approved when its id is in the set and its digest matches. Editing
  an approved task's title, acceptance or dependencies makes that task
  unapproved until `plan_amend` and `plan_approve` cover it again. Other
  approved tasks stay ready.
- A task added to an approved feature is not ready until an amendment that
  includes it is approved. Ideas are outside the set until promoted to to do.
- `plan_approve` and `plan_refuse` need a person: the feature's approver, or a
  member with the approver role. An agent identity is refused. The GitHub plan
  pull request is a projection. Its review decision reaches the owner as
  `plan_approve` from the verified reviewer, and the decision lives in native
  state.

This follows the rule the [peer protocol experiment](peers.md) uses:
approval binds to an exact revision, and a change clears it. The experiment
itself stays opt-in and is not the owner.

### Decision: the approval record is not plan approval

The existing approval record (`approval_open` / `approval_wait`, after the
cutover `approval_open` / `approval_wait`) was evaluated as the plan-approval
mechanism and **rejected**:

- It lives only in the cloud workspace. With no workspace connected it settles
  to its default at once. Plan approval must work offline and in a project that
  never connects.
- An unanswered approval settles to its declared default at `expires_at`. Plan
  approval must never be granted by a timer.
- It answers one question once, from the web app. Plan approval covers a set of
  tasks, has amendments, can be refused, and is given from the desktop, CLI or a
  GitHub review.
- Readiness must read approval from the plan store, the same snapshot as every
  other readiness input. A cloud poll cannot be part of that snapshot.

The approval record stays what it is: an agent's question inside a run that a
person answers ("needs a decision"), with a safe default. The design's Slack
question uses it. Plan approval is the native record above.

## Teams and scoped next

A team record names a team, the features it is bound to and its members (agent
sessions by provider and team label, people by `people.json` id). A team that
works in several projects has a record with the same name in each one. The
directory lists them together but stores no team.

`plan_next(team)` returns the most urgent ready task among the team's
features, the smallest priority number first. `plan_start_task` repeats the
scope check, so an agent outside the scope is refused even if it names the task
directly.

## Readiness

Readiness stays one derived function, `assess_readiness`
([readiness.rs](../../crates/magistr-workflow-contracts/src/roadmap/readiness.rs)),
fed from one snapshot. A task is ready when all five conditions hold:

1. It is to do (an expired checkout counts as to do).
2. It has no waiting reason.
3. Every dependency is done. A same-project dependency is done by its status. A
   cross-project dependency is done only by a verified `done` fact in the fact
   cache. A missing fact means not done (fail closed, as a missing dependency
   does today).
4. Its feature is approved and the task is in the approved set with a matching
   digest. A task with no feature is ready without approval, as it is today.
5. Its feature is under its limit: in-progress tasks with any holder are fewer
   than `limit`.

The result names the first failing condition. "Waiting on another team" is the
derived name for condition 3 failing on a qualified dependency.

## Cross-project dependency facts

A fact says one task in another project is done, signed by that project's
owner.

| Field | Meaning |
|---|---|
| `task` | Qualified id of the task |
| `state` | `done` or `reopened` |
| `sequence` | The publisher's per-project fact counter; the higher one wins |
| `done_at` | When the publisher completed it |
| `step_ref` | The verification step whose check results proved it |
| `owner_generation` | The publishing owner's generation |
| `signature` | Signature by the publisher's registered owner key |

- The publishing owner emits a fact on `plan_complete_task`, and a `reopened`
  fact when a done task moves back to in progress. Facts leave through the
  existing [cloud outbox](../../crates/magistr-workflow/src/cloud/outbox.rs), so
  an offline owner queues them.
- The depending owner accepts a fact only if its signature verifies against the
  publisher's registration and its `sequence` is higher than the cached one. It
  stores the fact unchanged. No verb, agent or person edits the cache.
- A `reopened` fact makes not-yet-started dependents unready. It does not take
  back a checkout or hold already issued.

## Directory

The directory is a registry of owners and a relay of facts. It owns no task,
decides no readiness and writes into no project.

### API on the unified record envelope

The directory speaks
[`unified-record-envelope.schema.json`](../../subsystems/workflow/contract/unified-record-envelope.schema.json)
and its routes ([wire contract](../../subsystems/workflow/docs/UNIFIED_CONTRACT.md)).
`directory-server` adds one family, `directory`, with two kinds:

| Kind | Envelope `id` | `record` | Idempotency key |
|---|---|---|---|
| `owner` | `project_id` | Project id, display name, team names, owner public key, build identity, registered_at | `sha256(tenant ␟ directory ␟ owner ␟ id)` |
| `fact` | Qualified task id | The fact fields above | `sha256(tenant ␟ directory ␟ fact ␟ id ␟ sequence)` |

- `tenant_id` is the company workspace. Every directory record carries
  `lenses: [<project_id>]`, so a project is a lens, as the wire contract already
  defines.
- Registration: `PUT /v1/records/directory/owner/{project_id}` with that owner's
  token. A project registers once, and only its own token updates it.
- Publish: `PUT /v1/records/directory/fact/{qualified_id}` with the publishing
  owner's token. The directory refuses a fact whose project is not the token's.
- Receive: owners read `GET /v1/records/directory/fact` filtered by the
  qualified ids they depend on, and follow `/v1/events` for changes. Each owner
  verifies every fact itself, so a directory that alters one is caught.
- Company board: owners may also publish read-only copies of their task
  envelopes, family `plan`, with qualified ids, for the company board. The
  directory serves them and never writes them back.

### Self-host topology

```
 project owner A ──┐  token, outbox   ┌──────────────────────────────┐
 project owner B ──┼────────────────▶ │ magistr directory serve      │
 project owner C ──┘  facts, events   │  SQLite (default) | Postgres │
                                      │  token or OIDC sign-in       │
                                      └──────────────────────────────┘
```

- `magistr directory serve` runs the directory in the binary companies already
  install. It is one process with SQLite by default, or Postgres when
  `--database postgres://…` is given.
- Owners sign in with an owner token minted by the directory. People sign in
  through OIDC with the existing device-flow login.
- The hosted directory runs the same code. Neither has anything the other lacks.
- An owner configured with a directory registers on start, publishes through
  the outbox and follows events while connected. Offline, it keeps working and
  cross-project dependencies keep their last verified facts.
- A project with no directory behaves exactly as it does today. A qualified
  dependency in such a project shows as "Waiting on another team · not
  connected".

## Observability

Every checkout issued, expired by the sweep, released or fenced, and
every fact accepted or refused, is one structured log line with the qualified
task id, holder kind and number. `fleet-metrics` builds its counters and
dashboard on these. No event pages a person.

## Out of scope

- Moving tasks between projects. A task belongs to one project for life.
- A shared store or a cross-project writer. Projects connect only through facts.
- Automatic plan approval, and automatic release of a person's hold.
