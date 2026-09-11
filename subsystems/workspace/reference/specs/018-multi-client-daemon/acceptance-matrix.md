# Acceptance matrix — multi-client workspace daemon

Each row names the scenario, the exact observable that proves it, and the gate that runs
it. "Today" is the measured behavior on the current code (this session's reproductions);
"Required" is what the ADR commits to. A row is done only when its test is green in the
named gate, never by inspection.

Gates in this repository (discovered, not invented): `bun test` (1034 tests at last
record), `npx tsc --noEmit`, `bun run lint`, `bun run lint:imports`, `bun run codegen:check`,
`bun run lint:skill`, `cargo check --workspace --all-targets`, `cargo nextest run --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`. New tests land in `bun test`. The
multi-client chapter is opt-in until it can be green — `WORKBENCH_MULTI_CLIENT=1 bun test
test/multi-client.test.ts` (`bun run test:multi-client`); without the flag it skips with a
printed reason so the required gate stays green. The Codex runner is an opt-in script,
`bun run scripts/codex-subagent-check.ts` (`bun run check:codex-subagent`, Magistr gate
`codex-subagent` with `required: false`), which skips with exit 0 when `codex` is not on PATH
or no `auth.json` exists.

| # | Scenario | Setup | Required observable | Today | Gate / test |
|---|---|---|---|---|---|
| 1 | Two-client startup | two `workbench-mcp <ws>` processes over stdio on one disposable root | both complete `initialize`; both `tools/list` return nine tools; `doctor` shows one daemon, two connections | second exits 75, zero stdout | `test/multi-client.test.ts` row 1 (opt-in `WORKBENCH_MULTI_CLIENT=1`; the failure names the second server's pid, exit 75 and lock stderr; the `doctor` observable is asserted once daemon-socket-server lands) |
| 2 | Startup race | spawn 8 bridges within 10 ms on a cold root | exactly one daemon pid in `daemon.json`; all 8 connect within the startup budget; no orphan daemon after all exit + idle | n/a | `test/multi-client.test.ts` |
| 3 | Simultaneous edits, different files | sessions A and B each `edit` a different file at once | both applied; two journal seqs; each row carries its own session id | works only in one process | `test/multi-client.test.ts` |
| 4 | Simultaneous edits, same file | A and B both `read` then `edit` with `expectedHash` | first applies; second answers STALE_ANCHOR with the fresh hash; nothing half-written | single process only | `test/multi-client.test.ts` |
| 5 | Stale preview | A gets CONFIRM_REQUIRED, an external write changes the file, A presents the token | STALE_ANCHOR, no apply | works (single process) | `test/mrtr-confirm.test.ts` (extend) |
| 6 | Token crosses sessions | A gets a `wb2` token; B presents it | CONFIRM_MISMATCH; A can still apply its own | `wb1` has no session binding | `test/mrtr-confirm.test.ts` |
| 7 | Cross-session undo | A edits; B runs `undo` default scope | B rewinds nothing ("nothing to undo in session scope"); B with `scope:"workspace", ops:[seq]` rewinds with conflict surfacing; A's `undo` rewinds its own | all ops anonymous, so B rewinds A's op | `test/undo.test.ts` (extend) |
| 8 | Session id survives daemon restart | A edits, daemon is killed, A's bridge reconnects, A runs `undo` | A's own op is rewound under session scope | n/a | `test/multi-client.test.ts` |
| 9 | Daemon crash mid-session | `kill -9` the daemon while A has a pending `search` | A receives one ENGINE_UNAVAILABLE envelope with recovery `retry` and the daemon diagnostics in notes; the next call succeeds on a new daemon; the tool list never changed | server dies with the process | `test/multi-client.test.ts` |
| 10 | Daemon crash mid-write | kill between journal append and swap (fault injection seam in write engine) | on restart the file is either before or after content, never torn; `write.lock` is broken by dead pid; journal row present | same rule, single process | `test/write-engine.test.ts` (extend) |
| 11 | Bridge crash / client EOF | close A's stdin while B is active | A's connection drops; B unaffected; daemon connection count decrements | n/a | `test/multi-client.test.ts` |
| 12 | Idle shutdown | last bridge exits; wait past `daemon.idle_ms` (test value) | daemon exits; socket, `daemon.json`, lock removed; a new bridge starts a new daemon | n/a | `test/multi-client.test.ts` |
| 13 | Idle shutdown vs build | last bridge exits during a Tier-1 build | daemon waits for `building:false`, then exits | n/a | `test/multi-client.test.ts` |
| 14 | External edits | `git checkout` / direct write while A and B are connected | both see fresh `search mode:references` after the watcher debounce; `read` never returns stale line numbers (mtime+size verify) | works | `test/watcher-wiring.test.ts` (extend with two sessions) |
| 15 | Symlinked root | connect via `/alias -> /real` and via `/real` | one daemon, one `.workbench`, two connections | two `root` strings, one lock | `test/multi-client.test.ts` |
| 16 | Git worktrees | two worktrees of one repo | two daemons, two `.workbench`, independent journals | already independent | `test/multi-client.test.ts` |
| 17 | Long socket path | root whose `.workbench/run/daemon.sock` exceeds 104 bytes | daemon binds under the runtime dir; `daemon.json.socket` names it; bridge connects | n/a | `test/multi-client.test.ts` |
| 18 | Permissions | inspect `run/` mode and the runtime-dir fallback | 0700 directories; connect from another uid refused (documented, manual on macOS) | n/a | `test/redteam-jail.test.ts` (extend) + manual |
| 19 | Version skew | daemon of build X running; bridge of build Y connects | VERSION_MISMATCH → retire-when-idle → new daemon; old sessions finish; new session answers | n/a | `test/multi-client.test.ts` |
| 20 | Legacy exclusive + daemon | daemon running; `workbench-mcp --exclusive <ws>` | exit 75 naming the daemon pid and the `daemon stop` action; reverse order also refuses | lock exists | `test/workspace-runtime.test.ts` (extend) |
| 21 | Daemon unreachable, actionable error | make the socket unbindable (occupied path, wrong perms) | bridge still answers `initialize` and `tools/list`; `tools/call` answers ENGINE_UNAVAILABLE with notes naming socket, log, next action; exit code 0 | exit 75 / silent tool loss | `test/multi-client.test.ts` |
| 22 | Statelessness across sessions | the existing gate, run through two bridges instead of two processes | byte-identical envelopes | passes per process | `test/statelessness.test.ts` (extend) |
| 23 | **Codex parent/subagent** | isolated `CODEX_HOME`, one `mcp_servers.workbench-mcp` entry, `codex exec` with `spawn_agent` | parent and subagent both call `read` successfully; wrapper log shows one daemon spawn and two bridge launches, zero exit-75; the subagent's tool inventory keeps all nine names | subagent: "not available to the model", second process exit 75 | `scripts/codex-subagent-check.ts` (opt-in; verdict from the parent's `--json` events, the `--output-schema` final answer and the wrapper launch log; FAIL exit 1 on 2026-09-09 with exactly today's observables) |
| 24 | Magistr cockpit live connection | `magistr run` with a live agent session, cockpit opens the Changes tab | cockpit connects instead of remembering a refusal (Magistr-side change; recorded as a signal) | refused | manual, Magistr repo |

Rows 1–22 are required before the default flips (Phase 4). Row 23 is required before
Phase 4 on any machine with Codex installed and is the release note's proof. Row 24 is a
follow-up in the Magistr repository.
