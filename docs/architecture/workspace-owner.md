# Workspace owner

`magistr workspace serve <root>` serves multiple Unix MCP clients through one
admitted `WorkspaceService`. The first invocation owns the service; later
invocations attach through Magistr's shared local owner endpoint. Native
`arch-14d2-workspace-attachment` adds this bounded composition after d1's
admission and session fencing. Production launchers still use their existing
legacy Workbench routing until `arch-14d-workspace-owner` meets its remaining
lifecycle and full-matrix acceptance.

## Admission and rendezvous

`WorkspaceService` holds the exclusive OS lock on `.workbench/server.lock`
before opening the key, journal, index or worker. Its immutable native marker
and durable `.workbench/server.generation` retain the d1 exclusion and fencing
contract. Lock contention is the only attachment path; legacy, empty or
malformed markers retain the documented
[offline migration refusal](../../subsystems/workspace/PROVENANCE.md#offline-migration).
No process takes ownership from a live holder, uses its PID as admission
proof, or resets the durable generation.

`OwnerIdentity::Workspace` names the canonical root independently of workflow
project identity, store and writer roles. Aliases resolve to the same owner;
distinct roots, including separate worktrees, have independent services and
journals. The shared [project owner protocol](project-authority.md#handshake-and-identity)
uses `Hello` version 2 and validates the identity, protocol and attachment kind
before handing a connection to MCP. Workspace endpoints accept MCP connections,
not workflow commands. Socket peer credentials restrict attachment to the
owner's operating-system user.

The endpoint is `<root>/.workbench/run/owner.sock` when the path fits the Unix
socket limit; long roots use
`/tmp/magistr-workspace-<uid>/<root-digest>.sock`. The containing directory is
private (`0700`) and the socket is `0600`. A live endpoint is preserved; only
an admitted owner may reclaim an abandoned socket. Startup contenders share a
five-second attachment retry budget while the owner opens its service and
publishes the endpoint. Each connect and greeting exchange has one absolute
deadline, capped at 250 ms per attempt. Failure is a startup error, without a
second service or a synthetic successful MCP initialization. Non-Unix remains
exclusive.

The owner's resolved policy governs all connections. An invocation carrying
explicit `--config` values refuses attachment rather than silently dropping
those values or changing the live owner's policy.

## MCP sessions and writes

Every accepted connection gets a fresh `WorkspaceMcpService` over the same
service and its own nonempty random fallback session. Client-supplied valid
`_meta["workbench/sessionId"]` retains its meaning and can recover durable undo
identity on a later connection. The byte relay preserves MCP framing after
the shared owner handshake; tool schemas, envelope negotiation, elicitation,
request cancellation and task messages use the existing MCP adapter.

Session identity remains a logical boundary for cooperating local clients.
The socket peer credential establishes user admission; an explicit session
string is not authentication. Native `WorkspaceSession` handles bind the root,
current generation and session identity. Confirmations retain the `mgw2`
binding to generation, session, exact intent and final byte plan. A token from
another session or owner cannot authorize a mutation. No reconnect or token
format change is introduced by attachment.

All clients use the same transaction, journal and index owners. Concurrent
edits retain exact-content revalidation and serialized application. Competing
edits based on one old hash produce one applied change and a stale-anchor
refusal; another connection's fallback session cannot select the first
connection's undo history. Explicit workspace undo retains its existing
selection and conflict rules.

## Process lifetime

The first invocation is an embedded owner. Attached client EOF drops only that
connection; the owner's stdio session and other attached clients continue.
When the owner's invocation ends, the host awaits its final stdio handler
release, stops endpoint admission and cancels attached connections. It joins
their service loops and awaits each connection's completion guard, which
signals after the last handler clone has dropped, including clones held by
rmcp request tasks. The host then requires sole ownership of `WorkspaceService`
before shutting it down; unexpected surviving references cause an explicit
error. All attached relays end when the owner is gone.

Workspace shutdown drains the shared formatter operation tracker before the
existing index and worker cleanup. Dropping a request during preparation
signals its owned formatter task; that task kills and waits for its child
before completing. This covers formatter discovery as well as formatting.
The barrier covers MCP handler ownership and these tracked cleanup paths; it
does not join every arbitrary background task.

There is no transparent reconnect, detached daemon, idle timer, retire protocol
or promise that a killed owner reaps every descendant through parent-death
handling. A new invocation may acquire the released OS lock after owner loss;
that is a new admission and generation. Durable explicit session identity can
be re-admitted, while old confirmation grants remain fenced. The remaining
lifecycle work must be verified before production launcher cutover.

## ADR-018 scope

The retained [ADR-018](../../subsystems/workspace/reference/specs/018-multi-client-daemon/adr.md)
and [acceptance matrix](../../subsystems/workspace/reference/specs/018-multi-client-daemon/acceptance-matrix.md)
are design inputs. ADR-018 chose a detached daemon; this chunk deliberately
implements the smaller embedded-owner attachment boundary. The table maps all
24 retained rows without claiming that partial equivalents close those rows.
Test references name native evidence targets; recorded ship checks and the
iteration receipt supply actual execution results and platform limits.

`attachment` below means
[`crates/magistr-cli/tests/workspace_attachment.rs`](../../crates/magistr-cli/tests/workspace_attachment.rs).

| ADR row | Native scope and evidence | Remaining observable |
| --- | --- | --- |
| 1: two clients | `attachment`: both initialize and use the tool surface through one owner | Daemon/connection counts in doctor |
| 2: startup race | `attachment`: eight barrier-released starts initialize, share generation 1 and close before generation 2 admits | The 10 ms spawn-window oracle, daemon PID record and idle cleanup |
| 3: different-file edits | `attachment`: both writes apply; each session lists only its own journal row | Explicit distinct journal-sequence assertion in the full matrix |
| 4: same-file edits | `attachment`: exactly one anchored edit applies; the other receives `STALE_ANCHOR`; both clients read the final bytes | Fresh-hash payload assertion in the full matrix |
| 5: stale preview | `attachment`: external bytes change invalidates the exact `mgw2` plan; `CONFIRM_MISMATCH` refuses the edit and journals nothing | Native error differs from ADR-018's `STALE_ANCHOR`; changing that contract is outside d2 |
| 6: cross-session token | `attachment`: another session's confirmation is refused; the originating session retains its grant | None within this attachment scenario |
| 7: cross-session undo | `attachment`: default sessions isolate histories; an explicit identity resumes its own undo over a new connection | Cross-session `scope: workspace` with named operations and conflict surfacing |
| 8: restart identity | d1 service re-admission tests preserve durable undo; d2 reconnects an explicit identity while the same owner is alive | Attached process undo across owner restart; transparent reconnect retaining the fallback identity |
| 9: owner crash | `attachment`: owner loss ends attached clients; a later invocation can admit | In-flight typed retry error and automatic recovery without tool loss |
| 10: crash mid-write | Existing native intent/transaction recovery boundary remains in force | Full owner-crash fault-injection process matrix |
| 11: client EOF/crash | `attachment`: orderly attached-client EOF preserves owner and peers | Abrupt attached-process crash and connection-count diagnostics; owner EOF intentionally ends all clients |
| 12: idle exit | Initial invocation owns lifetime | Detached idle shutdown |
| 13: idle versus build | Normal shutdown waits for handlers and tracked formatter cleanup before index/worker teardown; a real pending-edit test covers owner EOF | Idle timer waiting for a build; owner-crash descendant cleanup |
| 14: external edits | One shared native index and watcher retain c3/c4 live validation | Two-client watcher/reference freshness process scenario |
| 15: root alias | `attachment`: canonical aliases share the owner | None within this attachment scenario |
| 16: worktrees | `attachment`: independent canonical roots admit separately | Explicit Git-worktree fixture in full parent matrix |
| 17: long path | `workspace_socket_path.tests.rs` checks a stable fallback within the Unix path limit | Binding and real MCP attachment through a deliberately long-root fixture |
| 18: permissions | Shared endpoint enforces private directory/socket and peer credentials | Cross-UID execution and non-host platform evidence |
| 19: version skew | Shared handshake refuses mismatched protocol/identity; equal-protocol build versions are not compared | Build-version detection, retirement and replacement lifecycle |
| 20: legacy overlap | d1 native/Bun mutual-refusal process oracle; d2 never attaches a migration refusal | Daemon stop/retire diagnostics |
| 21: unreachable owner | Bounded startup failure; no second service opens | Always-available initialize/list and typed unavailable tool envelopes |
| 22: statelessness | Existing native served-schema/envelope tests use the same adapter | Full retained statelessness gate through attached processes |
| 23: Codex parent/subagent | Native subprocess attachment supplies the required transport boundary | Actual isolated Codex harness acceptance before launcher cutover |
| 24: cockpit | Shared endpoint is available to native clients | Production cockpit integration and real interface acceptance |

The retained reference files are unchanged. This matrix records a first-party
adaptation, not a passing execution of the upstream opt-in red suite.
