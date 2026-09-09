# Peer protocol

`magistr_engine::peer` implements agreement between two fixed peer sessions,
task objections, instruction leases, and worker admission. It is an engine
domain API. Production runtime authentication, transport, metering, persistence,
and dispatch are not wired into the CLI or interfaces.

The host is responsible for those boundaries. A model response cannot establish
its own identity, grant permission, report trusted usage, or certify completion.

## Plans and approval

`PlanAgreement::new(contract, body)` validates and seals revision 1. The immutable
contract binds project/run identity, two peer sessions, allowed worker
provider/model/effort tuples, an absolute deadline, token and attempt ceilings,
reconciliation rounds, retry allowance, and outage policy. Limits must be finite
and positive where required. `RunLimits::new` defaults to two reconciliation
rounds and zero retries.

A plan records scope, evidence references, ordered constraints, tasks and their
dependencies, supervising peers, acceptance, risks, selected workers, routing
policy, and retries. Worker selection is a nonempty unique subset of the run
allowlist. Tasks have unique IDs, fit the attempt ceiling, and form a graph
without missing, duplicate, or cyclic edges. Empty risk/constraint lists mean
none recorded; blank entries are invalid.

Each peer approves the exact `PlanReference`: project, run, revision, and digest.
The host supplies identity from the authenticated session, including role,
session ID, provider, model, and effort. All fields must match. Repeating one
peer's vote does not create a second vote. `has_dual_approval` applies only to
the current revision.

`revise(body)` validates the replacement, increments the revision, and clears
both approvals, even if the body repeats. A failed revision leaves the existing
plan and votes unchanged. The original run contract cannot be replaced.

## Plan encoding

Plan schema version 1 hashes `magistr-peer-plan-v1\0`, including its trailing NUL,
followed by compact serde JSON for `[schema_version, revision, contract, body]`.
The digest is SHA-256 in lowercase hex. Struct fields use declaration order;
there are no arbitrary JSON maps, floats, or implicit defaults in the payload.
Vector order and exact string bytes matter. Whitespace and object member order
in input JSON do not matter after typed decoding.

This is a versioned Magistr encoding, not generic sorted JSON or RFC 8785 JCS.
Changing field order or encoding requires a schema decision.
`PlanRevision::from_json(bytes, expected_contract)` rejects unknown fields and
versions, validates the graph and trusted contract, and recomputes the digest.
Restoring a bare revision with `PlanAgreement::from_revision` starts without votes.

## Reconciliation

`Reconciliation` collects one proposal from each fixed session against the
current reference and the same context. Context includes ordered constraints
and roadmap, code, and routing-history references. The proposal accessor exposes
the pair only after both submit. Identical resubmissions are idempotent;
contradictory submissions by one peer in the same round fail.

Once both proposals exist, `Reconcile` seals a candidate under the original
contract and context. Either peer may suggest a candidate; neither the host nor
matching proposals supply a vote. Both fixed sessions must approve it explicitly.
The host must deliver common evidence and isolate proposal generation.

Either peer can `Disagree` with a reason during candidate review, including after
agreement while using this planning aggregate. Disagreement clears both votes
and proposals, advances the revision, and opens another collection/review round.
Disagreement in the final allowed round stops the run permanently. A host cannot
replace a candidate during review without peer disagreement.

## Observations and limits

Every input carries trusted Unix milliseconds, cumulative whole-run tokens, and
cumulative worker starts. Include deliberation, evidence calls, workers, retries,
and rejected output in token usage. Attempts count worker starts, not messages.
Observations cannot decrease across rounds or restoration; clamp a backward
clock to the last observation.

Reaching the deadline or token ceiling, exceeding the worker-start ceiling, or
peer unavailability stops authority. An action can return `Stopped(reason)` as
a successful result without accepting the requested operation. Inspect and
persist that outcome. Stops do not resume, and neither outage policy supplies
a missing vote.

For new runs, exactly reaching `max_attempts` prevents additional admissions but
does not by itself invalidate existing worker completion. Other fences still
apply. Version 1 journals retain their older terminal stop at equality.

The API reads no clock, calls no provider, and reserves no tokens. Observe during
silence, reserve budget before external calls, and use fresh observations before
acting on state. Reconciliation input errors leave its state unchanged; consumed
usage must still reach it through `Observe` or the next cumulative reading.

## Shared briefing

Use `TaskAuthority::with_briefing` for runs that require common evidence.
`PeerEvidence` contains the actual roadmap, code evidence, and routing history;
a revision or URL alone does not provide that content. Sections must be nonblank.
An explicit statement can represent empty routing history.

`evidence.context(constraints)` derives role-separated, versioned SHA-256
commitments to the exact UTF-8 sections. `PeerBriefing::new(contract, body,
evidence)` binds them, the ordered constraints, and the run contract into the
initial plan. A briefing is strict version 1 JSON containing `plan`, `contract`,
`body`, and `evidence`, limited to 262,144 encoded bytes including whitespace.
Decoding checks nested types, commitments, initial reference, and trusted contract.

Both peers receive byte-identical canonical JSON from
`briefing_for(authenticated_session)`. Reading it records no delivery. After
sending those bytes, the host records and persists
`AuthorityAction::DeliverBriefing { peer, briefing }`. Each peer then submits
`acknowledge_briefing` with the exact initial reference. Acknowledgement before
delivery fails; duplicate identical receipts are idempotent. Only after both
acknowledge may either propose, including through the direct host protocol API.

Briefing receipts remain tied to the initial reference across later candidates.
They do not approve candidates, revive votes, or prove that a model understood
the evidence. Delivery is a host-only operation.

