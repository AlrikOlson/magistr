# Project authority

Every native workflow writer in Magistr is admitted by a `ProjectAuthority` before it
can open the think-and-ship engines. The type lives in `magistr-engine`
(`tas::ProjectAuthority`), not in the MCP transport: the MCP service, the
cockpit, the loops and one-shot commands all admit through it. It owns the
project's native identity, the writer lease for one role, the owner
generation that lease mints, the live feed when this writer publishes one,
and — once, by consuming itself — the opening of the engines.

A surface that finds its role already held does not open a second writer. It
attaches to the running owner's endpoint through a local application client
and acts through that owner's writer, or it fails closed. Attachment is local:
one user, one machine, Unix domain sockets.

The [workspace owner](workspace-owner.md) uses the same local endpoint and
handshake implementation with a distinct canonical-root identity. It owns
`WorkspaceService`, not a workflow writer role or workflow store.

## Capabilities

The read and write capabilities are separate types; the attached client is a
third, holding neither.

| Capability | Type | Admission | Holds |
| --- | --- | --- | --- |
| Read | `NativeReader` | None. `NativeReader::open` reads the store's locked merge-on-save files and `reload_*` re-reads them when `store_stamp` moves. | Queries, reloads, the store stamp, the reasoning archive. |
| Write | `NativeEngines` | Only `ProjectAuthority::open`. Every clone carries the lease; the role's lock is released with the last writer, never with a reader. | Typed verbs over the reader (`Deref`), carried out by native workflow use cases (see [One write path](#one-write-path)); `apply` dispatches the versioned `OwnerCommand` to the same implementation. |
| Transport | `UnifiedService` | Only `ProjectAuthority::serve`, which hands the same four family services the writer acts through to the wire, and the writer beside them. | The MCP verbs; the service keeps the writer and the authority beside them. |
| Attached | `OwnerClient` | A welcomed handshake on the owner's endpoint. No lease, no engine. | One connection, one command in flight, the owner's generation. |

No raw engine, mutable callback or independent constructor is reachable from
outside `magistr-engine`. The escape hatches `with_roadmap`, `with_think`,
`with_ship` and `with_signal` are gone; the verbs their callers needed
(`signal_research`, `roadmap_propose_title`, `roadmap_propose_status`) are typed
commands.

## One write path

Every supported native mutation runs the first-party workflow application use
case for its family. `ProjectAuthority::open` composes `ThinkUseCases`,
`ShipUseCases`, `RoadmapUseCases`, and `SignalUseCases` once, including the
reciprocal roadmap/signal capabilities. NativeEngines calls their synchronous,
typed methods directly. `ProjectAuthority::serve` wraps those same capabilities
in thin MCP adapters; an attached local client reaches them through the
versioned owner command adapter.

Validation and cross-family sequencing live below transports. Promoting a
signal creates and cross-links its roadmap chunk; empty bodies, blank research
summaries, illegal transitions, and unknown identities receive the same native
refusals. JSON serialization belongs only to transport presentation. The old
`handler_call` bridge, internal MCP argument/result decoding, and one-poll
noop-waker invocation are deleted.

Actual effects are explicit async operations. Ship checks admit a target before
running a command, execute outside the workflow lock, then settle against that
admission. Tracker setup probes its provider without holding the roadmap lock,
then applies the shared native setup policy. Approval waits similarly use the
application capability. CLI and MCP presentation retain their external names
and ordinary success/error envelopes.

The `native-write-path` gate keeps these boundaries executable: Magistr engine
composition is confined to `tas`; raw engine reads belong to `NativeReader`;
native writes and workflow application modules cannot depend on MCP handlers,
wire argument/result types, JSON result decoding, or noop-waker polling.
Magistr MCP code cannot call its own tools. Direct host dependencies remain
limited to `magistr-engine` and `magistr-cli`.

### Identities and check settlement

Native workflow state now carries additive execution identity: an objective run
and a task instance within that run. Existing stores without this metadata
remain readable through deterministic legacy identities; new objectives and
task instances receive fresh identities. Duplicate plan task IDs are refused.
Removed instances are fenced so a stale merge cannot resurrect them.

