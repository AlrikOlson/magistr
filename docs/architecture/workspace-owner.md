| 21: unreachable owner | The bridge serves discovery and typed unavailability while an owner is unreachable, then retries admission on the next request. Unknown markers refuse without modification. | Each request has a bounded attachment deadline; effects are never replayed. |ner

Every Magistr workspace client reaches one admitted native owner. The public
`magistr mcp --dir <root>` endpoint composes the workflow and workspace bridges.
Its workspace bridge speaks MCP to the admitted workspace owner over a local link.
The first invocation admits the workspace and owns the
`WorkspaceService` in-process; later invocations attach to it over Magistr's
shared local owner endpoint. Since `arch-14f2` a `magistr run` or cockpit
loop admits the workspace itself before its agent starts
(`magistr::application::HostedWorkspace`) and serves the same endpoint, so the
agent's bridge and the cockpit's port attach to the host and the host's own
authorized source writes run through the service in-process; when a live owner
already holds the lock — or the root cannot be admitted — the host admits
nothing and its sessions' source writes are refused
(`magistr::application::WorkspaceAdmission`, `audit-260912-04`), never written
directly. No external executable, checkout or runtime is
consulted: `arch-14d-workspace-owner` removed the Bun/PATH/home-checkout
resolver, so the launch line is this binary or nothing.

## Admission and rendezvous

