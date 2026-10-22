# Root cause: Workbench tools vanish for Codex subagents

Session date: 2026-09-09. Recorded natively as think step 1 (pinned). Companion to
[adr.md](adr.md).

## Question

A Codex CLI 0.153.4 parent agent in a separate project checkout had all nine Workbench
tools; its subagent briefly saw them, then every call answered "MCP tool `workbench-mcp/read`
is not available to the model", then the names disappeared from its inventory while Magistr
and SerpAPI names stayed. Hypothesis to verify: the process-lifetime project lock conflicts
with harnesses that launch a separate MCP connection per subagent.

## Confirmed findings

Each finding names the evidence that produced it.

1. **`workbench-mcp <ws>` holds a process-lifetime exclusive lock and a second process
   exits before any transport exists.** `openWorkspace` (`src/protocol/workspace.ts`)
   exclusive-creates `.workbench/server.lock` with `{pid, host}`; `main.ts` maps
   `WorkspaceLockedError` to exit 75. Reproduced with two raw stdio clients on a disposable
   root (`scratchpad/two-clients.ts`): client B's server exited with code 75, stderr
   `another workbench-mcp instance (pid 23009) already owns this workspace (lock: …/.workbench/server.lock)`,
   and wrote zero bytes to stdout, so B never received an `initialize` response. A kept
   answering; A's stdin EOF released the lock; a fresh client then connected normally.

2. **Codex launches a new MCP server process for each subagent thread.** Controlled run
   under an isolated `CODEX_HOME` whose only MCP server was a shell wrapper that logs every
   launch with pid and exit code before exec'ing `bun src/main.ts <ws>`:

   ```
   16:03:28 LAUNCH pid=28415 ppid=28360      ← parent's server (bun pid 28420 took the lock)
   16:03:47 LAUNCH pid=29686 ppid=28360      ← spawn_agent → second server process
   16:03:47 EXIT   pid=29686 code=75
   server-stderr.log: workbench-mcp: another workbench-mcp instance (pid 28420) already owns this workspace
   ```

   Both processes were children of the same `codex` process (ppid 28360); the second was
   started for the subagent thread and refused by the parent's lock.

3. **The subagent inherits the parent's tool names but its calls are unbound.** In the
   controlled run the subagent listed all nine `mcp__workbench_mcp__*` names and its
   `workbench_status` call answered
   `{"content":[{"type":"text","text":"MCP tool `workbench-mcp/workbench_status` is not available to the model"}],"isError":true}`.
   The production rollout (`~/.codex/sessions/2026/09/09/rollout-…-01a08846-2f9c-….jsonl`,
   thread `execution_audit`, parent `01a08844-fc4c-…`) shows the same sequence with
   timestamps: forked 22:24:56; at 22:25:01 `ALL_TOOLS` contained the Workbench names; at
   22:25:06 four `read` calls failed "not available to the model"; at 22:25:27 the filtered
   inventory was `[]` and `typeof tools.mcp__workbench_mcp__read` was `undefined`.

4. **The harness does not surface the failure.** Neither the `--json` event stream nor
   stderr of `codex exec` mentioned the second server's exit; the only trace is inside the
   server's own stderr, which Codex discards. This is why the symptom presents as
   "unexplained disappearance of tools".

5. **Not the launcher.** `magistr mcp-launch workbench-mcp <dir>`
   (`crates/magistr-cli/src/orchestration/commands/misc/cmd_mcp_launch.rs`) resolves the
   project root and `exec()`s into the Workbench command, replacing its own process; the
   `ps` tree on this machine shows `bun ~/.bun/bin/workbench-mcp <root>` directly under
   `codex`. It adds no lock and no filtering. The `codex-launch --diagnose` version match
   (0.153.4) is about skill discovery, unrelated to MCP.

6. **Not harness tool filtering.** The project and user configs carry no
   `enabled_tools`/`disabled_tools`; `codex mcp get workbench-mcp --json` reported
   `enabled=true` with no filters; the subagent's inventory kept every other MCP server.

7. **Not a failed parent handshake.** The parent's server completed `initialize`,
   `tools/list` and dozens of `read`/`search` calls before and after the subagent ran.

8. **Magistr has the same single-owner refusal.** Control probe (`scratchpad/two-magistr.ts`):
   a second `magistr mcp-serve` on one project exits 1 with
   `cannot acquire project owner at ~/.local/share/think-and-ship/roadmap/sessions/<id>.mcp-owner.lock: another Magistr service holds this resource`
   (`OwnerLease` in `crates/magistr-cli/src/mcp_tools/owner_lease.rs`). In the production
   incident the subagent only inspected Magistr tool *names* and never called one, so
   "Magistr remained available" was an inspection of the inherited snapshot, not a
   successful call. The Codex run with both servers wrapped is recorded below.

## Hypotheses (consistent with the evidence, not independently proven)

- Codex seeds a forked thread with the parent's tool list, then starts the thread's own
  MCP servers; a server that fails before `initialize` has its tools removed from the
  registry on the next refresh. The +5 s / +10 s / +31 s sequence and the "not available to
  the model" wording fit this; Codex source was not read to confirm it.
- The `required = true` setting did not abort the subagent turn; it appears to govern the
  parent session's startup only.

## What the lock is protecting (why it exists)

From the code, the singular resources are the engine child's `index.db` writer and build
(`indexer.ts`), the file watcher, and the ranking state files. The write pipeline is
already cross-process safe through `write.lock`; the journal is SQLite; confirm tokens are
stateless HMACs over a shared key. See ADR-018 §1.1 for the full table.

## Codex run with both servers wrapped (run 3)

Same isolated `CODEX_HOME`, both `workbench-mcp` and `magistr` behind logging wrappers,
`codex exec … < /dev/null` (without the stdin redirect `codex exec` blocked on
"Reading additional input from stdin" for ten minutes and launched nothing):

```
16:23:02 LAUNCH pid=10074 ppid=10013            ← parent: workbench-mcp
16:23:02 MAGISTR LAUNCH pid=10073 ppid=10013    ← parent: magistr mcp-launch magistr
16:23:28 LAUNCH pid=11563 ppid=10013            ← spawn_agent: second workbench-mcp
16:23:28 EXIT   pid=11563 code=75
16:23:28 MAGISTR LAUNCH pid=11567 ppid=10013    ← spawn_agent: second magistr
16:23:28 MAGISTR EXIT   pid=11567 code=1
```

Parent: both `workbench_status` and `roadmap_status` succeeded. Subagent, verbatim:
`MCP tool `workbench-mcp/workbench_status` is not available to the model` and
`MCP tool `magistr/roadmap_status` is not available to the model`; its first enumeration
listed both namespaces, its enumeration after the calls listed neither. So the incident's
"specific to Workbench" observation was an artifact of which tools the subagent exercised:
every single-owner MCP server in the project loses its tools in a Codex subagent, and the
fix must be shaped so that Magistr can adopt it too (recorded as a signal).

## Bottom line

The hypothesis is confirmed and narrowed: the cause is the process-lifetime `server.lock`
meeting a harness that starts one server process per subagent thread, and the failure is
invisible because the refused process dies before the MCP handshake. Lock contention in the
write path (`write.lock`) is not involved. The design answer is in [adr.md](adr.md).
