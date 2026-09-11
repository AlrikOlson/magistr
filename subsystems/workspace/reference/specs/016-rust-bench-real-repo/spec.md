# Feature Specification: Rust Bench at 1M LOC

**Feature Branch**: `016-rust-bench-real-repo`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description (chunk:backlog-bench-real-repo): "Give bench-index.ts /
bench-refs.ts a --root mode over a real repository, then record the 1M-LOC release profile
(tier-0, tier-1 time, references p95, IPC p95) on a real Rust monorepo checked out locally,
committed beside the synthetic baseline."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:backlog-bench-real-repo` only. It changes the three
> bench scripts and adds a committed results directory; it does NOT change the server, the
> engine child, the index, any tool, the CI gates, or the numbers those gates enforce. The
> synthetic 500k-LOC profile CI runs today keeps running unchanged.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A real repository can be benchmarked (Priority: P1)

A maintainer points the index and references benches at a repository that already exists on
disk instead of the generated corpus. Today both scripts generate a deterministic synthetic
tree in a temp dir, index it, and delete it; nothing can be pointed at a real tree, so every
recorded number is a synthetic-corpus number (specs/005 plan.md item 7). After this feature
`bun run bench:index -- --root <path>` and `bun run bench:refs -- --root <path>` index the
given tree through the same engine child, the same frames, the same tier-0 probe, the same
polling and the same gates, print the same report shape, and leave the tree exactly as they
found it (the index they created is removed unless asked to keep it).

**Why this priority**: it is the harness change the chunk was filed for; without it the
release profile cannot be recorded on anything real.

**Independent Test**: point both benches at a small fixture directory with the stub core;
the run answers a report, writes the JSON when asked, removes the index it created, and
never deletes or modifies a file of the fixture.

**Acceptance Scenarios**:

1. **Given** a directory with source files, **When** `bench-index.ts --root <dir>` runs,
   **Then** no corpus is generated, the tree is indexed from its own files, the tier-0 probe
   answers a real lexical hit from a file in that tree, tier-1 is polled to settled, source
   LOC / file counts / languages are reported, and the directory's files are unchanged
   afterwards with no `.workbench/` left behind.
2. **Given** the same directory, **When** `bench-refs.ts --root <dir>` runs, **Then** the
   reference queries are sampled from the symbols the index actually holds (most-frequent
   names as the hot queries, single-definition names as the rare ones, one guaranteed miss)
   and p50 / p95 / max are reported per query and overall.
3. **Given** `--json <file>`, **When** either bench runs (synthetic or `--root`), **Then** a
   machine-readable report with the corpus facts, machine facts, measurements and every
   gate's value / limit / pass is written, and the console report is unchanged.
4. **Given** a root that already has a `.workbench/` directory, **When** a bench runs without
   `--keep-index`, **Then** it refuses before touching anything, saying why.
5. **Given** `--probe <relpath>:<pattern>`, **When** `bench-index.ts --root` runs, **Then** the
   tier-0 probe searches that file for that pattern; without the flag a deterministic default
   is derived from the root (Cargo.toml, then README.md, then package.json).

---

### User Story 2 - The 1M-LOC release profile is a recorded number (Priority: P1)

The constitution's perf gates (tier-0 usefulness within 5 s, 500k LOC to tier 1 in under
60 s, references p95 under 50 ms at 1M LOC, IPC p95 under 1 ms) have only ever been measured
on the synthetic corpus. After this feature one run of both benches over a real Rust monorepo
of at least 1M lines of Rust, checked out locally, is committed as data: the repository and
its commit, the machine, the corpus facts, tier-0 seconds, tier-1 seconds with the LOC they
covered, references p50/p95/max, IPC p50/p95, and each gate named with pass or fail. A fail
is recorded, not hidden.

**Why this priority**: Constitution VI says no number, no merge; the Rust dogfood gate depends
on this chunk precisely for the number.

**Independent Test**: the committed results files parse, name a repository commit, carry at
least 1,000,000 source LOC, and every gate row has a boolean `pass`.

**Acceptance Scenarios**:

1. **Given** the local checkout of the chosen repository, **When** `bench-index.ts --root`
   and `bench-refs.ts --root` run with `--json`, **Then** the two reports are committed under
   the results directory with the repository URL and commit recorded.
2. **Given** the tier-1 gate is defined per 500k LOC, **When** the corpus is larger, **Then**
   the report states the measured seconds, the LOC, and the normalized seconds per 500k LOC,
   and judges pass/fail on the normalized figure, saying so.
3. **Given** the references p95 gate, **When** the corpus is at or above 1M LOC, **Then** the
   50 ms limit applies as written and its pass/fail is recorded.
4. **Given** the build does not settle before the deadline, **When** the bench stops,
   **Then** the report records tier-1 as not reached with the files indexed so far and the
   gate as failed. Timeout is data.

---

### User Story 3 - The synthetic baseline sits beside it as data (Priority: P2)

The synthetic numbers live as prose in a plan document. After this feature the synthetic
500k profile, the synthetic 1M profile (`--loc 1000000`, the release profile the references
bench already names in its header) and the IPC bench are recorded as JSON in the same
directory, from the same machine and session, so the real-repository run has a like-for-like
comparison beside it, and a README table reads all of them at a glance.

**Independent Test**: the results directory holds one JSON per (script, profile) with the
same schema, and the README table's numbers equal the JSON's.

**Acceptance Scenarios**:

1. **Given** the synthetic benches, **When** they run with `--json`, **Then** their reports
   use the same schema as the `--root` reports (profile named `synthetic-500k` /
   `synthetic-1m`).
