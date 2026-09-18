# Durable operations

`arch-13-durable-operations` makes Magistr's own multi-step work recoverable
without replacing any store. Native workflow storage stays the authority on
native state; the peer journal stays the authority on peer reservations; the
cost ledger stays a history. What this adds is a record of the operations a
run dispatches, written before each effect and settled after it, so a process
that dies between the two leaves recovery a question to ask rather than a
guess to make.

## Records and outcomes

The domain owns the vocabulary
([`workflow::operation_*`](../../crates/magistr-domain/src/workflow/operation_ledger.rs)):

| Type | Meaning |
| --- | --- |
| `OperationKind` | The effect family: `Reservation`, `NativeMutation`, `Receipt`, `Publication`, `ProviderDispatch` |
| `OperationRecord` | One attempt of one command: `AttemptId` (command plus attempt number), kind, target, outcome, and `external` — the runtime's own handle, kept apart from the command identity |
| `OperationOutcome` | `Accepted`, `Running`, `Completed`, `Failed`, `Cancelled`, `Indeterminate` |
| `OperationLedger` | The run's records with the admission and transition rules |

A live owner reports forward only: `Accepted` may become `Running` or any
settled outcome; `Running` any settled outcome or `Indeterminate` (a lost
response with the process alive). Settled outcomes latch; repeating one is a
no-op and changing one is refused. `Indeterminate` is reached from
`Accepted`/`Running` by `mark_interrupted` when a checkpoint is recovered, and
left only by `reconcile`, to a settled outcome.

Admission deduplicates only where an outcome is established. A command whose
latest attempt is `Completed` answers `Established` with that record and
nothing is redone. A `Failed` or `Cancelled` latest attempt admits the next
attempt number — a retry is a new attempt of the same command, never the same
external handle. An `Accepted`, `Running` or `Indeterminate` latest attempt
refuses admission until it is settled or reconciled. A command id reused for a
different kind or target is a conflict.

## The checkpoint

The loop checkpoint `.magistr/loop-state.json` carries the run identity
(`run_id`: the native project's id or the directory name, plus a key minted at
open from the clock and the process) and the `operations` ledger beside the
run state. Both are required fields: a checkpoint lacking them is refused like
any other missing stopping evidence, and an explicit new run needs the file
preserved elsewhere or removed. Command keys are scoped `run / attempt cursor
/ key`, so the same key on the next attempt is a new command and deduplication
is per attempt.

[`persist`](../../crates/magistr-core/src/workflow/iteration.rs) is atomic:
the state is written to a temporary sibling, fsynced, renamed over the target,
and the directory is fsynced
([`durable_write`](../../crates/magistr-core/src/workflow/durable_write.rs)).
A crash at any instant leaves the previous checkpoint or the new one, never a
torn file; a stray temporary is ignored and reclaimed. The cost ledger's
whole-file replacement uses the same write. Stop counters, budget observations
and the ledger are checkpointed together at every record transition and at
every settlement.

The attempt's slot is a `Reservation` record, settled `Completed` the moment
it is admitted: the checkpoint that carries it is what reserves the slot, and
a checkpoint that never lands reserves nothing. Reservations are never
refunded; `completed_attempts` advances only on settlement, as before.

## The recoverable attempt

One loop attempt is a sequence of recorded operations, each written before its
effect:

| Step | Kind and target | Written when | Settled by |
| --- | --- | --- | --- |
| Reserve | `Reservation` `attempt:<n>` | pre-iteration check | the checkpoint itself |
| Launch | `ProviderDispatch` `dispatch` (or `dispatch:<delivery>` per delegated delivery) | `Accepted` before connect; `Running` with the runtime's handle `pid:<pgid>@<start>` the moment the transport has spawned, before the session is opened | the turn: ended → `Completed`, closed → `Cancelled`, failed → `Failed`; a session that cannot be opened → `Failed` |
| Record | `Receipt` `gate:<name>@task:<task>` | `Accepted` before the gate is admitted; `Running` with `receipt:<id>` — the durable receipt's id — once admitted, before the gate runs | the ledger's settlement: applied → `Completed`, stale or refused → `Failed`; an admission that fails → `Failed` |
| Mutate | `NativeMutation` `start_chunk:<id>`, `complete_chunk:<id>`, `set_blocked_by:<id>`, `set_status:<id>:<status>` | before the handler is called | the handler's answer |
| Announce | `Publication` `frame:<kind>:<id>` | before the frame is posted | the post |

