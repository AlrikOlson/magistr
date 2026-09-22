# Magistr harness pilot

This integration compares Codex with and without Magistr's generated project
setup on three Terminal-Bench 4.0.0 tasks. It uses Harbor 0.18.0 and Codex
0.155.1. It is a local pilot, not a leaderboard submission.

## Experiment

The fixed selection seed is `magistr-pilot-2026-09-21:`. Eligible tasks have
category Software, no GPU requirement and at most 4096 MiB memory. Sort their
names by SHA-256 of seed plus name; take the first three:

1. bun-sourcemap-leak
2. session-window-debug
3. ontology-kg-querying

Task instructions, environments and separate verifiers stay unchanged.
Each arm runs one attempt per task, sequentially, with GPT-5.6 Sol at high
reasoning effort and web search disabled. The enforced and communicated timeout
is **1,800 seconds** instead of the official 28,800 seconds. The adapter changes
only the upstream deadline sentence in the delivered prompt; task files and
requirements stay unchanged. NVM is activated for both setup and execution.

Pair order is fixed before outcomes: baseline then Magistr on Bun, Magistr then
baseline on session-window, baseline then Magistr on ontology. Each job contains
one fresh trial; all six run serially with no retries or selective restarts.
Builds, all reference controls, both arms' installation checks on each task,
and a separate generated Codex/MCP integration probe must pass before scoring.
Do not overlap trials with builds, controls or unrelated heavy host work.

Both arms install the same Codex, initialize a Git repository and receive the
same short instruction against retrieving solutions or tampering with grading.
The treatment additionally uploads the pinned Linux Magistr binary, runs
`magistr init . --agent codex --yes`, and launches Codex through
`magistr codex-launch`. It uses the generated skills, instructions and MCP
configuration. There are no task-specific Magistr settings or solution hints.
Inspect the resulting trajectories to establish which tools were actually used.

Harbor owns container execution, hidden verification, reward files and result
records. Magistr owns workspace and workflow operations inside the task.
This does not test Magistr's terminal rendering, desktop UI or long-running
roadmap recovery.

## Reproduce

Install Harbor 0.18.0 with `uv tool install harbor==0.18.0`. Use its Python
interpreter for the scripts below. Docker must be running.

Clone the pinned public benchmark outside the project:

```sh
git clone --depth 1 --branch v4.0.0 \
  https://github.com/harbor-framework/terminal-bench.git \
  /tmp/magistr-terminal-bench-4.0.0
```

Build Magistr for the Linux architecture used by your Docker task containers.
Keep its source revision and SHA-256 with the results. The original run built
commit `01ac4d293b134989dac452c37409578bf90ddab9` with Rust 1.95.0
in the ARM64 `rust:slim-bookworm` image, using `cargo build --release --locked
-p magistr-cli --bin magistr`. The build prerequisites were Git, pkg-config,
libssl-dev, Clang and CMake. Host source was supplied by `git archive`, not
mounted with personal configuration or credentials.

For a fresh build, archive the new committed revision and run the build in a
disposable container with named Cargo/target caches. Mount only that archive
and the new output directory; do not mount the working checkout or home.
Record the image ID, Rust version, source archive hash, complete build log and
binary hash. A successful incremental build is a rebuild from the new source,
not permission to copy the previous pilot's executable.

Prepare and inspect the configs first (replace the revision and paths):

```sh
/path/to/harbor/python scripts/benchmarks/terminal_bench_pilot.py \
  --checkout /tmp/magistr-terminal-bench-4.0.0 \
  --binary /absolute/path/to/magistr-linux-arm64 \
  --magistr-revision NEW_COMMITTED_REVISION \
  --output /absolute/path/to/new-results
```

Use a new output directory for each experiment. After preparation, add
`--controls` to run three reference solutions, six installation checks and the
separate integration probe. Then add `--execute` to run all six scored trials in
the frozen order. Execution requires a successful controls receipt matching the
manifest and refuses to start twice. Job errors remain recorded while the
remaining scheduled trials run; they are never retried.

Set `CODEX_AUTH_JSON_PATH` to your existing Codex auth.json in the invoking
environment. This consumes that account's Codex allowance. Do not put the file's
contents in a config or report. The integration probe uses its own fixture,
containers and model context; it never shares benchmark solutions or prior
results with scored agents.

The adapter transfers credentials only into the disposable agent environment,
outside collected logs, and removes its temporary authentication directory
after the run. Nothing is uploaded to a leaderboard or Harbor Hub.

## Inspect results

Keep `manifest.json`, the job configs, job and trial result JSON, verifier
logs, agent trajectories, and Magistr evidence. An exception or absent reward
is an infrastructure/run error, not a zero task score. A timeout is a censored
pilot attempt; show it separately even if a verifier produces a reward.

Run `python3 scripts/benchmarks/summarize_pilot.py /path/to/results` to produce
`summary.json`. Report matched per-task outcomes and raw token counts. Harbor's
price-based cost estimates are not subscription charges; absent cost is unknown,
not free. Three task pairs
cannot establish a reliable win rate. Re-run with more tasks and repeated
attempts before drawing a product-level conclusion.
