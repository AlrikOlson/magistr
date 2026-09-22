# Magistr benchmark pilot — September 2026

Research and execution: 21–22 September 2026, America/Los_Angeles.
Harbor's raw trial timestamps use local time without an offset; workflow
records use UTC. Durations below come from each trial's own timestamp pair.

## Benchmark choice

**Use Terminal-Bench 4.0 through Harbor to evaluate Magistr's effect on a coding
agent.** There is no authoritative ranking of benchmark trustworthiness.
This recommendation weighs fit, inspectable tasks and grading, maintenance,
and reproducibility, rather than claiming a universal consensus.

The official benchmark catalog lists Terminal-Bench 4.0 on 28 August 2026.
Its release adjusts resource budgets, fixes tasks, and removes problematic or
saturated tasks. The public release and task repository make exact inputs
inspectable and pinnable. This is the current release to use, rather than
assuming the familiar 2.0 leaderboard is still current.
[Benchmark catalog](https://www.tbench.ai/benchmarks),
[4.0 release notes](https://www.tbench.ai/news/terminal-bench-4-0),
[pinned release](https://github.com/harbor-framework/terminal-bench/releases/tag/v4.0.0).

| Candidate | Assessment for this experiment |
| --- | --- |
| Terminal-Bench 4.0 | Best primary fit for testing an agent with its workspace tools in a real terminal environment. Harbor supports custom installed agents and separate grading. |
| SWE-bench Verified | Historically prominent, but contamination and task-quality problems weaken it as the primary measure today. |
| SWE-Bench Pro | More demanding repository work, but the July 2026 audit estimated roughly 30% broken tasks and withdrew its earlier recommendation. |
| SWE-Bench Pro Verified | A promising repair introduced 8 September and revised 16 September; too new to infer established community trust. |
| SWE-rebench | Useful complementary evaluation using fresh repository tasks; its stated emphasis is isolating models through a standardized scaffold. |

Sources for those distinctions:
[Verified audit](https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/),
[Pro audit](https://openai.com/index/separating-signal-from-noise-coding-evaluations/),
[Pro Verified paper](https://arxiv.org/abs/2609.08149v2),
[SWE-rebench methodology](https://swe-rebench.com/about),
[Harbor custom agents](https://docs.harborframework.com/core-concepts/agents/custom-agents).

Terminal-Bench is not immune to measurement problems. Resource configuration
can change outcomes, so exact settings and failure categories matter.
[Anthropic's infrastructure experiments](https://www.anthropic.com/engineering/infrastructure-noise)
are one reason not to interpret a small local difference as a reliable ranking.

## Measured results

**Neither arm solved a task: baseline 0/3, Magistr 0/3.** Every official
verifier returned reward 0. Baseline had two completed failures and one timeout;
Magistr had one completed failure and two timeouts. All three upstream reference
solutions passed locally. Both jobs recorded zero retries.

| Task | Baseline outcome | Magistr outcome | Verifier checks passed, baseline → Magistr |
| --- | --- | --- | --- |
| bun-sourcemap-leak | Failed, 311.411 s | Timeout, 600.301 s | 25/36 → 30/36 |
| session-window-debug | Failed, 480.604 s | Failed, 536.509 s | 5/7 → 3/7 |
| ontology-kg-querying | Timeout, 600.179 s | Timeout, 600.245 s | 11/13 → 9/13 |

Times cover agent execution, excluding setup and grading. Partial verifier
counts are diagnostics, not fractional benchmark scores; checks differ in
importance and some test the verifier itself. The Bun baseline's 11 unsuccessful
checks comprise nine failures and two pytest errors.

| Recorded usage across three attempts | Baseline | Magistr |
| --- | ---: | ---: |
| Input tokens, including cached input | 2,693,874 | 8,136,291 |
| Cached input tokens | 2,539,264 | 7,746,816 |
| Output tokens | 52,921 | 60,532 |
| Harbor provider-price estimate | $2.6926 | $5.8673 |
| Native MCP calls | 0 | 194 |
| MCP calls returning errors | 0 | 4 |

The cost estimates are not subscription charges. Actual billed usage is unknown.
Cached and uncached tokens must not be treated as having the same cost.

Magistr's native tools were exercised: 69 calls on Bun, 83 on session-window,
and 42 on ontology. Session-window and ontology each hit the empty-file edit
refusal; session-window also received two index-not-ready edit replies.
Tool calls are deduplicated by event item ID, so started/completed events count
once. The complete per-tool breakdown is in the local `summary.json`.

This pilot demonstrates that the native integration can run under an independent
verifier, and exposes concrete setup and editing problems. It shows **no measured
task-success advantage** for Magistr under this protocol. It cannot establish a
general performance ranking: the sample is tiny, both arms hit the score floor,
and the time budget and integration limitations below materially constrain the
comparison.

## What is being tested

Magistr supplies project instructions, skills, workspace tools, and workflow
state around an installed coding agent. Its native Codex launcher delegates
model execution to Codex. The comparison therefore holds Codex and the model
constant and adds Magistr's actual generated setup to the treatment.

The owning paths are `crates/magistr-cli/src/codex_launch.rs`, the native MCP
launchers, `magistr-workspace` for workspace operations, and the native workflow
service for reasoning, plans and checks. Harbor owns isolated task execution
and the external verdict. No Magistr completion claim substitutes for that
verdict.

This tests the generated Codex integration. It does not measure terminal or
desktop rendering, multi-session roadmap recovery, or every Magistr provider.

## Fixed protocol

- Terminal-Bench tag `v4.0.0`, commit
  `452bf305c6daa62fc59061d22133a7cbc7c1572e`.
- Magistr commit `01ac4d293b134989dac452c37409578bf90ddab9`, release build,
  Rust 1.95.0, Linux ARM64. Unrelated working-tree changes are excluded.
- Harbor 0.18.0; Codex 0.155.1; GPT-5.6 Sol, high reasoning effort,
  web search disabled; the same existing Codex sign-in for both arms.
- Three tasks selected before outcomes by sorting SHA-256 of
  `magistr-pilot-2026-09-21:` plus task name. Eligible tasks are Software,
  require no GPU, and need at most 4096 MiB memory.
- One attempt per task per arm; all baseline attempts precede Magistr attempts.
  Both arms use fresh task containers. No retries or best-of selection.
- Agent cap: 600 seconds, versus the official 28,800 seconds. Task instructions
  and verifiers are unchanged. The upstream instruction's time-budget sentence still
  describes the original 28,800 seconds; this mismatch applies to both arms.
  Follow-up source inspection corrected its earlier attribution to Harbor.
- Task CPU and memory settings remain unchanged. Docker provides ARM64 Linux
  on a shared macOS host, with 18 CPUs and about 7.65 GiB total VM memory.
  The Linux build overlapped the first baseline attempt; reference and
  installation checks also overlapped baseline work. Timing cannot support
  a clean speed comparison.

The pinned checkout contains 66 tasks, including 18 Software tasks; 10 satisfy
the pilot resource filter. The three selected tasks are
`bun-sourcemap-leak`, `session-window-debug`, and `ontology-kg-querying`.
This selection excludes most of the benchmark.

All three upstream reference solutions passed their separate verifiers locally.
The native Magistr installation command also completed in a separate Bun-task
check before scored treatment trials. That verifies installation, not its
project checks: the generated check/test/lint commands failed during setup.

**This pilot is not comparable to the official leaderboard.** Three task pairs,
one attempt, one model, a much shorter timeout, a shared host, and public tasks
do not establish general superiority or statistical significance. Report each
task's verdict, timeouts, errors, usage, and observed tool calls separately.

## Setup findings and limits

The Bun task has release/smoke scripts and no declared Vitest or ESLint.
Magistr nevertheless generated npm/Vitest/ESLint/Prettier settings and
`npx tsc --noEmit`, `npm test`, and `npm run lint` checks.
The detection code defaults absent JavaScript test tooling to Vitest, absent
linters/formatters to ESLint/Prettier, and no lockfile to npm; the scaffold
loads language-preset quality commands. This is a product setup finding:
observed tooling and suggested defaults need distinct treatment.

The owning fix was recorded in the native roadmap as
`setup-evidence-backed-project-checks`. The earlier empty-file finding was
linked to the existing `workspace-empty-text-content` item. The evaluated
session-window and ontology agents independently hit that same empty-file
edit refusal; session-window also records two index-not-ready edit replies. Neither product fix
is part of this experiment, and these observations do not quantify their
causal effect on the score.

There is also an integration limitation: the adapter activates NVM for the
Codex command but not the earlier Magistr init command. This caused an initial
npm PATH warning. It does not explain why absent test/lint tooling was declared,
but it further limits causal attribution. A follow-up should use the same
runtime environment for setup and execution, communicate the actual deadline,
and run without overlapping builds or control checks.

## Reproduction and evidence

The [runner and instructions](../scripts/benchmarks/README.md) describe the
installation and invocation. The runner freezes an adapter copy, task-file
hashes, binary hash, model settings, and generated job configs.

Local artifacts are under
`target/benchmark-pilot/2026-09-21/`: `manifest.json`, `environment.json`,
`summary.json`, job/trial results, verifier logs, Codex trajectories, reference
controls, and Magistr evidence. These are local artifacts, not uploaded results.
Harbor's dollar figures are provider-price estimates, not charges to the
subscription used for this run.

The benchmark uses the original product revision; no product fix was introduced
after observing a result.

## Recommended follow-up

1. Fix new-file population in the workspace write pipeline, and make project
   setup distinguish detected tools from optional defaults. These are owning
   product changes recorded on the roadmap, not benchmark-specific patches.
2. Correct the adapter's setup environment and deadline message. Run a new
   experiment with more time and an otherwise idle host; retain this pilot as
   recorded rather than replacing its failures with selective reruns.
3. Counterbalance arm order and repeat matched tasks before expanding the task
   sample. A leaderboard comparison requires the full benchmark, its resource
   settings, and the repeated-trial protocol described in the
   [official run guide](https://www.tbench.ai/run).