An intent that cannot be written is not dispatched for a provider launch or a
native mutation. A gate whose intent cannot be written is admitted and
recorded regardless, with a note: its evidence is worth more than the record
of recording it, and recovery reads the ledger either way. An `Established`
intent performs nothing: a dispatch established by recovery runs the gates and
the judge over the recovered work instead of launching a second runtime; a
receipt already applied for the attempt is not admitted again (a second,
unsettled receipt in front of the applied one would refuse the completion);
a mutation is not reapplied.

The runtime handle is a [`RuntimeHandle`](../../crates/magistr-core/src/workflow/runtime_handle.rs):
the process (group) id the transport spawned and the start identity the host
reports for that process ([`process_start`](../../crates/magistr-core/src/workflow/process_start.rs):
`proc_pidinfo` on macOS, `/proc/<pid>/stat` on Linux), observed while it lives.
It names that particular process, not whatever holds the id later. Every loop
records it: the cockpit driver right after the transport connects, the headless
loop from its supervisor's provider pid, and the plain loop through the
`launched` callback `cmd_run_workflow` calls as soon as its session is started.
The cockpit's `magistr next` and headless `magistr next` run one turn with no
loop checkpoint and record nothing new.

## Explicit loop requests

The plain, headless and cockpit `next-loop` commands all call
`IterationLoop::start_or_recover`. A settled terminal run is archived in
`.magistr/loop-history` before the caller persists a fresh checkpoint with
the newly requested iteration limit. Its budget and operation records start
fresh; its selected unfinished roadmap item remains selected. The archive
keeps the previous run identity and its settled records.

A paused or unfinished run retains its identity, limits, budget observations
and operations. Malformed checkpoints and terminal runs with unresolved
effects refuse a fresh start without replacing the saved checkpoint.
Archival failure also leaves the previous checkpoint in place.

Starting a command does not delete `current-plan.md` or `current-task.md`:
even a cancelled run can have unfinished work. Their existing cleanup stays
at the iteration boundary. Refused recovery therefore preserves both the
checkpoint and the context needed to continue it.

`tests/next_loop_recovery.rs` executes all three command handlers in isolated
child processes and checks their checkpoint, archive and saved context.
The cockpit startup fixture stops at terminal admission using piped input;
the loop-driver fake-terminal/ACP fixture verifies the subsequent fresh
opening prompt and displayed iteration limit.

## Recovery and reconciliation

`open_or_recover` reads the checkpoint, marks every `Accepted`/`Running`
record `Indeterminate`, and reconciles each against the authority on its
effect. A dispatch's persisted launch phase also establishes whether the transport could have spawned:

| Kind | Authority asked | Established outcome |
| --- | --- | --- |
| `Reservation` | the checkpoint carrying it | `Completed` |
| `NativeMutation` | the chunk as the native store holds it now (`roadmap_chunk` after reload) | `Completed` when the verb's effect is visible (`start_chunk`: in progress or done; `complete_chunk`: done; `set_blocked_by`: a blocker; `set_status`: that status), else `Failed`; an unknown verb stays open |
| `Receipt` | the durable receipt the record names (`ship_receipt` by the `receipt:<id>` handle) | `Completed` when that receipt is settled and applied; `Failed` when it is unsettled, settled stale, absent from the ledger, or the record carries no id — nothing was applied, since application needs the live loop's settlement, which always follows the persisted `Running`; a receipt naming another gate or task than the record does stays open. No check of the same name on the task is consulted |
| `Publication` | none needed | `Cancelled`: attached surfaces reseed from the store, where the announced mutation already is |
| `ProviderDispatch` | the particular runtime the handle names (`is_live`: the id alive and holding the recorded start identity), then the attempt's Git baseline | `Failed` when its phase is `pending` and it has no handle; open without a handle when its phase is `started` or absent on an older checkpoint; open while the runtime is alive or the host cannot say; once it is gone — or its id is held by another process — `Completed` when HEAD advanced from the baseline (the baseline is an ancestor of HEAD, `git merge-base --is-ancestor`), else `Failed`. A reset, a rebase or a branch switch is movement, not an advance |

What stays open refuses the next attempt: `pre_iteration_check` answers
`Unreconciled` with the records, the loop driver ends with
`LoopEnd::Unreconciled` and the headless and plain loops return an error
naming them. No terminal reason is latched — the run is waiting, not stopped —
so the same command retries once the runtime is gone or an operator records
its outcome with `magistr resolve-dispatch`. A `Failed` dispatch keeps the attempt's
Git baseline; its retry is attempt number two of the same command. Without
engines (a yaml project, or an attached reader) a native mutation or receipt
cannot be asked about and stays open. A dispatch recorded before this
generation (`pid:<pgid>` with no start identity) still parses: its liveness
is the id's alone.

