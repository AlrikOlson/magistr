# Projections

Workflow projections describe authored records and the evidence available for a
read. They do not grant permission to change those records. The workflow engines
own persistence, `NativeReader` reads through those owners, and the application
retains the resulting observations for its consumers.

The current desktop renders conversations, session controls and project setup.
It does not yet render a workflow browser. Its application seam is
`NativeReader` → `ProjectSeed` / workflow reseeding → `SessionObservation`.
Adding work views must consume that seam rather than introduce another store.

## Canonical values

`magistr-workflow-contracts` owns authored records, family frame enums and pure
projection values. It has no store, process, provider, workspace-client or async
runtime dependency. Filesystem reads, broadcasters, report-file parsing and
owner-generation allocation remain with their effectful owners.

Roadmap readiness is a pure policy over one complete set of chunks. Only a
completed dependency satisfies a requirement. Workflow queries, workstream
reports and new-work admission use that policy. Lower numeric priorities come
first, with ties retaining canonical insertion order. A saved view grants no
authority: the writer checks current records when it admits a start.

`WorkstreamCatalog` separates unfinished groups from completed and obsolete
history counts, retaining a distinct ungrouped bucket. Historical chunk IDs
are returned only by explicit history inspection. Each current group reports
lifecycle counts, readiness counts and its next candidate.
`WorkstreamFrontier` assesses each item against the complete roadmap, including
dependencies outside its group. These values are derived, never stored; closing
or moving the last unfinished item does not delete or relocate history.

Saved focus names are exact identities. A read reports `current`, `closed`,
`missing` or `unset` without changing focus. Explicit selection accepts an
unambiguous current-name fragment; an exact closed name remains closed,
including a case-insensitive exact match. `roadmap_focus_get` defaults to
`view: "current"`; `view: "history"` returns completed and obsolete chunk IDs
for the named or saved group, including a group that also has current work.
The full-record read remains available for those IDs.

## Validated family reads

`NativeSnapshot` contains four independent `ReadState` values:

| Family | Value | Persistence owner |
| --- | --- | --- |
| Roadmap | `RoadmapData`: chunks and progress from the same read | `RoadmapEngine` |
| Execution | `ShipData`: selected and retained runs from the same read | `ShipEngine` |
| Signals | `Vec<Signal>` | `SignalEngine` |
| Reasoning | `ThinkData`: selected session, steps and branches | `ReasoningServer` |

Each observation preserves both its value and its provenance:

| State | Meaning | Consumer behavior |
| --- | --- | --- |
| `Unobserved` | No read has completed | Do not infer absence; request a read |
| `Current(Observed<T>)` | The latest read succeeded | Use the value and its source |
| `Stale { last, error }` | A read failed after an earlier success | Retain the earlier value and expose the failure |
| `Unavailable(error)` | A read failed without an earlier success | Expose the failure; do not manufacture empty records |

A successful observation has a `ReadSource`: `Memory` when persistence is
disabled, `Missing` when the owner's file is absent, or `File(StoreStamp)` for
validated bytes from an opened file. An absent file and a successfully decoded
empty file can both yield empty records, but their sources remain distinct.
I/O, decoding, schema and identity failures are errors, never successful absence.
Public failures contain recovery text; detailed paths and parser diagnostics
belong in logs.

The persistence owner opens its actual configured file and reads metadata from
that descriptor along with the bytes it decodes. It checks metadata before and
after reading and refuses an observed in-place change. It does not pair bytes
from one open with a pathname stamp from another. Atomic replacement may leave
an already opened descriptor on the preceding file; the next read opens the
replacement. The execution owner reads the file selected by its canonical
working copy and execution identity, not a guessed project-wide ship path.

`ShipData` exposes the selected `current` run and `retained` runs. Each `ShipRun`
keeps its run ID, task-instance identities, objective and complete tasks, including
actions, check details and artifacts. Selecting another objective retains the
previous run in the same locked execution aggregate, whether unfinished or closed.
Returning to an unfinished scope resumes its original run; returning to a closed
scope starts another run without overwriting history. Reused task names never
identify historical work on their own. `NativeReader::ship_run` resolves an exact
run from a validated read and distinguishes missing work from a failed read.
Historical checks retain their receipt IDs; the existing receipt reader resolves
the actual command, source and acceptance evidence independently. A historical
pass does not certify the source currently being edited. Receipt compaction keeps
receipts belonging to retained runs. Explicit execution reset clears this aggregate;
ordinary objective selection does not.

Reasoning reads use the active session of a shared owner, or the captured
configured session of a detached owner. `ThinkData::session_id` names that
scope; `None` names the one default project archive. A retained value keeps its
original session identity after a failed read of another session. These reads
do not claim to discover every reasoning session in the project.

An observation returns validated durable data. Failed mutations leave the held
state at its preceding commit; an observation never certifies an attempted
change. A writer refresh adopts only a successfully validated value. Reading does not repair old identities, adopt other projects or rewrite
old formats. Failed validation remains retryable even if file metadata is
unchanged.

`NativeReader` holds each family's owner lock through its read and the update
to a transient, clone-shared snapshot. Its retained successful value advances
only after that family's successful read. One family can be current while
another is stale or unavailable; there is no global file revision or
cross-family transaction. Derived workstream
views preserve the roadmap's read state and source.

Attaching a native reader does not itself read workflow files. Shared session
resolution reads current work before provider startup using the workflow inputs
captured in `ProjectInputs`, the canonical working copy and the selected execution.
The same execution is bound into tool subprocesses. One ship observation supplies
the objective, authored requirements and complete task plan with stable identities;
missing work is explicit and a failed read refuses startup. Fresh and resumed
sessions get a new observation, while an already resolved session keeps its snapshot.
Purpose policy continues to restrict planning conversations. Recovery instructions
require a fresh `ship_status` read after provider compaction; the current transport
has no compaction callback that refreshes instructions automatically.

