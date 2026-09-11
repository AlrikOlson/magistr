# Migration and rollback: exclusive server → daemon + bridges

Companion to [adr.md](adr.md). Every phase is a roadmap chunk in group `m7-multi-client`;
the order below is the dependency order.

## Invariants that hold in every phase

1. **Never two writers on one root.** The daemon and the legacy exclusive server take the
   same lock file (`.workbench/server.lock`, later read as `daemon.lock` with a `role`
   field). Whichever starts second is refused. `write.lock` keeps serializing the §7.5
   pipeline across processes, so the lock-free CLI paths (`journal`, `doctor`) and any
   exclusive server that outlives a flip stay safe.
2. **No on-disk format changes that an older binary cannot read.** `daemon.json` and
   `run/` are new, transient, cache-class files; the lock file gains an optional field the
   old reader ignores; journal, workspace and index schemas are untouched. Confirm grants
   change prefix (`wb1` → `wb2`), which only matters for a token minted seconds earlier.
3. **The launch line does not change.** `.mcp.json` (`bun run src/main.ts .`), Codex
   (`magistr mcp-launch workbench-mcp .`) and the global `~/.bun/bin/workbench-mcp` link
   all keep working; the role is decided inside `main.ts`.

## Phase 0 — evidence in the tree (chunk `daemon-acceptance-harness`)

- Check in the two reproductions from this session as tests: a two-client stdio test
  (today: second client's server exits 75) and a Codex parent/subagent runner that skips
  when `codex` is not on PATH or no auth is present. Both are red on the current code and
  define done for the rest.
- No behavior change. Gates unchanged.

## Phase 1 — endpoint and daemon role (chunks `daemon-workspace-endpoint`, `daemon-socket-server`)

- `openWorkspace` canonicalizes the root with `realpath`; workspace id derived; `run/`
  created 0700; lock record gains `role` and `bootId`.
- `workbench-mcp daemon start|stop|status <ws>` exists and `doctor` shows the daemon
  section. Nothing spawns a daemon automatically yet; the default serving path is still the
  exclusive stdio server. Users can opt in by starting a daemon by hand and running
  `WORKBENCH_MODE=bridge workbench-mcp <ws>`.
- Rollback: stop the daemon; the exclusive server is what every launcher still starts.

## Phase 2 — bridge behind a flag (chunk `bridge-stdio-relay`)

- `main.ts` reads `WORKBENCH_MODE` (`exclusive` | `daemon`) and the `--exclusive` flag.
  Default stays `exclusive`. Under `daemon` the entry becomes the bridge: connect-or-spawn,
  handshake, relay, session stamping, typed degradation, reconnect.
- Dogfood on this machine's Codex projects by adding `env = { WORKBENCH_MODE = "daemon" }`
  to their `mcp_servers.workbench-mcp` entries (project-local `.codex/config.toml`), one
  project at a time.
- Rollback: remove the env entry; the next session starts exclusive. A running daemon and a
  starting exclusive server refuse each other by the shared lock, so the operator gets a
  clear exit-75 message naming the daemon pid and `workbench-mcp daemon stop <ws>`.

## Phase 3 — identity and token binding (chunks `undo-session-identity`, `confirm-session-binding`)

- Bridge-minted session ids on every `tools/call`; `wb2` grants bound to the session.
- Rollback: independent of the transport flip; `wb2` verification can be reverted to
  `wb1` without touching stored data (nothing durable stores a grant).

## Phase 4 — flip the default (chunk `daemon-default-and-constitution`)

- Default becomes `daemon`; `--exclusive` and `WORKBENCH_MODE=exclusive` remain for one
  release. Constitution, SDD §5.2/§7.7, the routing skill and `lint:skill` updated together
  in the same commit (the skill lint is a gate).
- Windows stays `exclusive` by default until the named-pipe transport is verified.
- Rollback: `WORKBENCH_MODE=exclusive` (per launcher env) or a one-line default change;
  no data migration either way.

## Phase 5 — retire exclusive mode (follow-up, after the acceptance matrix is green in CI)

- Remove the flag; `serveStdio` survives only as the daemon's in-process transport for
  tests. Not scheduled in this spec.

## Magistr follow-ups (outside this repository, filed as a signal here)

- `magistr mcp-serve` has the same single-owner refusal (`OwnerLease`; a second process
  exits 1 with "cannot acquire project owner"), so Codex subagents lose Magistr tools too
  once they call one. The same daemon+bridge shape applies; the cockpit's `McpWorkbench`
  can drop its remembered-refusal path once Workbench accepts a second connection.

## Operator checklist for the first daemon flip on a project

1. `workbench-mcp doctor <ws>` shows no daemon and no live `server.lock` holder.
2. Set `WORKBENCH_MODE=daemon` in the project's launcher env.
3. Start one session; `doctor` shows the daemon pid, socket and one connection.
4. Start a second session (a Codex subagent, a second terminal, the cockpit); `doctor` shows
   two connections; both sessions answer `workbench_status`.
5. Close both; within the idle window `doctor` shows the daemon gone and no stale socket.
