# Unified MCP and installation evidence — 2026-10-19

This delivery joins all Magistr-owned workflow and Workbench MCP capabilities at
`magistr mcp --dir <project>`, with selected-agent setup and macOS arm64 Cargo/npm
distribution. External MCP services remain separate. Registry publication and
the declaration of public release were not performed.

## Ownership and behavior

One workflow authority owns project records; one workspace authority owns each
working copy's source, journal and workers. The public endpoint attaches to those
owners. Direct tools, discovery and composition use the same native operation
catalog and schemas. All mutations retain native permissions, confirmations,
source hashes and journaling. Compositions stop at the first failure and return
completed receipts without replay.

Two existing owner defects surfaced during integration and were corrected at
their owners: equivalent workflow-store paths now derive the same socket
address, and settings guards explicitly unlock before closing file descriptors
that a child process might have inherited. Unknown workspace lock records still
refuse admission without modifying their bytes.

Setup applies the reviewed administrative change plan, generates only selected
integrations, and checks actual stdio initialization, read-only context and
workspace health. Preserved custom stdio commands are checked too. MCP startup
does not repair project files. Custom HTTP launchers are preserved and reported
as unsupported by the setup connection check. The starter prompt requests a
plan; setup starts no implementation.

## Real agent comparison

The task investigated a Python name-normalization defect, edited with a current
source hash, preserved the test, recorded a plan and decision, captured feedback,
added future roadmap work, ran a recorded check, and resumed in a fresh agent
process to complete the objective. Each host used the same fixture and prompts
for its baseline and candidate. Each row includes both work and resumption.

| Host and surface | Independent task verification | Tool calls | Tool errors | Input tokens, including cache | Seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| Codex, split baseline | Pass | 34 | 0 | 906,118 | 88.58 |
| Codex, unified | Pass | 29 | 1 | 568,262 | 73.90 |
| Claude Code, split baseline | Pass | 46 | 0 | 867,651 | 164.60 |
| Claude Code, unified | Pass | 36 | 2 | 732,305 | 256.57 |

[Per-turn metrics and verification](agent-evaluations.json) retain tool sequences
and native usage counters. Codex input already includes cached input; Claude's
total adds ordinary input, cache creation and cache reads. Do not compare host
token accounting or latency as though the models were interchangeable.
Claude reported model `claude-fable-5-1`; the Codex stream did not record a model
identifier in the collected metrics.

The unified Codex run recovered from an invalid roadmap grouping after its
feedback mutation had succeeded. Claude's resumption recovered from a missing
focus-lane argument and a missing estimated step count. Completed mutations were
preserved. All four conditions independently passed the original unittest and
persisted objective/task, verified-check, feedback, future-roadmap and decision
record checks.

An earlier completed Codex comparison exposed repeated schema/name discovery;
batch schema lookup, name suggestions and optional response schemas were added
before the reported run. Earlier invalid fixture/configuration trials were
excluded. A Claude trial refused a prompt framed around reasoning extraction;
both Claude conditions were rerun with prompts requesting only concise public
decisions and observed evidence. Private model thinking is excluded from this
archive and from future harness transcripts.

This is one small synthetic task per host and surface, not a statistical benchmark.
Lower calls and input tokens did not guarantee lower latency: unified Claude
resumption was slower. The reported agent trials preceded binary-hash capture in
the harness; they establish design evidence, not exact final-artifact identity.
The final binary's independent process checks and package inventories provide
that separate artifact evidence.

## Focused verification

[Native verification receipts](verification.json) record commands, real exit codes,
source snapshots and outcomes, including recovered failures. No full
workspace suite, foreground desktop journey or reference-comparison suite was
requested or run for this delivery.

- CLI library: 2,920 tests passed.
- Source-owner and public interfaces: 51 integration tests passed.
- Scaffolding/current interface: 10 integration tests passed.
- Shared settings: 231 tests passed; workflow path aliases: 3 passed.
- Claude transport: 49 tests passed; desktop command dispatch: 7 passed.
- Unified stdio scenarios: all 8 passed, covering complete catalog/schema
  coverage, simultaneous connections, separate worktrees, owner restart,
  stale confirmations, session undo, cancellation, bounded and partial failure,
  planning restrictions, and startup without repair.
- Guided onboarding: all 6 process scenarios passed, including repeated and
  multiple-agent setup, preserved custom commands, unavailable agents, repair
  and removal. The Claude plugin's actual unified transport passed its native
  process check and all 27 independent adapter contracts.
- Formatting, focused Clippy, structure, workspace-source, workflow-source and
  native-write-path gates passed.

The first owner-interface gate failed on canonical-path rendezvous and stale
fixture assumptions. The owner was corrected, fixtures were aligned with the
unified contract, and the same named gate subsequently passed. The receipt
archive retains this failure and recovery. A final caller audit also corrected the
Claude plugin's transport and remaining generated skill references. The package
identity check rejected artifacts made before those template edits; preparation
and installation were rerun for the final artifacts.

No GUI appearance changed. Desktop verification covered command dispatch only;
no native input or accessibility result is claimed.

## Distribution and release boundary

The Cargo CLI closure contains 14 version-matched packages in dependency order.
The npm tarball contains the native macOS arm64 executable, without a JavaScript
launcher or install hook. Inventories include the 550-package macOS runtime/build
dependency closure, nested notices, pinned supplemental notices and QuickJS
provenance. Every dependency has notice files; missing or changed supplemental
notices stop packaging.

The final [Cargo installation receipt](cargo-install-verification.json) and
[npm installation receipt](npm-install-verification.json) both passed on
2026-10-19 and bind the exact archive hashes. The final release binary also
passed all eight protocol and six onboarding scenarios. [SHA256SUMS](SHA256SUMS),
[Cargo file inventory](cargo-inventory.json), [npm file inventory](npm-inventory.json),
[binary/dependency inventory](binary-inventory.json), [source manifest](source-manifest.json)
and [publication order](publication-order.txt) accompany them. Prepared archives
are available locally in `target/cli-distribution/`; the CI job regenerates them.
These receipts capture the source before its delivery commit. Cargo's VCS metadata
therefore names the base commit with a dirty flag; the source manifest identifies
the verified file contents. Publication rebuilds and verifies the chosen committed
revision before uploading it.

The bundled QuickJS license is preserved byte for byte, including an upstream
space on its second line. Owned-source whitespace checks exclude that notice;
Rust and plugin formatting checks passed.

The packed-inventory assertion caught source files named `license.rs` and
`copying.rs` in the recursive notice collection. The collector now excludes
programming source files, and the same npm gate passed after repacking.

Local verification uses isolated install directories, user configuration and
workflow state on macOS arm64, without a checkout in the installation. Cargo
builds extracted normalized archives against checksum-verified vendored registry
packages. npm uses an isolated global prefix and a PATH without Rust or Bun;
a fresh cache and loopback registry exercise the generated version-pinned
`npx` registration. This is isolated local verification, not a claimed fresh VM.

The manual CI workflow prepares the same artifacts on a fresh GitHub-hosted
macOS arm64 runner. Its publishing job is separately selected and uses a protected
environment. That remote workflow and registry publication have not been run here.
The proposed npm and Cargo names were unregistered at the [recorded registry
check](registry-check.json); availability does not establish publishing access. npm ownership/trusted
publisher setup and Cargo publishing credentials remain release-owner steps.
The runner selection follows [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
See [release procedure](../../../releasing-cli.md).