## Requests and attribution

`TaskAuthority::receive_peer(observation, authenticated_session, bytes)` accepts
one UTF-8 JSON object, limited to 65,536 bytes including whitespace:

```json
{
  "schema_version": 1,
  "request": {
    "kind": "approve",
    "plan": {
      "project_id": "project-a",
      "run_id": "run-1",
      "revision": 2,
      "digest": "<exact current plan digest>"
    }
  }
}
```

Use the real digest. `PeerRequest::to_json()` checks the same bound but does not
authorize a request. Markdown fences, multiple values, missing/unknown versions,
unknown or duplicate fields, and oversized messages fail.

| Request | Additional fields |
| --- | --- |
| `acknowledge_briefing` | `briefing` |
| `propose`, `reconcile` | `base`, `body` |
| `approve` | `plan` |
| `disagree` | `plan`, `reason` |
| `object` | `plan`, `id`, `tasks`, `reason` |
| `resolve` | `plan`, `id` |
| `issue_lease` | `plan`, `task`, `expires_unix_ms` |
| `admit_worker` | `plan`, `task`, `lease_generation`, `id`, `worker` |

The receiver records usage first, then checks identity, decodes the message, and
applies the attributed action. An error can therefore update the journal or
expire leases. A decreasing observation fails without changing state. An existing
or reached stop returns `Stopped` without interpreting the message. Checkpoint
after every receive, including errors.

The wire format has no peer identity, observation, outage, host-delivery,
completion, or termination field. Never deserialize model output directly into
`AuthorityAction`, which contains privileged host operations. The journal stores
translated actions and observations, not raw rejected messages. Candidate
reconciliation checks identity but does not record which peer suggested it.

## Objections and leases

Either fixed peer can object against the agreed reference using a run-unique ID,
nonempty unique task IDs, and a reason. The named tasks and all transitive
dependents pause; independent tasks retain their leases. Identical active
objections are idempotent. IDs cannot be reused with different content or after
resolution. Only the author can resolve an objection against the current agreed
revision. Overlapping objections remain active, and resolution never revives leases.

Plan disagreement revokes every lease. Ownership changes require a new plan and
both fresh approvals; joint approval also resolves objections to the old plan.
There is no mutable owner override or extra voting peer.

`IssueLease` requires current dual agreement, a known unpaused task, its exact
supervising session and plan reference, and an expiry after the observation but
no later than the deadline. Leases bind the plan, task, owner, increasing run
generation, and expiry. Each issuance replaces that task's previous lease.
Objection, plan change, expiry, or stop invalidates it permanently. Generations
survive revision/restoration; overflow refuses issuance.

Validate the lease at every instruction operation with a fresh observation and
authenticated session. Validation records observations even when the fence check
fails. It does not interpret instruction prose, enforce free-text routing policy,
or prove that a revoked worker has stopped.

## Worker admission and settlement

`AdmitWorker` requires a current lease, run-unique request ID, exact planned
provider/model/effort, and accepted completion of all dependencies. One new
admission returns a `WorkerPermit`. An identical repeat returns `AlreadyAdmitted`
without another permit and still requires current authority. Reusing an ID with
different content fails. A worker must be in the plan's model list, not just the
broader run allowlist. Ad hoc children are refused.

A task has at most one unsettled attempt. Even after revocation, the host must
reconcile that runtime and record termination before replacement. Accepted tasks
cannot restart. Each stable task gets its first attempt plus its retry allowance;
counts do not reset across revisions or restoration.

Admission reserves a run attempt even if launch fails. Reservations are at least
the observed whole-run starts, increment for each new admission, and are never
refunded. All subsequent starts must pass through this authority; unmanaged
starts cannot safely run alongside its accounting.

`AcceptWorker` requires the supervising peer's current fence and a nonempty
host verification receipt. The host verifies artifacts, checks, and termination
before acceptance. Acceptance settles the attempt and unblocks dependents.
`RetireWorker` records host-only termination without acceptance and remains
available after a stop. Neither action refunds an attempt or settles it twice.

`validate_worker` checks fresh observations, the lease, and the unsettled permit
at receiving operations; it does not authorize a second launch. Objections clear
dependency completion for their affected closure. Resolution does not restore it.
A new plan clears all dependency completion while retaining receipts and retries.

## Journals and host responsibilities

| Journal version | Behavior |
| --- | --- |
| 1 | Stops when observed starts equal the ceiling; retains this policy on replay and reserialization |
| 2 | Separates exhausted start capacity from completion of the final admitted worker |
| 3 | Authority with shared briefing; validates matching evidence and delivery/acknowledgement order |

Versions 1 and 2 cannot carry a briefing. `TaskAuthority::new` retains the ungated
constructor; use `with_briefing` for shared-context integration. There is no
in-place journal upgrade. The immutable plan schema and digest remain version 1.

Restoration validates the trusted original contract, replays accepted transitions,
rejects malformed, stale, impossible, and noncanonical actions, and applies fresh
limits. It can restore votes and objections but discards live instruction leases.
Worker request IDs, reservations, retry history, unsettled attempts, and receipts
survive. Old permits do not regain authority; reconcile unsettled runtimes first.

Hashes and valid replay are not authentication or rollback prevention. Protect
host storage, persist atomically before external effects, and serialize receipt,
persistence, dispatch, validation, and invalidation under one authority. If
persistence fails, do not dispatch. A copied checkpoint is not another authority,
and a permit does not prove exactly-once execution or termination.

Run `cargo test -p magistr-engine peer::tests --lib` for protocol and replay tests.
They use fixture identities and do not establish live peer authentication or
governed runtime dispatch.
