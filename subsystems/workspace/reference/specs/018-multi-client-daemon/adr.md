# ADR-018: One workspace daemon, many thin MCP bridges

- **Status:** Accepted 2026-09-09 (adopted by the maintainer; implementation proceeds through the `m7-multi-client` roadmap chunks, starting with `daemon-acceptance-harness`)
- **Date:** 2026-09-09
- **Deciders:** workbench-mcp maintainers
- **Evidence:** [root-cause.md](root-cause.md) (think step 1, pinned)
- **Companions:** [migration.md](migration.md), [acceptance-matrix.md](acceptance-matrix.md), roadmap group `m7-multi-client`

## 1. Context

`openWorkspace` (`src/protocol/workspace.ts`) takes `.workbench/server.lock` by exclusive
create for the life of the server process. A second `workbench-mcp <ws>` process exits 75
before any MCP transport exists. Every stdio MCP client is one server process, and Codex
CLI 0.153.4 launches a fresh server process per subagent thread, so a subagent's server dies
at birth, the harness never completes `initialize`, and the nine Workbench tools become
"not available to the model" while the parent keeps working. Claude Code shares the parent's
connection with in-process subagents and is unaffected; Cursor, VS Code windows, a second
terminal, and the Magistr cockpit hit the same wall as Codex.

The SDD (§5.2, §7.7) and the constitution say "exactly one server instance owns a workspace";
the actual resource that must be singular is narrower than a server process.

### 1.1 What actually requires exclusive ownership (from the code)

