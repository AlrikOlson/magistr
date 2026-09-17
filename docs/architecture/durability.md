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

## Recovery and reconciliation

`open_or_recover` reads the checkpoint, marks every `Accepted`/`Running`
record `Indeterminate`, and reconciles each against the authority on its
effect, never against the record:

| Kind | Authority asked | Established outcome |
| --- | --- | --- |
| `Reservation` | the checkpoint carrying it | `Completed` |
| `NativeMutation` | the chunk as the native store holds it now (`roadmap_chunk` after reload) | `Completed` when the verb's effect is visible (`start_chunk`: in progress or done; `complete_chunk`: done; `set_blocked_by`: a blocker; `set_status`: that status), else `Failed`; an unknown verb stays open |
| `Receipt` | the durable receipt the record names (`ship_receipt` by the `receipt:<id>` handle) | `Completed` when that receipt is settled and applied; `Failed` when it is unsettled, settled stale, absent from the ledger, or the record carries no id — nothing was applied, since application needs the live loop's settlement, which always follows the persisted `Running`; a receipt naming another gate or task than the record does stays open. No check of the same name on the task is consulted |
| `Publication` | none needed | `Cancelled`: attached surfaces reseed from the store, where the announced mutation already is |
| `ProviderDispatch` | the particular runtime the handle names (`is_live`: the id alive and holding the recorded start identity), then the attempt's Git baseline | open without a handle (nothing says whether a runtime exists or existed: the intent is written before the launch), open while that runtime is alive or the host cannot say; once it is gone — or its id is held by another process — `Completed` when HEAD advanced from the baseline (the baseline is an ancestor of HEAD, `git merge-base --is-ancestor`), else `Failed`. A reset, a rebase or a branch switch is movement, not an advance |

What stays open refuses the next attempt: `pre_iteration_check` answers
`Unreconciled` with the records, the loop driver ends with
`LoopEnd::Unreconciled` and the headless and plain loops return an error
naming them. No terminal reason is latched — the run is waiting, not stopped —
so the same command retries once the runtime is gone, or the checkpoint is
explicitly set aside for a new run. A `Failed` dispatch keeps the attempt's
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
a dispatch with no handle, which stays open until the checkpoint is set aside
by hand, and possibly an orphaned runtime nobody is waiting for. A native
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
retried as attempt two; no handle → open whatever HEAD did; the id reused
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

Formatting and every required gate in `.magistr/project.json` remain
mandatory. This chunk changes no rendering, goldens, motion rules or roadmap
export policy.