Direct decision readers require a current successful read and return errors otherwise. Work selection, status decisions and
checkpoint capture cannot interpret retained stale records as current or
failures as an empty roadmap.
Checkpoint progress and roadmap identity come from the same validated
observation. Receipt-ledger reconciliation reads its own ledger independently
of whether the execution aggregate can be read.

## Mutation acknowledgement

Each family mutates freshly validated canonical state under its stable file lock.
The shared transaction primitive writes a candidate to a temporary file, flushes
and synchronizes it, atomically replaces the destination, and synchronizes the
parent directory. It returns the stamp of the committed descriptor. Only then
does the owner install the candidate and publish its family events. Disabled
persistence is an explicit memory transaction and returns `ReadSource::Memory`.
Unreadable, malformed, unsupported or wrongly owned records refuse the mutation;
they are never replaced with an empty starting point.

Successful commands return `Observed<T>`. Current native, owner IPC and MCP
consumers preserve that source rather than pair a value with a later read.
`WriteError` distinguishes read refusal, failure before replacement and an
uncertain result after replacement. The first two report `effect: none`;
uncertainty reports `effect: unknown` and requires reading saved records before
retry. `WorkflowMutationError` preserves this distinction across owner IPC.
A partial signal promotion carries the committed roadmap identity and source;
retry resumes the same reciprocal link instead of creating another work item.

Reasoning stores all project histories in one schema-2 document. That owner
assigns project-wide numbers while preserving immutable authored record IDs.
Selected sessions and bounded history windows are projections, not independent
writers. Import identifies records by authored ID, keeps distinct records with
equal prose or numbers, remaps unambiguous references, and refuses conflicting
identities. Reads never renumber authored records.

Execution receipts retain the admitted identity and complete command outcome.
Settlement persists the outcome, records application intent, applies the check
idempotently by receipt ID, then acknowledges the terminal receipt. A retry
finishes that sequence without rerunning the command or duplicating the check.
Reset commits an empty run under the same stable owner lock; it does not unlink
the lock or erase receipts. Late outcomes remain attached to their original run.

## Feed and reseeding

The owner's NDJSON feed supplies typed family events. `NativeFrame::decode`
parses wire values at the transport boundary; views do not infer phases from
narrative text or inspect arbitrary JSON fields. `MagistrFrame` carries phase
announcements and running gate output. Those observations are distinct from
persisted family snapshots.

An `EventCursor` contains the admitted owner's epoch and the publisher's event
sequence. Sequence numbers order emissions from that owner, not file writes or
records. The subscriber retains its cursor across reconnects so a later frame
can reveal a gap.

| Delivery | Meaning |
| --- | --- |
| `Local` | The local owner returned a typed event |
| `Unsequenced` | No event cursor was supplied |
| `First` | The subscriber has no prior event cursor |
| `InOrder` | The next sequence arrived in the same epoch |
| `Duplicate` | That sequence or a later one was already observed |
| `Gap` | One or more events were missed |
| `NewEpoch` | A different owner generation published the event |

A duplicate does not update records or clear a read failure. Meaningful new
family events request a coalesced validated snapshot. Gaps, new owners and
reconnections also request a read. An event is evidence that something changed;
it cannot certify that a file read succeeded. In particular, execution events
on the project feed may belong to another execution, so consumers reread their
own configured execution rather than fold that event into its records.

One `SnapshotReads` coordinator in the CLI serializes each read through
comparison and publication. The engine client, timer and explicit session
refresh share it, so a slow earlier success cannot arrive after a later failure
and clear that failure. A transient scheduling generation fences invalidations
that arrive during a read; an overtaken result is suppressed and the coalesced
request supplies another read. This generation is not file provenance. Explicit
refresh awaits that same publication path.

The reseed task reads on its interval even while a feed is attached. It also
reads immediately when requested. Every attempt validates the owner sources
before comparing observations. A timer suppresses unchanged notifications only
after that validation; it never skips a read because a timestamp stayed the
same. Unless a later invalidation supersedes it, a requested read publishes a
validated seed even when the values are unchanged. There is no cursor-only
freshness confirmation. Failed blocking reads remain visible and retain earlier
successful values.

## Session retention and identity

`SessionObservation` keeps workflow observations in its existing `ProjectSeed`,
separately from the bounded conversation transcript. A large workflow snapshot
does not consume the message budget or evict a reply. Refreshing workflow state
does not reset the conversation, draft, permissions or retained message IDs.
A failed family read cannot advance a successful baseline for that family.

Authored record identity, execution identity, file provenance and event ordering
are separate concepts. A new read does not renumber records. The workspace
generation, source revision and mutation receipt revision described in the
[application identity map](application.md#identity-map) remain independent too.

## Verification

Focused owner and consumer tests cover initial failures, retained values after
failure, recovery, absent and empty stores, wrong identity and malformed data,
execution-specific changes and deletion, unchanged-metadata retries, feed
ordering, gaps, duplicates, reconnects and reads while attached. Checkpoint and
selection tests require current reads. Retention tests verify that workflow
snapshots preserve conversation records and identity. The isolated native MCP
fixture corrupts persisted roadmap and execution files while connected, requires
repeated read refusals, restores the exact bytes and verifies recovery with
unchanged authored records.

These checks exercise the existing application seam. They do not establish
workflow-browser visuals or accessibility; those require verification when the
desktop exposes those views.