`WorkspaceService` holds the exclusive OS lock on `.magistr/workspace/server.lock`
before opening the key, journal, index or worker. Magistr also supplies a stable
lifecycle lock at the physical Git working-copy root. The source service holds
that shared lock until it closes, including when its source root is a nested
project. Archive takes the same lock exclusively and refuses active source or
workflow sessions. [Working-copy lifecycle](project-authority.md#working-copy-lifecycle)
owns archive and reopen; source roots, journals and generations remain independent.
Its immutable marker and durable `.magistr/workspace/server.generation` enforce
exclusion and fencing. Only contention on a valid marker permits attachment.
Empty or unrecognized markers refuse admission without interpreting process IDs,
moving records or opening stores. Recovery restores known-good workspace state
offline. No process takes ownership from a live holder or resets its generation.

`OwnerIdentity::Workspace` names the canonical root independently of workflow
project identity, store and writer roles. Aliases resolve to the same owner;
distinct roots, including separate worktrees, have independent services and
journals. The shared [project owner protocol](project-authority.md#attachment)
uses `Hello` version 6, which carries a
[`BuildIdentity`](project-authority.md#attachment) for diagnostics and typed
native attachment. MCP connections are opaque proxies: the owner alone supplies
the tool definitions and runs their handlers, so a rebuilt proxy attaches to
the running owner without opening another service. Exact protocol, canonical
root and peer-user checks still apply. Typed command and probe connections
retain exact build admission. Workspace endpoints accept MCP connections, not
workflow commands. Socket peer credentials restrict attachment to the owner's
operating-system user.

The endpoint is `<root>/.magistr/workspace/run/owner.sock` when the path fits the Unix
socket limit; long roots use
`/tmp/magistr-workspace-<uid>/<root-digest>.sock`. The containing directory is
private (`0700`) and the socket is `0600`. A live endpoint is preserved; only
an admitted owner may reclaim an abandoned socket. Contenders share a
five-second attach-or-admit budget while an owner opens its service and
publishes the endpoint; each connect and greeting exchange has one absolute
deadline, capped at 250 ms per attempt. Startup with no reachable owner and no
admissible workspace never opens a second service. When the refusal is of the
temporary kind (`EX_TEMPFAIL`, 75: the lock is held by an owner that has
published no endpoint, or the marker is unrecognized) the bridge process
does not exit either: it serves discovery itself, as described under
[the bridge](#the-bridge), and reconnects on its next request. Every other
refusal — a missing root (66), durable data written by a newer binary (65),
`--config` overrides against a live owner (a policy refusal) — is still a
startup error with its exit code, since no later request can outlive it.
Windows workspace attachment uses the existing named-pipe listener and
canonical root identity, restricted to the current user's SID. Other platforms
without a local transport remain exclusive. This workspace path does not imply
Windows workflow MCP attachment; see [clients](clients.md#limits).

The owner's resolved policy governs all connections. An invocation carrying
explicit `--config` values refuses attachment rather than silently dropping
those values or changing the live owner's policy.

## The bridge

The bridge relays newline-delimited JSON-RPC verbatim in both directions and
looks at three things:

- **Identity.** Every `tools/call` that carries no `_meta["magistr/sessionId"]`
  is stamped with the bridge's durable identity — `MAGISTR_WORKSPACE_SESSION_ID` when
  the launcher set it, else `bridge-<random>` minted once per process. Journal
  rows therefore carry a real identity per client process, `undo` session
  scope means "this client's ops", and that identity survives the owner
  changing. An explicit client identity is forwarded untouched.
- **Initialize.** The client's `initialize` params are recorded. A new owner is
  initialized on the client's behalf under a bridge-owned request id whose
  answer is swallowed, followed by `notifications/initialized`; the client
  never sees a second handshake.
- **Requests in flight and the last listing.** Outstanding request ids are
  tracked, and the last successful `tools/list` result is kept.

**One capability definition.** What the server *is* — the `initialize`
result (tools and the SEP-2663 tasks extension, the instructions, this
binary's name and version, the served protocol eras and the negotiation that
caps 2026-07-28+ to 2025-06-18, echoes a served era and otherwise answers the
default) and the core tool listing — lives in one place
(`workspace_mcp::capability`). The rmcp handler over an admitted workspace
serves from it, and so does the bridge when no owner is reachable, so a
client discovers the same server either way; only plugin tools, which an
owner alone can enumerate, are appended by the handler. A bridge that reaches
its first owner later initializes it on the client's behalf exactly as after
an owner change.

**Embedded owner.** The owning invocation serves its own client the same way,
over an in-process duplex to the rmcp handler, so both paths are one code path
and every client gets a stamped identity.

**No owner reachable.** Whether an attached owner's socket ended or none was
reachable when the bridge started, every request that needs an answer gets
one at once: `initialize` the shared capability negotiated for that request,
`tools/list` the last listing an owner gave or the core definition, a
`tools/call` the retained `ENGINE_UNAVAILABLE` envelope with recovery `retry`
(text and `structuredContent` alike) whose message names the refusal and the
action that lifts it — restoring invalid workspace state offline, or the
timed-out attachment for a lock held without an endpoint — `ping` an empty
result, anything else a JSON-RPC error naming the reason. The tool surface
never disappears and the bridge process never exits on owner loss. The next
request that needs an answer runs one bounded attach-or-admit attempt: it
attaches to whoever admitted the workspace meanwhile, or admits the workspace
itself and becomes the embedded owner for its peers — the embedded-owner
analog of ADR-018's connect-or-spawn, with no daemon. If neither is possible
the request is answered with the typed refusal and the next request tries
again; repaired workspace state is considered on the next request. Nothing that was in flight is replayed: a call
the owner may or may not have applied is answered refused with the
read-back note, the identity the bridge stamps survives, and a confirmation
minted under the lost owner stays fenced by the new generation.

**Client EOF.** When the client closes its side, the bridge half-closes the
link so the owner sees EOF and answers what was in flight; those answers are
relayed within a bounded drain, then an embedded owner shuts down in order
(handler release, endpoint, formatter tracker, index and worker) and an
attached socket is dropped. The owner's own client leaving does not end its
peers: each survivor reconnects on its next request.

## MCP sessions and writes

Every accepted connection gets a fresh `WorkspaceMcpService` over the same
service and its own nonempty random fallback session (rarely selected, since
bridges stamp identities). Native `WorkspaceSession` handles bind the root,
current generation and session identity. Confirmations retain the `mgw2`
binding to generation, session, exact intent and final byte plan. A token from
another session or owner cannot authorize a mutation; a grant minted under a
lost owner is fenced by the new generation.

All clients use the same transaction, journal and index owners. Concurrent
edits retain exact-content revalidation and serialized application. Competing
edits based on one old hash produce one applied change and a stale-anchor
refusal; another connection's identity cannot select the first's undo history
by default, and `scope: workspace` with named ops remains the explicit,
conflict-surfacing way to rewind another client's op — which is how the
cockpit's journal model rewinds an agent's edit through the same owner.

## Process lifetime and workers

The owner is embedded in an invocation, never a daemon. Its `--workspace-worker`
children run in their own process group and are killed and reaped on every
orderly path; a killed owner runs no path, so each worker also watches its
parent (a `getppid` poll every 250 ms) and ends itself once re-parented, after
shutting its sandbox pool. There is no idle timer or retire protocol: the
owner's own client ends it, and its peers reconnect.

## Ownership diagnostics

`magistr workspace doctor <root>` appends two sections to the six retained
probes, both composed by the CLI host from observations the workspace crate
alone cannot make, and both read-only: no lock is taken, no store opened,
nothing admitted, minted, marked or written.

- **`owner`.** The durable generation (`.magistr/workspace/server.generation`) and
  the lock record (`ServerLock::inspect`: absent, the native marker, or
  unrecognized bytes) are read as records of past
  admissions. Liveness comes only from one bounded `probe` handshake on the
  owner endpoint ([project authority](project-authority.md#attachment)):
  a live owner is reported with its generation, pid, build, endpoint and
  every attached session by kind with the caller each declared (`mcp` for
  bridges and the cockpit's port, `commands` for a workflow client); the
  embedded owner's own client speaks over an in-process duplex and is not
  a socket session, so a lone `magistr run` reports zero. A durable counter
  that disagrees with the live owner, a refused identity/user probe and a
  listener that accepts but never greets are unhealthy findings. A probe
  refused on build equality is informational: it names the running build
  and the diagnostic limitation without judging MCP attachment or telling
  the user to close a working session; no endpoint, or an endpoint file
  nothing answers on, is reported as *no owner reachable* with the counter
  named as the last admission, never as a live owner. An unrecognized record is an offline restoration finding.
- **`operations`.** The run checkpoint (`.magistr/loop-state.json`) read as
  it lies, see [durability](durability.md#diagnostics-versus-evidence).

Both sections render in the report and ride the bundle as `owner.json` and
`operations.json` through the same redaction choke point. They are
diagnostics: computed on request, retained nowhere.

## Production launch and discovery

`magistr mcp --dir <root>` selects the nearest configured project within its
working-copy boundary and serves both owners through one catalog. The scaffold
writes one `magistr` registration for each selected agent. The cockpit source
port uses that registration; its journal reads and undo requests use `execute`
to call the admitted owner's `undo` operation. Explicit owner fixtures that
exercise this connection must compose the shipping sandbox interpreter.
See [unified MCP](unified-mcp.md) for direct tools, discovery, composition,
cancellation and per-connection planning restrictions.

Initialization seeds `.magistr/workspace.toml` through native detection. No external
workspace executable or Bun runtime is required. Multiple agents connect to the
same workspace owner; distinct working copies retain separate journals and indexes.
The optional `codex-subagent` gate exercises real parent and subagent attachment
in a disposable project.

## ADR-018 scope

The retained [ADR-018](../../subsystems/workspace/reference/specs/018-multi-client-daemon/adr.md)
and [acceptance matrix](../../subsystems/workspace/reference/specs/018-multi-client-daemon/acceptance-matrix.md)
are design inputs. ADR-018 chose a detached daemon; Magistr implements the
embedded-owner bridge above. The table maps all 24 retained rows to native
evidence. `attachment` means
[`crates/magistr-cli/tests/workspace_attachment.rs`](../../crates/magistr-cli/tests/workspace_attachment.rs)
and its support modules; recorded ship checks and the iteration receipt
supply actual execution results (macOS host).

| ADR row | Native evidence | Remaining |
| --- | --- | --- |
| 1: two clients | `attachment`: both initialize and use the tool surface through one owner; `doctor_reports_the_live_owner_and_counts_attached_bridges_as_they_come_and_go` reads the owner's generation and its attached count rising with a second bridge through the probe | None |
| 2: startup race | `attachment`: eight barrier-released starts initialize, share generation 1 and close before generation 2 admits | None for the embedded model (no daemon record, no idle cleanup) |
| 3: different-file edits | `attachment`: both writes apply; each identity lists only its own journal row | None |
| 4: same-file edits | `attachment`: exactly one anchored edit applies; the other receives `STALE_ANCHOR`; both clients read the final bytes | None |
| 5: stale preview | `attachment`: external bytes change invalidates the exact `mgw2` plan; `CONFIRM_MISMATCH` refuses and journals nothing | Native code differs from the retained `STALE_ANCHOR` by design |
| 6: cross-session token | `attachment`: another session's confirmation is refused; the originating session retains its grant | None |
| 7: cross-session undo | `attachment`: identities isolate histories; an explicit identity resumes its own undo over a new connection; the cockpit rewinds by `scope: workspace` | None |
| 8: identity survives owner restart | `attachment`: a bridge's stamped identity lists the same op under generation 2, edits again, and rewinds its own latest batch | None |
| 9: owner crash mid-session | `attachment`: after `kill -9` a survivor's next call succeeds under generation 2 (one admits, the other attaches); with admission denied, the call answers `ENGINE_UNAVAILABLE`/`retry` and the listing keeps answering until an owner returns | Fault injection *inside* a request (the answer is the owner's cancellation or the typed refusal, never a replay) |
| 10: crash mid-write | Existing native intent/transaction recovery boundary; `workspace_served_stdio` proves a crashed owner's grant is fenced on restart | Full fault-injection matrix |
| 11: client EOF/crash | `attachment`: attached EOF preserves owner and peers; owner EOF is survived by the remaining client, which admits on its next call; the doctor's attached count falls to zero after the bridge's client closes and reports no owner reachable after the owner's | None |
| 12: idle exit | Not applicable: an embedded owner ends with its own client; peers reconnect | — |
| 13: idle versus build | Owner shutdown waits for handlers and tracked formatter cleanup before index/worker teardown; a real pending-edit test covers owner EOF | — |
| 14: external edits | One shared native index and watcher retain live validation | Two-client watcher freshness scenario |
| 15: root alias | `attachment`: canonical aliases share the owner | None |
| 16: worktrees | `attachment`: independent canonical roots admit separately | Explicit Git-worktree fixture |
| 17: long path | `workspace_socket_path.tests.rs` checks a stable fallback within the Unix path limit | Real attachment through a deliberately long root |
| 18: permissions | Shared endpoint enforces private directory/socket and peer credentials | Cross-UID and non-host platform evidence |
| 19: version skew | `owner_endpoint.tests.rs`: a `Hello` from another build is refused with `AttachRefusal::Build` naming both identities — another checkout's contract digest and the other profile of one tree are told apart from an equal package version, a copy of the same build attaches, and a protocol-3 `Hello` is answered with an explicit `Protocol` refusal; a bridge answers typed refusals until the owner of the other build ends, then admits; a probe from another build is refused the same way, and the doctor reports that owner as live but of another build | A real two-binary process fixture (the identities are exercised over the wire from one test process) |
| 20: unrecognized owner record | `source_pipeline_routing` and `workspace_served_stdio`: refuse admission, preserve the record byte for byte and open no workspace stores | None |
| 21: unreachable owner | `attachment`: a bridge preserves an unrecognized record and reconnects after an offline restore; a bridge started against a lock held without an endpoint answers within bounded windows, opens no store and admits once the lock is released. Non-temporary refusals still exit (66, 65, policy) | Startup against a lock held without an endpoint costs one bounded window per request until it is released |
| 22: statelessness | Existing native served-schema/envelope tests use the same adapter through every path | Retained gate replayed through two bridges |
| 23: Codex parent/subagent | `scripts/codex-subagent-check.py` passed on 2026-09-12 with codex-cli 0.154.0: parent and subagent `read` ok, two launches, zero exit-75, all nine tool names in the subagent's inventory | The gate is optional (needs `codex` and auth) |
| 24: cockpit | `launcher_tests`: the cockpit port built from the project's `.mcp.json` maps, reads and lists the journal through the live owner without admitting | Live `magistr run` Changes-tab acceptance |

The retained reference files are unchanged. This matrix records a first-party
adaptation with its own executed evidence, not a passing execution of the
upstream opt-in red suite.