2. **Given** `bench-ipc.ts --json <file>`, **When** it runs, **Then** the IPC p50/p95/p99 and
   its gate are written in the same schema.
3. **Given** the README, **When** a reader compares the synthetic and real rows, **Then** the
   machine, the core flavour (native or wasm) and the workbench commit are stated for each.

---

### Edge Cases

- The root is not a directory or does not exist: refused with a clear message, exit 2.
- The root holds no symbols (an empty or non-source tree): the references bench reports the
  measurement as vacuous and fails, as it does today when the hot query has no edges.
- The native core cannot be loaded and the wasm fallback runs: the report records the flavour;
  the numbers are still recorded but labelled.
- `.gitignore`d and junk-flagged paths: the bench measures what the index does (gitignored
  files are not walked; junk is flagged and still indexed); LOC counts source rows that are
  not junk and says so.
- The corpus contains deliberately malformed sources (a compiler test suite does): the bench
  measures the indexer over them as they are; a crash is a finding for the build, recorded.
- A path outside the workspace jail is irrelevant here: the benches drive the engine child
  directly, as they already do.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `bench-index.ts` and `bench-refs.ts` MUST accept `--root <path>` and, when
  given, skip corpus generation, index that tree through the same engine-child frames, and
  never delete or modify the tree; the index directory they create MUST be removed at exit
  unless `--keep-index` is given, and a pre-existing index directory MUST make the bench
  refuse to run without `--keep-index`.
- **FR-002**: Both benches MUST report the same measurements in `--root` mode as in synthetic
  mode (tier-0 seconds, tier-1 seconds and settle status, references p50/p95/max and edge
  counts) plus corpus facts the synthetic mode derives by construction: source LOC counted
  over the index's non-junk source files, file counts, junk count, per-language files and
  LOC, and, when the root is a git checkout, its remote URL and commit.
- **FR-003**: `bench-index.ts --root` MUST take its tier-0 probe from `--probe
  <relpath>:<pattern>` or, absent that, from a deterministic default derived from the root.
- **FR-004**: `bench-refs.ts --root` MUST sample its queries from the built index: the three
  most frequent symbol names of at least three characters (hot), three single-definition
  names picked at fixed quartiles of the sorted list (rare), and one absent name (miss);
  the same iteration count and percentile report as today, plus per-query percentiles.
- **FR-005**: All three benches MUST accept `--json <path>` and write one report in a shared
  schema: schema id, script, profile, corpus facts, machine facts (platform, arch, CPU, cores,
  memory, bun version, core flavour, workbench commit), measurements, and gates as
  `{ name, value, limit, unit, pass, note }`.
- **FR-006**: Gates MUST be judged as the constitution states them: tier-0 5 s as written;
  tier-1 on seconds normalized per 500k LOC against 60 s, the note saying so; references
  p95 50 ms as written; IPC p95 1 ms as written. A failed gate is recorded with `pass:false`
  and the process exit code stays non-zero unless `--report-only`.
- **FR-007**: A `--deadline <s>` MUST bound the tier-1 poll; the synthetic defaults are
  unchanged and `--root` defaults to a longer bound; a deadline reached is recorded as tier-1
  not reached, not as a crash.
- **FR-008**: The recorded runs MUST be committed as JSON under a results directory with a
  README table: synthetic 500k, synthetic 1M, IPC, and the real repository (index and
  references), each with machine and commit facts.
- **FR-009**: The CI invocations (`bench:index`, `bench:refs`, `bench:ipc`) and the numbers
  they gate MUST be unchanged; `--root` never runs in CI.
- **FR-010**: The shared helpers (argument parsing, probe default, LOC counting, sampling,
  gate normalization, report writing) MUST be unit-tested without a real repository, and the
  `--root` mode MUST be exercised end to end over a fixture with the stub core.

### Key Entities

- **Bench report**: one JSON per (script, profile): corpus, machine, measurements, gates.
- **Corpus facts**: kind (synthetic | root), root path, LOC, files, junk files, languages,
  repository URL and commit.
- **Gate**: name, measured value, limit, unit, pass, note (the normalization rule when one
  applies).
- **Reference sample**: role (hot | rare | miss), name, definition count, per-query p50/p95,
  edges answered.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Both benches run in `--root` mode over a fixture with the stub core, answer
  the full report, write the JSON, leave the fixture byte-identical and remove the index
  they created; a pre-existing index directory is refused. Pinned by a test.
- **SC-002**: The shared helpers are pinned by unit tests: probe default derivation, LOC
  counting over a files table, hot/rare/miss sampling over a symbols table, gate
  normalization (tier-1 per 500k LOC), report schema.
- **SC-003**: One recorded run over a real Rust monorepo of at least 1,000,000 source LOC is
  committed with its repository commit, the machine, and all four constitution gates named
  with pass/fail. Committed as data whether they pass or not.
- **SC-004**: The synthetic 500k, synthetic 1M and IPC reports from the same machine are
  committed beside it in the same schema, and the README table matches the JSON.
- **SC-005**: Full gate sweep green; CI bench invocations unchanged; no behaviour change
  under `src/` or `crates/`.

## Assumptions

- A real Rust monorepo of at least 1M LOC can be shallow-cloned onto the build machine; the
  chosen repository is recorded in research.md with its measured size and the reasons the
  alternatives were rejected. It does not need to build with cargo: the benches index, they
  do not compile.
- The native core artifact is present on the recording machine; the report records the
  flavour so a wasm-fallback run cannot be mistaken for the release profile.
- Numbers are machine-specific by nature; the report carries the machine facts and the
  README says which machine produced them, as the 013 SC-006 record does.