Receipt compaction preserves exact IDs referenced by unresolved operations in the
active checkpoint, including receipts already settled in the workflow store.
It reads those references under the ledger lock, after pinning the current
objective; the loop must persist its reference before receipt settlement.
See [receipt retention](project-authority.md#receipt-retention). A corrupt or
unreadable checkpoint refuses compaction.

## Resolving an interrupted agent

Every new dispatch starts with `dispatch_phase: pending`. The ACP transport
calls a fallible checkpoint callback immediately before spawning, recording
`started` and `Running` before a provider can exist. The phase survives
interruption and repeated recovery. The handle is attached afterward without
replacing an existing runtime identity. All plain, headless, cockpit and
delegated loop launch paths use that boundary; checkpoint failure prevents
spawn. Older checkpoints have no phase and remain conservatively unknown.

Inspect saved work with `magistr workspace doctor . --json`. After checking
that the loop and every agent it started have stopped, resolve one attempt
using the run, command and attempt reported there:

```sh
magistr resolve-dispatch . --run run-… --command 0/dispatch --attempt 1 \
  --outcome failed --reason "The agent stopped before completing the work." \
  --confirm-stopped
```

`failed` and `cancelled` allow the next attempt of the same command.
`completed` suppresses another dispatch for that command; it does not complete
a roadmap item or manufacture gate evidence. The explanation is retained as
`resolution` on that record. Other records, run limits, budget, terminal status,
Git baseline and saved plan/task files are preserved.

The command refuses a changed run ID, an unknown attempt, a non-dispatch or
already settled record, a blank explanation, and a known live loop or agent.
An OS-held `.magistr/loop-owner.lock` excludes all three loop command entries
while resolution reads and atomically replaces the checkpoint. The lock file
is stable and never deleted; process exit releases the lock. Older loop clients
do not take this lock, so the saved owner PID is checked too. An absent handle
cannot establish that an unrecorded agent stopped: `--confirm-stopped` is the
operator's assertion after checking it, not automatic process settlement.
No provider is killed and no workflow server is opened by this command.

## Diagnostics versus evidence

`magistr workspace doctor <root>` reads the checkpoint as it lies on disk
(`IterationState::read`: parsed and validated, never marked, reconciled or
written back) and reports it as the `operations` section: for every record
the project and run from the run key, the attempt's command and number, the
kind, target and external handle, a receipt's gate, task and receipt id, a
dispatch's runtime pid and start identity with whether that runtime is
alive now, the workspace's canonical root and durable generation, and the
loop process the run key names with whether it is alive (pid evidence only:
the key is not pinned to a start identity). Four facts are stated per
record: *cancellation requested* (the run has latched `cancelled` or
`aborted`), *terminal observed* and *effects settled* (the outcome is
settled), *ownership released* (a dispatch's runtime is gone; otherwise the
loop process is gone; `null` when the host cannot say). Each record gets a
state in recovery's own words: `settled`; `live` (an unsettled record owned
by a loop that is still running); `stalled` (the loop is gone and the
runtime it dispatched is alive — the next loop start waits for it,
`Unreconciled`, and admits nothing meanwhile; an unhealthy finding that
says to wait for or end that runtime, since nothing is signalled or
replayed for you); `recoverable` (the loop and any runtime are gone — the
next start reconciles it against the authority named for its kind);
`indeterminate` (a started or legacy dispatch with no runtime handle, or a
host that cannot say; requires checking the runtime and explicit resolution).
A pending dispatch with no handle is recoverable without launching again
during recovery: it settles Failed. The report includes launch phase and any
operator explanation. Session and turn identities are
declared *not persisted*: a provider session and its turns exist only in
the live host's session feed and end with it; live clients are the
`owner` section's connections.