`TaskAdmission` names the project, run, task ID, and task instance.
`CheckAdmission` binds that target to the check request before an effect begins.
Native `ship_check` retains this admission across command execution inside the
workflow crate; `magistr_gate` and the loop driver retain it through the
[composition boundary](application.md#the-composition-boundary)'s
`AdmittedCheck`, which can also bind the workspace's `SourceSnapshot` to the
same admission. The loop additionally fixes one task admission for the complete
gate set, including retries. Reusing a task name in a later objective cannot
collect an earlier command's result.

Settlement re-reads the current native ship state and compares identity under
the existing persistence lock before appending an ordinary check. A stale
settlement returns its admission and execution outcome intact; the MCP adapter
reports `stale_result`, and host runners report an unrecorded result. This
boundary supplies correlation for the later receipt implementation. It does
not yet durably archive stale results or provide exactly-once settlement.

Ordinary completion policy remains native and compatible: incomplete tasks or
failed required checks produce the inherited finalization warnings. Durable
CheckReceipts, configured evidence freshness, and enforcement at completion
belong to the pending `arch-14-verification-receipts` chunk, which consumes the
workspace's `SourceSnapshot` and `MutationReceipt` capabilities
([owned subsystems](owned-subsystems.md#evidence-requires-source-identity)).
There is no second receipt database or workflow-owned source scanner in this
extraction.

## Roles

A lease belongs to a `WriterRole`, and one writer of each role is admitted per
project and store:

| Role | Who | Lock |
| --- | --- | --- |
| `Service` | `magistr mcp-serve`: the agent's tools, and the only publisher of the live feed. | `<data_dir>/roadmap/sessions/<project_id>.mcp-owner.lock`, or `<project_root>/.magistr/mcp-owner.lock` without persistence. |
| `Host` | The cockpit's `run`, `next` and `next-loop`, the plain and headless loops. | `<data_dir>/roadmap/sessions/<project_id>.host-owner.lock`, or `<project_root>/.magistr/host-owner.lock` without persistence. |

The two roles coexist by design: a cockpit and the `magistr mcp-serve` its
agent starts are the normal pair of writers on one store. A second writer of
the same role is refused with `WouldBlock` before any engine opens; what
happens next depends on the surface (see [Attachment](#attachment)). The
cockpit loop attaches to the loop driver's admitted writer in-process rather
than opening a second one. The bare cockpit, the launcher and `magistr roadmap
show|next|status|export` are observers: they open a reader and run beside any
host.

## Lock identity and generation

The lock path comes from the native persistence identity, so copied project
identities using one store contend, distinct project/store pairs are
independent, and filesystem aliases contend on the same inode.

Lock files are never truncated, removed, or replaced. The operating system
releases the lock when its last handle closes or the process exits. A leftover
file is normal; deleting it while an owner runs can allow a second inode and
defeat exclusion.

Each admission mints an `OwnerGeneration`: a fixed-width counter at the head of
the role's lock file, read and advanced in place under the held lock, so it is
monotonic across processes and restarts without a clock and never shortens the
file. The generation changes when ownership changes, never on a state write.
Every command an attached client sends carries the generation it was welcomed
under; the owner refuses any other with `StaleGeneration`, so a client that
outlived one owner cannot act on the next until it reconnects and re-reads.

## Attachment

### Endpoint discovery

An admitted writer serves an `OwnerEndpoint` on a Unix socket derived from the
same three facts the lease is keyed by — store, project identity and role —
so every process computes the rendezvous and nothing announces it. The socket
sits beside the role's lock file (`<lock>.sock`) when that fits the kernel's
`sun_path`; a store rooted too deep for it gets
`<runtime_dir>/magistr-<digest>.<role>.sock` (the digest is the lock path's,
`runtime_dir` is `XDG_RUNTIME_DIR`, else `TMPDIR`, else `/tmp`), and a runtime
directory that is itself too deep falls back to `/tmp`. A copied project
identity on one store finds the same owner; a different store finds none. The
socket file is mode `0600`, and a live listener at the path refuses a second
endpoint while a socket file nothing answers on is reclaimed.

### Handshake and identity

Both ways the wire is one JSON object per line. The client opens with a
`Hello` naming protocol version 3 (`OWNER_PROTOCOL_VERSION`, exact match
only), its build (the package version; an owner serves only its own build),
an `OwnerIdentity`, the kind of attachment it wants (`commands` or
`mcp`) and its `CallerIdentity` (pid and surface). Workflow identity names
the project and writer role; workspace identity names the canonical root
without a workflow role. The owner checks the peer credential of the socket
first — the connecting process must be the owner's own user, which the
socket's file mode also enforces — and then the version, build, identity and
offered kind, answering `Welcome` with its generation and pid or `Refused`
with the reason. The caller identity is kept
for diagnostics; the credential is the proof.

### What an owner serves

| Owner | Endpoint | Attached surface |
| --- | --- | --- |
| Cockpit `run`, cockpit `next` / `next-loop`, plain and headless `next-loop` (Host) | `OwnerEndpoint::serve` over the writer: typed `OwnerCommand`s — every `NativeEngines` verb, carried out by `NativeEngines::apply` on the host's own writer. | A second `magistr run` on the project: `EngineClient::admit_or_attach` attaches when the host lease is held and its `AttachedEnginePort` sends each cockpit verb to the running host. Reads still come from the attached cockpit's own `NativeReader`, the feed and the re-seed timer. |
| `magistr mcp-serve` (Service) | `OwnerEndpoint::serve_mcp` over the bare admission: an rmcp session of the same `MagistrService`, under the same protocol cap, on every welcomed socket. Typed commands are refused at the handshake. | A second `magistr mcp-serve` on the project: refused the Service lease, it attaches as `mcp` and proxies its stdio to the owner's session, so a second agent session in one project gets the full tool set through the one admitted writer. |

`magistr workspace serve` also uses the MCP endpoint with
`OwnerIdentity::Workspace`, and unlike the workflow proxy its attached
invocation is a bridge that outlives the owner: it answers in-flight calls
with a typed refusal and reconnects — attaching or admitting — on its next
request. Its admission, policy and process lifetime are described in the
[workspace guide](workspace-owner.md).

`next` and `next-loop` on a held host stay refused: execution cycles are
serialized per project by design, and a loop that cannot be the host is not
started as a client.

### Typed attachment on the Service owner (retired)

`arch-11b-service-owner-commands` asked the Service owner to serve typed
`OwnerCommand`s so that a one-shot CLI write could attach to a running
`magistr mcp-serve`. Its premise was that `ProjectAuthority::serve` composed
the family services without a writer over the same engines; that has not
been true since the one write path: `serve` returns the `UnifiedService` and
the `NativeEngines` over the very same family services, and `MagistrService`
holds that writer for its own gate and phase records. Its consumer never
existed either: every Host admission in the binary is a loop or the cockpit,
and `magistr roadmap show|next|status|export` are readers. `arch-14f` therefore
retired the requirement rather than manufacture a command or a second
protocol. The Service endpoint keeps refusing `commands` at the handshake; a
real one-shot writer, if one is ever added, attaches through the existing
`OwnerEndpoint::serve` path the Host already uses.

### Process lifetime

The owner is an embedded host, alive for its CLI or session invocation — not a
daemon. The endpoint lives exactly as long as the value that serves it: the
cockpit run, the loop, the stdio MCP session. It is dropped after the runtime
and before the writer, so the owner stops taking clients, hangs up every
attached connection, unlinks its socket, and only then releases the project.
Nothing an attached client does keeps the owner alive, and the owner never
waits for a client. Attached surfaces see EOF: the attached cockpit's port
reports the owner gone and reconnects on its next verb (to whoever owns the
project next, refusing once when the generation changed); the attached
`mcp-serve` ends with an error telling the client to start a new server, which
then takes the lease itself.

### Detach

A client detaches by sending `Detach` and closing, or simply by closing. The
owner cancels nothing on a detach: its own surface and every other attached
client continue, and a detached client can attach again. Detaching a viewer is
not cancelling a turn and not terminating a session; those stay the session
supervisor's explicit operations.

### Command deadlines

The client is synchronous — the cockpit's engine port runs inside its session
task, and a local round trip is bounded, not awaited — and every round trip has
a deadline (`OwnerClient::DEADLINE`, five seconds by default). A deadline or a
hang-up spends the connection: a late answer could no longer be told from the
next command's, so the client is detached until an explicit `reconnect`, whose
answer says whether the owner is still the one it was reading from.

### Read-only fallback and fail-closed behavior

Attachment never widens into ownership. A surface whose role is held and whose
owner endpoint does not take it fails closed: `EngineError::Occupied` for the
cockpit (carrying the lease's refusal and the attachment's), the lease's
refusal plus the endpoint's for `mcp-serve`. Neither opens a writer. Read-only
is an explicit choice the user makes — the observer cockpit (bare `magistr`,
`magistr roadmap`) and the one-shot roadmap reads open a `NativeReader` and
run beside any owner — never a silent downgrade of a surface that asked to
write.

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
loop reloads the roadmap at every iteration boundary. An attached mutation is
carried out on the owner's cached engines, so the owner's own views are
current; the attached surface sees it through the feed or its next reload.
Within one process the use cases and reader share the same engines. Ship use
cases refresh the current persisted state before operating; admitted settlement
also compares and appends under the file lock. General inherited mutations still
use whole-task merge behavior; this extraction does not claim a transactional
multi-writer store. A settled `magistr_gate` record is visible to the same
server's `ship_status`.

Reloading is separate from command authority. It neither admits a second writer
nor authenticates a client.

## Limits

Admission is cooperative among Magistr binaries that take the lease. Older
binaries, standalone think-and-ship and direct edits of the store or its lock
files do not participate. Advisory locks do not protect against deliberate
modification, and the generation fences attached clients of an owner, not
arbitrary writers of the store.

Attachment is local and unauthenticated beyond the peer credential: there is
no remote endpoint, no daemon that outlives its invocation, no upgrade path
across protocol versions (a mismatch is refused), and no shared writer
routing. The lease must not be described as a distributed authority or a
governed peer runtime; a `Revision` is still comparable only within one
authority's history, and admission does not make cached reads coherent across
the two roles. Off Unix there is no endpoint and no client; both report
`Unsupported`.

The event publisher can outlive a dropped service inside a running async
runtime. Admission continues to refuse a still-live endpoint; releasing an
owner lease alone does not establish that its publisher has shut down. Stop the
owning runtime or process normally before reconnecting.

## Verification

Engine unit tests (`tas::native_engines::tests`) cover the one write path:
promotion from the in-process writer creating the backlog chunk and linking
both ways, idempotently; the handlers' refusals — an empty body, a blank
summary, an illegal transition, an unknown task — reaching the caller in the
native error messages; and a fresh objective reporting how many stale tasks
it cleared. Imported family use-case tests cover native/MCP valid and invalid
behavior, reciprocal changes, tracker probe concurrency, legacy execution
identity, peer rollover, reused task IDs, and stale settlement. CLI process
tests hold commands across objective rollover and prove that both gate adapters
leave the newer task untouched.
`tas::project_authority::tests` cover role coexistence,
same-role refusal, the lease living with the last writer clone while readers
hold nothing, the stateless lock under the project root, lock I/O failure
before any engine opens, and the publisher probe.
`tas::owner_endpoint::tests` cover an occupied host reached through its
endpoint with no second lease, a stale generation refused until the client
reconnects, a detach that cancels nothing for the owner or another client, a
silent owner tripping the command deadline, the handshake refusing another
protocol, project, role or kind, owner shutdown leaving the client without a
writer and the role free, the MCP hand-off after the welcome, an MCP-only owner
refusing typed commands, a live endpoint refused and a stale socket reclaimed,
and generations growing per role; `tas::owner_generation::tests` and
`tas::owner_socket::tests` cover the counter and the derived path. The
cockpit's `engine::client::tests` cover a refused second host, an attached
writer, an observer beside it, a second cockpit acting through the running
host, a held host without an endpoint failing closed, and an attached port
reporting a gone owner without opening anything. The CLI integrations
`mcp_service_owner` and `mcp_project_owner` cover lock lifetime, contention,
aliases, independent stores, stale sockets, process exit, a host admitted
beside a live service, a real `magistr roadmap status` process answering beside
a running stdio owner without a lease, and a real second `magistr mcp-serve`
attaching to the owner, writing through it, and ending when the owner is
killed. `mcp_serve_integration` covers a green `magistr_gate(all)` leaving
one verified check per gate on the named task and opening `done`, and a gate
running exactly once with or without a task; the loop driver's tests cover a
judged completion naming the task its gates were recorded on.
