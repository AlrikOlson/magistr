# Workspace owner

Every Magistr workspace client reaches one admitted native owner. `magistr
workspace serve <root>` — and its retained production alias `magistr
mcp-launch workbench-mcp <root>`, the line every scaffolded `.mcp.json` and
Codex `config.toml` carries — is a *bridge*: its client speaks MCP on the
bridge's stdio, and the bridge speaks the same frames to whichever owner is
reachable. The first invocation admits the workspace and owns the
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

`WorkspaceService` holds the exclusive OS lock on `.workbench/server.lock`
before opening the key, journal, index or worker. Its immutable native marker
and durable `.workbench/server.generation` retain the d1 exclusion and fencing
contract. Lock contention is the only attachment path; legacy, empty or
malformed markers keep the documented
[offline migration refusal](../../subsystems/workspace/PROVENANCE.md#offline-migration),
now performed by `magistr workspace migrate <root>`: it moves the obsolete
record aside (never deletes it) only when the record was written on this host
and its process is not alive, never touches a native marker, and exits 75
otherwise. No process takes ownership from a live holder, uses its PID as
admission proof, or resets the durable generation.

`OwnerIdentity::Workspace` names the canonical root independently of workflow
project identity, store and writer roles. Aliases resolve to the same owner;
distinct roots, including separate worktrees, have independent services and
journals. The shared [project owner protocol](project-authority.md#handshake-and-identity)
uses `Hello` version 4, which carries a
[`BuildIdentity`](project-authority.md#build-identity): an owner serves only
clients of its exact build — package version, contract digest, profile and
target — and refuses any other with `AttachRefusal::Build` naming both
identities, so one workspace never mixes two contracts even when their
package versions are equal. Workspace endpoints accept MCP connections, not
workflow commands. Socket peer credentials restrict attachment to the owner's
operating-system user.

The endpoint is `<root>/.workbench/run/owner.sock` when the path fits the Unix
socket limit; long roots use
`/tmp/magistr-workspace-<uid>/<root-digest>.sock`. The containing directory is
private (`0700`) and the socket is `0600`. A live endpoint is preserved; only
an admitted owner may reclaim an abandoned socket. Contenders share a
five-second attach-or-admit budget while an owner opens its service and
publishes the endpoint; each connect and greeting exchange has one absolute
deadline, capped at 250 ms per attempt. Startup with no reachable owner and no
admissible workspace is a startup error (exit 75), without a second service or
a synthetic successful MCP initialization. Non-Unix remains exclusive.

The owner's resolved policy governs all connections. An invocation carrying
explicit `--config` values refuses attachment rather than silently dropping
those values or changing the live owner's policy.

## The bridge

The bridge relays newline-delimited JSON-RPC verbatim in both directions and
looks at three things:

- **Identity.** Every `tools/call` that carries no `_meta["workbench/sessionId"]`
  is stamped with the bridge's durable identity — `WORKBENCH_SESSION_ID` when
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

**Embedded owner.** The owning invocation serves its own client the same way,
over an in-process duplex to the rmcp handler, so both paths are one code path
and every client gets a stamped identity.

**Owner loss.** When an attached owner's socket ends, every request still
waiting is answered at once: a `tools/call` receives the retained
`ENGINE_UNAVAILABLE` envelope with recovery `retry` (text and
`structuredContent` alike), `tools/list` the cached listing, `ping` an empty
result, anything else a JSON-RPC error naming the reason. The tool surface
never disappears and the bridge process never exits on owner loss. The next
request that needs an answer runs one bounded attach-or-admit attempt: it
attaches to whoever admitted the workspace meanwhile, or admits the workspace
itself and becomes the embedded owner for its peers — the embedded-owner
analog of ADR-018's connect-or-spawn, with no daemon. If neither is possible
(for example a marker that needs migration), the request is answered with the
typed refusal and the next request tries again.

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

## Production launch and discovery

- `magistr mcp-launch workbench-mcp <dir>` selects the project root exactly as
  before (nearest `.magistr` or `.jig` project within the repository, else the
  directory) and runs the bridge. The scaffold's `.mcp.json` and Codex
  `config.toml` entries are unchanged, so a project configured before the
  cutover works the moment its binary is upgraded.
- `magistr init` seeds `workbench.toml` through the native detection
  (`magistr workspace init`), never an installed `workbench-mcp init`.
- The interview's temporary `.mcp.json` always includes the workspace entry.
- The cockpit's port reads the project's `workbench-mcp` launch line, spawns
  it, and attaches to the run's owner — the host itself when it admitted the
  workspace, else whoever did — during `magistr run`; its journal view runs
  `magistr workspace journal`.
- Codex parent and subagent threads each launch a bridge; both attach to one
  owner. `scripts/codex-subagent-check.py` (the optional `codex-subagent`
  gate) runs the real `codex exec` under an isolated `CODEX_HOME` and passes
  only when the parent and the subagent both read through the same owner with
  two launches and zero exit-75.

A workspace whose `.workbench/server.lock` still holds the retained legacy
server's record is refused until `magistr workspace migrate` runs with that
server stopped. This repository's own `.mcp.json` keeps its explicit legacy
entry until that migration is performed offline; nothing here runs the native
owner against a live legacy workspace.

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
| 1: two clients | `attachment`: both initialize and use the tool surface through one owner | Connection counts in doctor |
| 2: startup race | `attachment`: eight barrier-released starts initialize, share generation 1 and close before generation 2 admits | None for the embedded model (no daemon record, no idle cleanup) |
| 3: different-file edits | `attachment`: both writes apply; each identity lists only its own journal row | None |
| 4: same-file edits | `attachment`: exactly one anchored edit applies; the other receives `STALE_ANCHOR`; both clients read the final bytes | None |
| 5: stale preview | `attachment`: external bytes change invalidates the exact `mgw2` plan; `CONFIRM_MISMATCH` refuses and journals nothing | Native code differs from the retained `STALE_ANCHOR` by design |
| 6: cross-session token | `attachment`: another session's confirmation is refused; the originating session retains its grant | None |
| 7: cross-session undo | `attachment`: identities isolate histories; an explicit identity resumes its own undo over a new connection; the cockpit rewinds by `scope: workspace` | None |
| 8: identity survives owner restart | `attachment`: a bridge's stamped identity lists the same op under generation 2, edits again, and rewinds its own latest batch | None |
| 9: owner crash mid-session | `attachment`: after `kill -9` a survivor's next call succeeds under generation 2 (one admits, the other attaches); with admission denied, the call answers `ENGINE_UNAVAILABLE`/`retry` and the listing keeps answering until an owner returns | Fault injection *inside* a request (the answer is the owner's cancellation or the typed refusal, never a replay) |
| 10: crash mid-write | Existing native intent/transaction recovery boundary; `workspace_served_stdio` proves a crashed owner's grant is fenced on restart | Full fault-injection matrix |
| 11: client EOF/crash | `attachment`: attached EOF preserves owner and peers; owner EOF is survived by the remaining client, which admits on its next call | Connection-count diagnostics |
| 12: idle exit | Not applicable: an embedded owner ends with its own client; peers reconnect | — |
| 13: idle versus build | Owner shutdown waits for handlers and tracked formatter cleanup before index/worker teardown; a real pending-edit test covers owner EOF | — |
| 14: external edits | One shared native index and watcher retain live validation | Two-client watcher freshness scenario |
| 15: root alias | `attachment`: canonical aliases share the owner | None |
| 16: worktrees | `attachment`: independent canonical roots admit separately | Explicit Git-worktree fixture |
| 17: long path | `workspace_socket_path.tests.rs` checks a stable fallback within the Unix path limit | Real attachment through a deliberately long root |
| 18: permissions | Shared endpoint enforces private directory/socket and peer credentials | Cross-UID and non-host platform evidence |
| 19: version skew | `owner_endpoint.tests.rs`: a `Hello` from another build is refused with `AttachRefusal::Build` naming both identities — another checkout's contract digest and the other profile of one tree are told apart from an equal package version, a copy of the same build attaches, and a protocol-3 `Hello` is answered with an explicit `Protocol` refusal; a bridge answers typed refusals until the owner of the other build ends, then admits | A real two-binary process fixture (the identities are exercised over the wire from one test process) |
| 20: legacy overlap | d1 native/Bun mutual-refusal oracle; `migrate` refuses a live legacy pid and a foreign host | None |
| 21: unreachable owner | Post-startup: typed refusals, cached listing, bounded retry per request. Startup: exit 75 | Always-available `initialize` before any owner was ever reached |
| 22: statelessness | Existing native served-schema/envelope tests use the same adapter through every path | Retained gate replayed through two bridges |
| 23: Codex parent/subagent | `scripts/codex-subagent-check.py` passed on 2026-09-12 with codex-cli 0.154.0: parent and subagent `read` ok, two launches, zero exit-75, all nine tool names in the subagent's inventory | The gate is optional (needs `codex` and auth) |
| 24: cockpit | `launcher_tests`: the cockpit port built from the project's `.mcp.json` maps, reads and lists the journal through the live owner without admitting | Live `magistr run` Changes-tab acceptance |

The retained reference files are unchanged. This matrix records a first-party
adaptation with its own executed evidence, not a passing execution of the
upstream opt-in red suite.