| Resource | Owner today | Truly exclusive? | Why |
|---|---|---|---|
| `index.db` build + upserts | engine child of the one server (`indexer.ts`, "single SQLite writer") | **Yes** | cache-class discard-and-rebuild, generation counters, FTS rowid alignment all assume one writer; two builders would race and one would discard the other's work |
| File watcher (`fswatch/wiring.ts`) | server process | **Yes (one is enough)** | N watchers issue N refreshes and overwrite one `watcher.json` |
| Engine child (`supervisor.ts`) | server process | **No, but expensive** | N clients = N Bun+native engines (the Codex/Serena duplication problem, openai/codex#12333) |
| Write pipeline (§7.5) | `write.lock` per batch (`workspace-lock.ts`) | Serialized already | PID-aware advisory file lock + in-process FIFO; cross-process safe today |
| `journal.db` | SQLite WAL, one transaction per op | Multi-process safe | SQLite serializes writers; `undone`/eviction are transactional |
| `workspace.db` | meta only | Multi-process safe | trivially |
| `keys/workspace.key` | created at init, read per call | Race at first init only | two simultaneous creators rename over each other; tokens minted with the loser's key answer CONFIRM_MISMATCH |
| Confirm tokens | stateless HMAC | Multi-process safe | shared key on disk |
| Session identity / undo scope | `_meta workbench/sessionId`, "" when absent | Stateless, but **unpopulated** | no real client sends it, so every op is anonymous and session scope is meaningless |
| `ranking-promotions.json`, `ranking-shadow.jsonl` | read-modify-write files | Not safe under N writers | plain file rewrites |
| `trace.jsonl`, `quarantine.jsonl` | append-only | Fine | |

Conclusion: one **indexer** (engine child + watcher + ranking state) per workspace must be
singular. Everything client-facing is already stateless per connection (`buildServer` is a
pure function of the runtime, proven by the statelessness gate), so multiplying MCP
*sessions* costs nothing in correctness.

## 2. Decision

Split the server process into two roles inside the one binary:

- **Daemon** (one per canonical workspace root): owns the engine child, the index, the
  watcher, the journal handle, the write pipeline and the ranking state. Listens on a local
  IPC endpoint. For each accepted connection it constructs a `Server` via the existing
  `buildServer(runtime, …)` over a socket `Transport`, so every MCP semantic the stdio
  server has today (elicitation, cancellation, tasks, notifications) passes through
  unchanged.
- **Bridge** (one per MCP client, what the harness launches as `workbench-mcp <ws>`): a byte
  relay between the client's stdio and one daemon connection, plus three small duties:
  connect-or-spawn the daemon, stamp a session identity on `tools/call`, and answer typed
  errors instead of dying while the daemon is unavailable.

The command line, `.mcp.json`, Codex `config.toml` and `magistr mcp-launch` stay as they are:
`workbench-mcp <ws>` *is* the bridge. No harness has to share a bespoke connection.

### 2.1 Component boundaries

```
 client (Codex thread / Claude Code / cockpit)         client 2               client N
        │ stdio (MCP JSON-RPC lines)                       │                      │
 ┌──────▼───────┐                                  ┌───────▼──────┐        ┌──────▼──────┐
 │ bridge       │  same binary, role "bridge"      │ bridge       │  …     │ bridge      │
 │ connect-or-  │                                  │              │        │             │
 │ spawn, relay │                                  │              │        │             │
 └──────┬───────┘                                  └───────┬──────┘        └──────┬──────┘
        │ unix socket (.workbench/run/daemon.sock, dir 0700; TMP fallback)        │
 ┌──────▼──────────────────────────────────────────────────▼──────────────────────▼──────┐
 │ daemon (one per realpath(root); holds daemon.lock)                                     │
 │  per-connection: handshake → buildServer() → SocketTransport   (N Server instances)    │
 │  shared: EngineSupervisor → engine child (index.db writer), watcher, JournalDb,         │
 │          WriteEngine + write.lock (FIFO in-process; file lock still guards CLI/legacy), │
 │          RankingSignals, tracer, idle timer                                             │
 └────────────────────────────────────────────────────────────────────────────────────────┘
```

Import-graph rule (§6.4): the bridge lives in `src/protocol/bridge/`, the daemon in
`src/protocol/daemon/`; both depend on `protocol/` and `ports/` only. `serveStdio` becomes
one of two transports the daemon can serve a connection over (stdio for `--exclusive`, socket
for bridges).

### 2.2 Lock scopes after the change

| Lock | Scope | Holder | Lifetime | Stale rule |
|---|---|---|---|---|
| `.workbench/daemon.lock` (today's `server.lock`, one file, `role` field added) | one indexer/daemon per canonical root | daemon **or** a `--exclusive` legacy server | process lifetime | dead pid on this host → broken once (unchanged) |
| `.workbench/write.lock` | one write batch at a time across *all* entry points | whichever process runs the §7.5 pipeline | one batch | 5 min or dead pid (unchanged) |
| in-daemon write FIFO | order of batches from N sessions | daemon | one batch | n/a |
| SQLite locks (`journal.db`, `workspace.db`, `index.db`) | one transaction | SQLite | one transaction | SQLite |

A legacy exclusive server and a daemon can never coexist on one root because they take the
same `daemon.lock`; that is what keeps the migration free of competing writers (§ migration).

### 2.3 Endpoint, identity, permissions

- **Canonical root** = `realpath(root)`; **workspace id** = `sha256(canonical root)`. A
  symlinked alias resolves to the same daemon. Each git worktree is a different canonical
  root, has its own `.workbench/`, and gets its own daemon (already true today, now named).
- **Socket path**: `<root>/.workbench/run/daemon.sock` when it fits `sockaddr_un` (104 bytes
  on macOS, 108 on Linux), else `${XDG_RUNTIME_DIR:-$TMPDIR}/workbench-mcp-<uid>/<wsid[:16]>.sock`.
  `run/` is created `0700`; the socket inode itself is `0755` on macOS (probed), so the
  directory mode is the access control. `.workbench/**` is jail-protected, so no tool can
  rewrite the endpoint record.
- **`.workbench/daemon.json`** (transient, cache-class): `{v:1, pid, bootId, socket, protocol,
  serverVersion, root, startedAtMs}`. Readers validate it by pid liveness **and** a connect
  probe, never by trusting the file (same rule Magistr's `ensure_feed_available` uses).
- **Windows**: named pipe `\\.\pipe\workbench-mcp-<wsid>` once Bun's support is verified;
  until then `--exclusive` remains the Windows default. Recorded as an open item, not hidden.

### 2.4 Wire protocol (bridge ↔ daemon)

Line 1 from the bridge is a handshake; everything after is MCP JSON-RPC, newline-delimited
exactly as the SDK's stdio transport frames it, relayed verbatim in both directions.

```json
{"workbench":"bridge","protocol":1,"serverVersion":"<SERVER_VERSION>","pid":123,
 "session":"01J…","client":{"name":"codex","version":"0.153.4"}}
```

Daemon reply: `{"ok":true,"pid":…,"serverVersion":…}` or
`{"ok":false,"code":"VERSION_MISMATCH"|"PROTOCOL_MISMATCH","message":…,"retryAfterMs":…}`.
`protocol` is bumped only when the handshake or relay framing changes; `serverVersion`
mismatch means the bridge and daemon come from different checkouts.

### 2.5 Sessions, undo, confirm tokens

- **Session identity** is minted by the bridge, once per bridge process:
  `WORKBENCH_SESSION_ID` if the launcher set it, else a ULID. The bridge stamps
  `_meta["workbench/sessionId"]` on every `tools/call` that lacks one and leaves a
  client-provided value alone. The daemon keeps using `sessionIdFrom(meta)`; journal rows
  finally carry real identities, so `undo` session scope means "this bridge's ops".
- **Reconnect**: the bridge keeps its session id across daemon restarts; `undo scope:"session"`
  survives a daemon crash. A harness that resumes a thread with a new bridge gets a new id;
  the existing `scope:"workspace"` + `ops:[seq]` path is the documented way to rewind
  another session's op, and the conflict surfacing (§7.6) already protects it.
- **Confirm grants** move from `wb1.<diff>.<content>.<mac>` to
  `wb2.<diff>.<content>.<sessionHash>.<mac>` with
  `mac = HMAC(key, diff ‖ content ‖ sessionId)`. A token minted for session A and presented
  by session B answers CONFIRM_MISMATCH; a token whose files moved still answers
  STALE_ANCHOR with the fresh hash. `wb1` is refused after the flip (tokens are minutes-old
  by design; `key rotate` semantics unchanged).
- **Reads, previews, applies** keep the existing optimistic chain: `read` → `contentHash`;
  `edit … expectedHash` → STALE_ANCHOR on mismatch; `dryRun` → CONFIRM_REQUIRED (token seals
  diff+content); `confirm` → re-verified against the *current* content under the write lock.
  Nothing here changes; it simply now runs in one process for every client.

### 2.6 Lifecycle

```mermaid
sequenceDiagram
  participant H as Harness (Codex thread)
  participant B as bridge (workbench-mcp <ws>)
  participant L as .workbench/daemon.lock + daemon.json
  participant D as daemon
  H->>B: spawn, stdio
  B->>B: realpath(root), wsid
  B->>L: read daemon.json, pid alive?, connect probe
  alt daemon reachable
    B->>D: connect + handshake
  else none
    B->>L: exclusive-create daemon.lock {pid, host, role:"daemon-spawner", bootId}
    alt won
      B->>D: spawn detached (stdio ignored, stderr → .workbench/daemon.log)
      D->>L: rewrite daemon.lock as {role:"daemon", pid}, bind socket, write daemon.json
      B->>D: connect (bounded wait, 50 ms poll, ≤ startup budget)
    else lost race
      B->>L: wait for daemon.json + connect probe (bounded)
      B->>D: connect
    end
  end
  D-->>B: {"ok":true}
  H->>B: initialize
  B->>D: initialize (relayed)
  D->>D: buildServer() for this connection
  D-->>B: result
  B-->>H: result
  Note over H,D: tools/list, tools/call, elicitation, cancel — all relayed verbatim
  H-->>B: stdin EOF
  B->>D: close connection
  D->>D: last connection gone → idle timer (default 10 min); exits, removes socket/json/lock
```

Second client while the first is live:

```mermaid
sequenceDiagram
  participant H2 as Codex subagent thread
  participant B2 as bridge 2
  participant D as daemon (already running)
  H2->>B2: spawn
  B2->>D: probe ok → connect + handshake {session: new ULID}
  H2->>B2: initialize / tools/list / tools/call
  B2->>D: relayed; D builds a second Server instance for this connection
  Note over H2,D: nine tools available; reads concurrent; writes FIFO behind write.lock
```

Daemon loss:

```mermaid
sequenceDiagram
  participant H as client
  participant B as bridge
  participant D as daemon
  D--xB: socket EOF (crash or idle exit)
  B-->>H: every in-flight request → ENGINE_UNAVAILABLE envelope (recovery: retry, note names daemon pid/log)
  B->>B: connect-or-spawn loop, backoff ladder [0,50,100,200,400,800] ms
  H->>B: next tools/call
  B->>D: new daemon → relayed → answered
  Note over H: tools never disappear; the client sees one typed error, not a dead server
```

Version skew (bridge from a newer checkout than the daemon):

1. handshake answers `VERSION_MISMATCH`; the bridge sends `workbench/retire` (daemon stops
   accepting, exits when its connections close) and waits, answering `tools/list` from the
   generated schemas and `tools/call` with ENGINE_BUSY + `backoffMs` meanwhile;
2. the bridge then spawns a daemon of its own version. Old clients on the old daemon keep
   working until they disconnect. `workbench-mcp daemon stop <ws>` forces the same path by
   hand.

### 2.7 Startup races, crashes, stale state

- **Two bridges start simultaneously**: exclusive-create of `daemon.lock` elects one spawner;
  the loser waits for the endpoint. A spawner that dies before the daemon publishes
  `daemon.json` leaves a lock naming a dead pid, which the next bridge breaks (existing
  rule). A daemon that dies leaves a socket that answers ECONNREFUSED; only the new lock
  holder unlinks and rebinds it.
- **Daemon crash mid-write**: journal-before-swap and per-file temp+rename are unchanged;
  `write.lock` is broken by the next holder on dead pid. Compensation on partial swap is
  the existing best-effort rewind (`write-engine.ts`).
- **Orphaned clients**: a bridge whose stdin hits EOF exits (today's behavior); the daemon
  drops that connection and its in-flight engine calls are cancelled through the existing
  supervisor cancel path.
- **Orphaned daemon**: intended for the idle window, then reaped by the idle timer. `doctor`
  reports it; `daemon stop` ends it.
- **Idle shutdown never interrupts a Tier-1 build**: the timer waits for `building:false`.

### 2.8 Cross-client freshness and external edits

One watcher, one indexer, one generation counter. An edit through any bridge is swapped to
disk and refreshed into the index by the same daemon; an external edit (git checkout, IDE
save, `cargo fmt`) reaches the same index through the watcher exactly as today. `read`
keeps verifying mtime+size before answering, so a stale line number still never leaves the
server, whichever bridge asked.

### 2.9 Actionable initialization errors

The bridge always completes `initialize` and `tools/list`. When the daemon cannot be
reached or started it answers every `tools/call` with a typed envelope:
`ENGINE_UNAVAILABLE` (recovery `retry`) whose notes carry the lock holder pid/host, the
socket path, the daemon log path and the exact next action (`workbench-mcp doctor`,
`workbench-mcp daemon stop`). Exit 75 remains only for `--exclusive`. `doctor` gains a
`daemon` section: pid, version, socket, connection count, idle deadline, last error.

## 3. Alternatives considered

### A. Multiple server processes with narrowly scoped locks (rejected for now)

Each client keeps its own full server; an *indexer lease* elects one process to build and
watch, the others open `index.db` read-only; `write.lock` already serializes the pipeline;
`journal.db` is shared through SQLite; ranking files gain a lock.

- Pro: no IPC, no daemon lifecycle, the statelessness gate already proves envelope
  equivalence across processes.
- Con: N engine children (each a Bun runtime plus the native core) per workspace, the
  duplication users already complain about in Codex; index freshness for non-lease
  processes depends on the lease holder's watcher debounce, and a non-lease process cannot
  build when the holder is gone without re-running the election inside a request; four
  more races to own (key init, ranking files, watcher.json, cache-class discard-and-rebuild
  seen by a reader mid-rebuild). It solves the symptom while keeping the hidden coupling.

### B. Host-level connection multiplexing (rejected)

Ask harnesses to reuse the parent's connection, or make `magistr mcp-launch` a mux.

- Codex closed the request as not planned (openai/codex#12333); only in-process-subagent
  hosts (Claude Code) already do this. A Magistr-side mux is the daemon design living in the
  wrong repository and only helps Magistr-launched sessions. Not portable, and it does not
  help the cockpit, a second terminal, or an IDE window.

### C. Daemon + bridges (chosen)

One engine per workspace, N sessions, MCP semantics untouched because each connection is a
real `Server`. Matches the precedent already running on this machine: Magistr's own
think-and-ship socket daemon under `.magistr/` with an owner lease. Cost: a lifecycle to own
(spawn, idle, retire, version skew) and one more IPC protocol, both bounded and specified
above.

### Variant not chosen: host the daemon inside the first client's process

Simpler spawn story, but the first client's exit takes the engine away from everyone else
and forces a failover blip on every parent/subagent handoff. A detached daemon with an idle
timer has one more moving part and none of the blips.

## 4. Consequences

- Constitution and SDD §5.2/§7.7 wording changes from "one server instance per workspace"
  to "one engine per workspace, any number of sessions". This is not the fleet profile
  (Appendix B stays designed-not-built): still one index owner, still local, still no
  network.
- The statelessness gate keeps running (per connection). A new concurrency suite becomes
  release-blocking (constitution VI already lists "concurrency").
- `journal-read-only-instance` (roadmap) is subsumed: a read-only bridge is the same daemon
  with a narrowed `tools/list`. The Magistr cockpit can hold a live connection during a run.
- The routing skill and `lint:skill` need the new error notes (ENGINE_UNAVAILABLE with
  daemon diagnostics, VERSION_MISMATCH) documented.
- Open items carried explicitly: Windows transport; peer-credential check on Linux
  (SO_PEERCRED) as defense in depth beyond the 0700 directory.
