# Projections

A projection is what a surface shows of the native think-and-ship store: the
cockpit's rail, roadmap, trace and dock, the loop driver's view of a running
iteration, the guard that decides whether a phase may be entered. Projections
read stored and live facts; they are not another workflow authority. This
document names the one producer of those facts, the typed shapes it produces,
the identities that travel with them, and what a consumer may conclude from
each.

## One producer

`magistr_engine::native` is the only producer of workflow facts for every
surface. It offers two reads of the same store:

| Read | Type | Source | When |
| --- | --- | --- | --- |
| Snapshot | `NativeSnapshot` | `NativeReader::snapshot()` — each family read whole under its own engine lock | before the feed speaks, on the re-seed timer while no feed is attached, and whenever the feed asks for one |
| Live feed | `NativeEvent` with an optional `EventCursor` | the owner's NDJSON broadcast socket, decoded by `NativeFrame::decode` | every mutation the owning writer makes, as it makes it |

Both are built from the same typed records (`Chunk`, `Signal`, `ThinkStep`,
`Objective`, `Task`, `Action`, `Check`) and the same family frame enums
(`RoadmapFrame`, `SignalFrame`, `ThinkFrame`, `ShipFrame`). A fifth family,
`MagistrFrame`, carries the facts no store family records: a running gate's
pty output and the phase the agent announced through `magistr_set_phase`.

`NativeFrame` is the wire line. It is parsed and decoded at the transport edge
(the cockpit feed task, the remote relay) and nowhere else: no view, model,
guard or host reads a `family` or `type` string, digs a JSON payload, or infers
a phase from the words of a recorded action. The phase travels typed as
`MagistrFrame::PhaseSet { phase: WorkflowPhase }`; the action the MCP service
also records on the ship trace is narrative.

A process's own mutations return through its engine port as the typed event the
owner would have announced, delivered `Delivery::Local`, so a rail, dock or
roadmap folds a local answer through the same path as a remote frame.

## Identities

Five identities travel with projected facts. They are distinct, and no type
combines them.

| Identity | Type | Stamped by | Meaning | Changes when |
| --- | --- | --- | --- | --- |
| Schema version | `schema_version: u32` in the think history file (`SCHEMA_VERSION = 1`) | the think persistence layer | the on-disk shape of a family's file | the file format changes; a history at another version is skipped, never merged |
| Owner epoch | `epoch: u64` on every feed line (`OwnerGeneration`) | the publishing owner's role lease | which admitted writer emitted the frame | a new writer of that role is admitted |
| Aggregate revision | `updated_at` on `Chunk` and `Signal`; timestamps on ship actions and checks | the family engine at the write | the last write to one record | that record is written |
| Event sequence | `seq: u64` on every feed line | the publisher's broadcaster, one counter shared by all its family handles | the position of one frame in one owner's emission order | every emitted frame, whatever its family |
| Snapshot cursor | `SnapshotCursor { modified, length }` | the store's on-disk stamp, read before the families | where a whole read stood | any family file is written |

Per-family guarantees:

- **think**: the history file carries the schema version; steps are numbered
  per project and the archive may trail the live engine by one unflushed step.
- **ship**: the objective and its tasks are one file; a task and the checks on
  it agree within a snapshot. Actions and checks carry ids and timestamps; there
  is no file-level version.
- **roadmap**: chunks carry `created_at`/`updated_at`; the file carries no
  version and no global revision.
- **signal**: signals carry `created_at`/`updated_at`; the file carries no
  version.
- **across families**: none. Four files, four locks. A snapshot is coherent
  within each family and may straddle a write between two of them; the cursor
  taken before the read moves on such a write and the next re-read catches it.
  No global revision is fabricated over the four.

The workspace generation, source revision and mutation receipt revision of the
[application identity map](application.md#identity-map) are unrelated to these
and stay in their own table.

## Delivery and freshness

The feed classifies every sequenced frame against the cursor it last folded:

| `Delivery` | Rule | Consumer's move |
| --- | --- | --- |
| `Local` | the process's own writer answered | fold |
| `Unsequenced` | the publisher stamped no cursor | fold; nothing to order against |
| `First` | no cursor held yet | fold; baseline |
| `InOrder` | `seq == last + 1`, same epoch | fold |
| `Duplicate` | `seq <= last`, same epoch | skip; the record is already applied |
| `Gap { missed }` | `seq > last + 1`, same epoch | fold, mark stale, request a snapshot |
| `NewEpoch` | the epoch changed | fold, mark stale, request a snapshot |

The cursor is kept across reconnects on purpose: the socket buffers nothing, so
the frames emitted while a subscriber was detached are revealed only by the
first frame after reattaching, as a gap against the cursor kept from before. A
fresh cursor would read it as an innocent first frame.

A consumer never blocks on a gap. It records what it saw, marks itself stale and
asks for a snapshot through the `FeedLink` shared with the re-seed task; the
re-seed reads at once, attached or not, and answers one of two ways. When the
snapshot cursor moved past the one the model was seeded with, it posts a seed
and the model folds it whole. When the cursor did not move, it posts
`EngineEvent::Confirmed { cursor }` — a bookmark, not a read: the store stands
where the last seed left it, so the projection returns to current and no view
is touched (the open detail, the rail and the trace stay as painted, no frame
is counted, nothing is announced). Either answer restores current; ordinary
in-order frames never do, so a gap is concealed by nothing but the store's own
word. Every (re)connect requests a snapshot the same way, so an observer
cockpit that attached late — or one that reattached over an unchanged store —
is covered without a command of its own. The re-seed's baseline is the cursor
of the snapshot the model was seeded with, handed to it by the entry point, so
a confirmation is always relative to what the model actually holds.

`Projection` on the cockpit model holds the resulting freshness:

| `Freshness` | Meaning |
| --- | --- |
| `Unavailable` | no snapshot and no feed yet |
| `Current` | the last seed, confirmation or in-order frame is what the store holds |
| `Stale` | a gap, a new epoch or a hangup was seen and neither a snapshot nor a confirmation has landed since |

A rebuild is deterministic: folding the same seed then the same in-order frames
yields the same model, and folding a duplicate changes nothing.

## Offline fallback

With no feed attached the re-seed timer reads the store on its interval and
posts a seed only when the cursor moved; an unchanged store on a timer wake
posts nothing — not a seed, not a confirmation — so an idle cockpit stays idle
and paints nothing. A surface therefore always shows the last coherent read; it
does not synthesize events from timestamps or guess a phase from action
descriptions. The loop driver seeds each iteration from a
fresh snapshot and observes completions as typed roadmap events.

## Verification

Companion suites pin the contract: ordering, duplicates, gaps, reconnect and
re-snapshot on the feed cursor and the feed task; the snapshot's per-family
coherence and cursor on the reader; deterministic rebuild, duplicate folding
and the three freshness states on the projection, with the confirmation that
restores current after a hangup, a gap or a new owner and the in-order frames
that do not; the offline re-seed, the feed's snapshot request and its
confirmation over an unchanged store measured against the seeded cursor on the
re-seed task; the reconnect-with-unchanged-store and owner-epoch-change folds
on the model, with their selection untouched; a live broadcaster round trip
through `NativeFrame::decode`; and, in the CLI, that gate output, phase
announcements and chunk completions reach hosts typed. A structural check
keeps `payload[` digs and phase-from-description parsing out of production
cockpit code.