The section is a diagnostic, not evidence. It is computed on request,
retained nowhere and decides nothing: it never establishes an outcome,
never advances a record and cannot be cited by a completion. The evidence
is the checkpoint itself, the receipts it names and the native store, each
with its own retention — the checkpoint until the run is set aside,
receipts under [receipt retention](project-authority.md#receipt-retention),
the store for good — while a doctor report or bundle lives exactly as long
as its reader keeps it, and a bundle passes the same redaction as every
other member. What the doctor explains as stalled or recoverable is what
the next loop start will find; that start performs automatic reconciliation,
while `resolve-dispatch` records the operator's explicit decision.

Fixtures: `doctor_operations.tests.rs` (a lost loop with a live runtime is
stalled, then recoverable once the runtime is killed, beside settled,
receipt and handle-less rows, with the checkpoint left unmarked; a running
loop owns its unsettled rows; a torn checkpoint is an unhealthy finding)
and `tests/workspace_cli_verbs.rs::doctor_explains_stalled_then_recoverable_operations_and_a_legacy_marker`
against the binary.

## Per-family atomicity

| Family | Store | Atomic unit | Cross-family atomicity |
| --- | --- | --- | --- |
| Loop run (run state, stop counters, budget, operations) | `.magistr/loop-state.json` | one whole-file atomic replacement | none with any other family; the ledger records the run's side of every effect |
| Native workflow (roadmap, ship, think, signal) | think-and-ship's own per-family files, written by the library | one family write | none across families; the library does not commit a mutation and its receipt together, so the ledger marks the operation and recovery reads the family back |
| Cost history | the machine's cost ledger file | one whole-file atomic replacement | none; a session's spend is written after its terminal result and is bookkeeping, never a stop rule |
| Cockpit checkpoint spine | in-memory `SessionSnapshot` chain | one snapshot | none; it is the session's own ledger, not run persistence |
| Peer journal | its own versioned journal | one accepted transition | none; peer reservations, unsettled worker attempts and receipts stay there and are not loop-checkpoint fields |
| Publication | none | a frame delivered to attached surfaces | none; surfaces reseed |

Nothing is written twice as two authorities. The ledger says what the run
dispatched and what it saw; the native store says what a mutation came to;
Git says what a runtime left; the runtime's process table says whether it is
still running.

## What is not proven

A record, a receipt or an atomic checkpoint does not make an external effect
exactly-once. A provider may have edited files, run tools or spent tokens
before it died with nothing committed; that attempt is `Failed` for the loop
and retried, and the retry's runtime starts on whatever tree it left. A
runtime that outlived the loop is never signalled — it is waited for. The
handle pins the runtime to one process on hosts that report a start identity;
elsewhere a live id is evidence, not proof. Git attributes nothing to a
process: a commit that descends from the attempt's baseline, made by someone
else after the runtime died, reads as the runtime's. The window between the
transport's spawn and the handle's checkpoint is real: a crash inside it leaves
a started dispatch with no handle, which stays open until an operator checks
the runtime and resolves the attempt. A crash before the persisted pre-spawn
marker leaves a pending dispatch that safely retries; a crash after the marker
can still leave an orphaned runtime whose identity was never saved. A native
mutation that landed a second before the crash and one that never landed read
the same way to recovery: by the store's current state, not by the moment. The
peer journal's stronger permit rules are documented in [peers](peers.md) and
are separate.

## Verification

Crash injection is in-process: each test writes the intent a live loop would
have written, performs the effect or not, drops the loop without settling —
what a killed process leaves on disk — and recovers
([`iteration_operations.tests`](../../crates/magistr-core/src/workflow/iteration_operations.tests.rs)):
reservation (a torn temporary beside the checkpoint; the previous checkpoint
recovers untorn), native mutation (applied → `Completed` and deduplicated; lost
→ `Failed` and retried as attempt two; every verb read back; an unknown verb
stays open), receipt (admitted and never settled → `Failed` even with a
same-named applied check on the task; applied → `Completed`; settled stale
after the objective rolled over → `Failed`; no id → `Failed`; a record naming
another gate's receipt stays open), publication, and provider dispatch against
a real child process (open while alive, even after a commit; `Completed` once
gone with HEAD advanced from the baseline, and then `Established` for the
attempt; a launched runtime that died leaving no advance → `Failed` and
retried as attempt two; pending with no launch → Failed and attempt two; started or legacy without a handle → open whatever HEAD did; the id reused
under another start identity → gone, and HEAD moved back to an ancestor is no
advance). The loop driver's suite checks that a judged iteration leaves every
kind settled in the checkpoint under one run with every receipt record naming
an applied receipt, and that a checkpoint whose dispatch is still owned by a
live runtime ends the loop `Unreconciled` with no launch and no terminal
reason. The domain suite covers the transition table and the admission rules.

```sh
cargo nextest run -p magistr-domain -E 'test(operation)'
cargo nextest run -p magistr-core -E 'test(iteration) | test(durable_write) | test(process_alive) | test(runtime_handle)'
cargo nextest run -p magistr-cli -E 'test(a_judged_iteration_leaves) | test(an_unreconciled_dispatch)'
```

Transport tests execute a child that observes its pre-spawn marker and prove
checkpoint refusal prevents spawn. Driver fixtures verify the recovered retry
number and persisted phase before connecting. Executable CLI tests cover all
three loop entries, every explicit resolution outcome, confirmation and
identity/liveness refusals, lock exclusion and saved-context preservation.

Verification is focused on affected domain, core, ACP and CLI behavior,
all-target compilation and Clippy, formatting and relevant architecture and
custom gates. This change does not alter rendering, goldens, motion or roadmap
export policy.
