# Project authority

Every native writer in Magistr is admitted by a `ProjectAuthority` before it
can open the think-and-ship engines. The type lives in `magistr-engine`
(`tas::ProjectAuthority`), not in the MCP transport: the MCP service, the
cockpit, the loops and one-shot commands all admit through it. It owns the
project's native identity, the writer lease for one role, the live feed when
this writer publishes one, and — once, by consuming itself — the opening of the
engines.

## Capabilities

The read and write capabilities are separate types.

| Capability | Type | Admission | Holds |
| --- | --- | --- | --- |
| Read | `NativeReader` | None. `NativeReader::open` reads the store's locked merge-on-save files and `reload_*` re-reads them when `store_stamp` moves. | Queries, reloads, the store stamp, the reasoning archive. |
| Write | `NativeEngines` | Only `ProjectAuthority::open`. Every clone carries the lease; the role's lock is released with the last writer, never with a reader. | Typed commands over the reader (`Deref`), each a short critical section on one engine lock. |
| Transport | `UnifiedService` | Only `ProjectAuthority::serve`, which composes the four family services inside the engine crate. | The MCP verbs; the service keeps the authority beside them. |

No raw engine, mutable callback or independent constructor is reachable from
outside `magistr-engine`. The escape hatches `with_roadmap`, `with_think`,
`with_ship` and `with_signal` are gone; the verbs their callers needed
(`signal_research`, `roadmap_propose_title`, `roadmap_propose_status`) are typed
commands.

## Roles

A lease belongs to a `WriterRole`, and one writer of each role is admitted per
project and store:

| Role | Who | Lock |
| --- | --- | --- |
| `Service` | `magistr mcp-serve`: the agent's tools, and the only publisher of the live feed. | `<data_dir>/roadmap/sessions/<project_id>.mcp-owner.lock`, or `<project_root>/.magistr/mcp-owner.lock` without persistence. |
| `Host` | The cockpit's `run`, `next` and `next-loop`, the plain and headless loops. | `<data_dir>/roadmap/sessions/<project_id>.host-owner.lock`, or `<project_root>/.magistr/host-owner.lock` without persistence. |

The two roles coexist by design: a cockpit and the `magistr mcp-serve` its
agent starts are the normal pair of writers on one store. A second writer of
the same role — another cockpit or loop on the same project, another MCP
service — is refused with `WouldBlock` before any engine opens. The cockpit
loop attaches to the loop driver's admitted writer rather than opening a
second one. The bare cockpit, the launcher and `magistr roadmap
show|next|status|export` are observers: they open a reader and run beside any
host.

## Lock identity

The lock path comes from the native persistence identity, so copied project
identities using one store contend, distinct project/store pairs are
independent, and filesystem aliases contend on the same inode.

Lock files are never truncated, removed, or replaced. The operating system
releases the lock when its last handle closes or the process exits. A leftover
file is normal; deleting it while an owner runs can allow a second inode and
defeat exclusion.

## Event sockets

`ProjectAuthority::publish` locks `<socket>.mcp-owner.lock` beside the project
lock, so different stores cannot race on one publisher path. A bounded
connection probe keeps a live listener intact. Only a missing or
connection-refused endpoint proceeds to binding; a probe timeout, another
connection error or a bind failure refuses startup and releases the lease. A
stale socket is reclaimed without deleting its lock file.

Unix CLI startup requests the project feed by default. Explicit configurations
can omit broadcasting. Non-Unix startup keeps a no-feed default, and an explicit
Unix-socket request returns an unsupported error.

## Cached state and reload

Native engines cache state in memory. A persistence merge can retain another
writer's additions on disk without refreshing the caller's snapshot. Reload
methods on the reader update the roadmap, ship and signal views; the cockpit
reloads when its detached-feed timer observes a changed store stamp, and the
loop reloads the roadmap at every iteration boundary.

Reloading is separate from command authority. It neither admits a second writer
nor authenticates a client.

## Limits

Admission is cooperative among Magistr binaries that take the lease. Older
binaries, standalone think-and-ship and direct edits of the store or its lock
files do not participate. Advisory locks do not protect against deliberate
modification.

Proxy attachment, client authentication, generation fencing and shared writer
routing are separate work. The lease must not be described as a distributed
authority or a governed peer runtime; a `Revision` is still comparable only
within one authority's history, and admission does not make cached reads
coherent across the two roles.

The event publisher can outlive a dropped service inside a running async
runtime. Admission continues to refuse a still-live endpoint; releasing an
owner lease alone does not establish that its publisher has shut down. Stop the
owning runtime or process normally before reconnecting.

## Verification

Engine unit tests (`tas::project_authority::tests`) cover role coexistence,
same-role refusal, the lease living with the last writer clone while readers
hold nothing, the stateless lock under the project root, lock I/O failure
before any engine opens, and the publisher probe. The cockpit's
`engine::client::tests` cover a refused second host, an attached writer and an
observer beside it. The CLI integrations `mcp_service_owner` and
`mcp_project_owner` cover lock lifetime, contention, aliases, independent
stores, stale sockets, process exit, a host admitted beside a live service, and
a real `magistr roadmap status` process answering beside a running stdio owner
without a lease.
