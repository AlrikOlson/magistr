# Iteration speed — 2026-10-20

This change addresses the 17 opportunities from the iteration-speed audit.
Delivery goes directly to `main`, after GitHub's required push checks on the
same commit. No pull request is needed.

| # | Opportunity | Implemented change | Evidence |
|---|---|---|---|
| 1 | Avoid whole-project CI for small edits | Select changed packages and reverse dependencies; website/docs-only changes omit Rust and packaging. Unknown/shared inputs select the full matrix. | `scripts/ci_plan.tests.py` tests both selection and refused missing results. |
| 2 | Stop defaulting bounded work to every gate | New objectives default to focused verification. Durable old objectives with no mode still load as full. Generated Claude/Codex instructions share the policy. | Objective and workflow tests; stored-state fixture preserves the old acceptance contract. |
| 3 | Remove PR/manual-check detours and repeated main verification | Eligible push checks run on `verify/**`; the same commit pushes directly to main. Main reuses a successful quality run for at most 24 hours; current security scans remain fresh. | Selector tests reject other SHAs, manual runs, failed runs and expired results. |
| 4 | Reuse valid verification | Indexed durable receipts bind command, source selection, config, toolchain and environment. Reuse refers to the original actual success, carries no fabricated exit code and is refused after a newer failure. | 46 focused evidence tests passed in receipt `de1b351e-9503-4cec-b3e0-f90f3d737c4f`; actual command/reuse/change tests cover the CLI path. |
| 5 | Remove flaky owner-lifecycle reruns | Fixtures explicitly await owner shutdown and subscriber registration is synchronized. | 700 repeated endpoint tests passed in 8.432 seconds; affected engine/workspace checks passed. |
| 6 | Stop rebuilding desktop/package jobs unnecessarily | Desktop CI is reusable and selected by affected dependencies. Release builds and manual CLI distribution share Rust cache keys. | Workflow validation; real hosted jobs remain the delivery check. |
| 7 | Remove quadratic reasoning appends | Validate and persist an append delta; update step indexes incrementally, retaining locked durable writes and external-change detection. | Actual 1,000/5,000/10,000 append measurements: 30/147/292 ms on this machine. The prior hosted 10,000-append test took 265.8–295.8 seconds; these are different hosts. |
| 8 | Start expensive tests earlier | nextest prioritizes known long tests and reserves threads for owner fixtures with shared contention. | Checked-in nextest policy and real parser validation. |
| 9 | Stop hashing unchanged large source files repeatedly | Bounded content-digest cache reuses only stable file identities; each observation still walks membership and detects additions, removals, ignore changes and replacements. Fresh/racy files are read again. | 8,980 files / 555,097,361 bytes: cold 12.109 s; warm 0.611 and 0.621 s. Source selection/cache regression tests passed. |
| 10 | Coordinate gate concurrency | One resource scheduler serves native checks, MCP gates, loop checks and setup validation. Cargo gates serialize; declared independent work overlaps within a configurable limit. Queue time counts against the deadline. | Scheduling tests cover overlap, conflicts, capacity, cancellation and abandoned waiters. |
| 11 | Avoid repeated Wasm compilation | Share immutable modules within an engine and use Wasmtime's persistent compiled-code cache across engine/process restarts. Stores and capabilities stay independent. CI restores the cache. | Cache-hit, changed-guest, distinct-engine, concurrent-compilation and isolated-store tests. |
| 12 | Compile Rust tests once | Build one nextest archive, then run up to four hash partitions from that exact archive on matching hosts. Small package selections use one worker. | CI archive workflow and selector tests; hosted execution validates transport/remapping. |
| 13 | Stop repeating failures | Normal batches stop scheduling after a required failure; the loop records the first failure once. Diagnostic mode deliberately collects all results. | Batch tests and loop fixtures assert actual invocation counts; scheduled/manual CI uses the diagnostic profile. |
| 14 | Remove duplicate compilation/check commands | An identical Cargo check is covered by Clippy. Different targets, features or directories keep their separate checks. Init runs only its promised setup check. | Resolver and validation tests, including the required-check policy after consolidation. |
| 15 | Reduce small-operation and response overhead | Architecture gates read sources once instead of launching grep/awk/hash processes per file. Mutation replies return compact summaries with receipt references. Nextest config validation uses real metadata and no build. | Structure 0.712 s; ownership 0.447 s; duplication 0.146 s; nextest config 0.148 s. All 22 existing structure/duplication fixtures and new ownership/parser refusal tests pass. |
| 16 | Reuse dependencies and browser installs | Cache npm downloads, Cargo registries, WebKit, Rust compilation and Wasmtime artifacts with dependency/tool-sensitive keys. | Pinned workflow configuration; a cache miss still performs installation/build/validation. |
| 17 | Make time limits and slow stages visible | Shared configurable gate budgets replace unrelated fixed limits. Receipts separate queue and execution time and retain the resolved deadline and its configuration origin. CI stages enforce process-group deadlines and upload timings plus slow JUnit cases for both profiles. | Actual timeout/descendant-cleanup tests; queue tests; normal and diagnostic report paths are retained. |

The scheduler/reuse/MCP change passed 318 unit tests and 20 MCP integration tests,
Clippy, formatting and the structure/ownership gates in recorded check
`d8e0c1a9-da5b-486c-b62c-51ad27d3c1ff`. The default-policy change passed 905
workflow/contracts unit tests, 35 receipt/use-case integration tests and 126
affected CLI tests during development. The final recorded check and hosted CI
are attached to delivery, rather than guessed from these intermediate runs.


The final recorded timing check `dde8ccc1-abc8-41aa-a1b5-f2e0153cfb7e`
uses the built production admission, execution, reuse and settlement APIs over
this checkout, with isolated temporary workflow state. All four results passed.

| Check | Admission | Execution | Settlement | Total |
|---|---:|---:|---:|---:|
| No-op, cold source cache | 12,303 ms | 4 ms | 712 ms | 13,020 ms |
| No-op, warm source cache | 803 ms | 4 ms | 731 ms | 1,539 ms |
| Configured structure check | 494 ms | 677 ms | 353 ms | 1,525 ms |
| Same structure check, valid reused receipt | 363 ms | 0 ms | 350 ms | 714 ms |

The audit's recorded no-op took 23,770 ms on this machine. The warm result is
about 15 times faster; a cold observation still reads the complete selected
content once. The reused result references its actual successful receipt and
does not claim another command execution. The already-running MCP server keeps
its loaded binary until the next session; these measurements use the new build.

Full verification remains available. Source scopes must cover every dependency;
opaque commands and checks of live external state do not acquire reuse merely
because their previous result was green. The project leaves live-policy,
Git-index, package-install and external-reference checks uncached. Missing tools,
invalid nextest configuration, command failures and absent required CI results
are failures, never successful skips.

The website copy/docs rewrite is included in the same direct-main delivery.
Its corpus and reviewed screenshot evidence remain in their dated evidence
folders. This optimization does not claim to publish unreleased packages or make
private public-link prerequisites available.
