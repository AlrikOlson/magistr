# Feature Specification: cargo Runner Depth

**Feature Branch**: `013-rust-cargo-runner-depth`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description (chunk:rust-cargo-runner-depth): "Deepen the cargo runner adapter:
cargo test and cargo nextest failure extraction (test name, file:line, assertion, trimmed
backtrace), cargo build/clippy --message-format=json diagnostics as structured rows,
per-workspace-member command seeding, timeout => partial output as data."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:rust-cargo-runner-depth` only. The public
> `workbench:runner@0.1.0` interface is NOT changed (its record fields are frozen at this major,
> wit/README.md rule 3); the cargo adapters stay bundled TypeScript adapters and do not move onto
> the component lane (that was the go adapter's job, chunk:m5-runner-adapter); `run`'s no-shell
> guard, allowlist and envelope contract are untouched; no `cargo fmt` command is seeded (the
> formatter shipped as specs/012).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A failing Rust test run comes back as failures, whichever runner ran it (Priority: P1)

An agent runs the declared `cargo-test` (or `cargo-nextest`) command over a Cargo workspace.
Today `cargo test` output is parsed only when a test's panic message begins with `assertion`
— a plain `panic!("boom")` yields an empty assertion, the file keeps Windows backslashes, a
workspace run's summary is whichever binary printed last, and `cargo nextest run` output is not
parsed at all (it falls to head+tail). After this feature every failing test — from either
runner — is one row with its name, its workspace-relative file, its panic message and a trace
trimmed to the panic location plus the frames inside the workspace, and the summary counts the
whole run.

**Why this priority**: it is the chunk's purpose and the difference between an agent that fixes
the failing test and one that reads 300 lines of runner chatter to find it.

**Independent Test**: feed captured `cargo test` and `cargo nextest run` output (checked-in
goldens from real runs) through `run`; every failure is extracted with the §9.6 quartet; the
expected rows are byte-equal to the goldens.

**Acceptance Scenarios**:

1. **Given** `cargo test` output where a test panicked with a plain message, **When** it is
   parsed, **Then** the failure's assertion is that message (the line after `panicked at`), its
   file is the workspace-relative path with forward slashes, and its trace begins with
   `<file>:<line>:<col>`.
2. **Given** `cargo test --workspace --no-fail-fast` output from several test binaries,
   **When** it is parsed, **Then** every failure across every binary is a row, two same-named
   tests in different binaries are distinguishable, and the summary aggregates passed/failed
   across binaries and carries cargo's `to rerun pass …` hints.
3. **Given** `cargo nextest run` output, **When** it is parsed, **Then** each `FAIL` line
   becomes one row named `<binary-id> <test>` with the panic message and location from its
   stderr block, the repeated `FAIL` lines under the final summary are not duplicated, and the
   summary is nextest's `Summary` line.
4. **Given** output captured with `RUST_BACKTRACE=1`, **When** it is parsed, **Then** the
   trace keeps the panic location and the frames whose paths are inside the workspace and drops
   standard-library and toolchain frames.
5. **Given** a test run whose output is neither runner's shape (for example an exit 101 with
   no test lines at all), **When** it is parsed, **Then** no structure is claimed and the
   head+tail fallback answers, as today.

---

### User Story 2 - A build or lint run comes back as diagnostics (Priority: P1)

An agent runs `cargo-build`, `cargo-check` or `cargo-clippy`. Today the output is head+tail
text. After this feature compiler and clippy diagnostics — level, code, message, file, line,
column and the help/notes — are structured rows, whether the command emits rustc's JSON
(`--message-format=json`) or the rendered human form the seeded commands emit today, and the
two forms yield identical rows for the same build.

**Why this priority**: the chunk names it, and a compile error is the most common Rust failure
an agent has to act on; a diagnostics row that carries `file:line:col` is directly actionable.

**Independent Test**: feed captured JSON and human `cargo build` / `cargo clippy` output through
`run`; the diagnostics rows match the goldens; the JSON and human captures of the same build
produce equal diagnostics.

**Acceptance Scenarios**:

1. **Given** `cargo build --message-format=json` output containing errors and warnings,
   **When** it is parsed, **Then** each compiler message becomes a diagnostic with its level,
   code (when present), message, primary span's file/line/column and its help/note children;
   `compiler-artifact` and other non-message lines are ignored; duplicates (the same warning
   from a lib and its test build) collapse to one.
2. **Given** the human `cargo build` output for the same build, **When** it is parsed,
   **Then** the diagnostics are equal to those from the JSON form.
3. **Given** `cargo clippy` output (either form), **When** it is parsed, **Then** lint
   diagnostics are rows with the lint name as their code.
4. **Given** a green build (exit 0, no warnings), **When** it is parsed, **Then** the result
   is a diagnostics row with zero diagnostics and a summary saying so — not head+tail.
5. **Given** a `cargo test` run that failed to compile, **When** it is parsed, **Then** the
   compile errors are the diagnostics rows (there are no test results to report).
6. **Given** more diagnostics than the budget allows, **When** the row is cut, **Then** errors
   are kept before warnings and the cut carries a truncation manifest (dropped count, reason,
   narrowing hints, cursor) — never silent.
7. **Given** a non-zero exit with no recognizable diagnostic (a linker failure, a missing
   subcommand), **When** it is parsed, **Then** location-less `error:` lines are still rows,
   and when there are none at all the head+tail fallback answers.

---

### User Story 3 - `init` seeds a Cargo workspace honestly (Priority: P2)

A human runs `workbench-mcp init` on a Cargo workspace. Today it seeds `cargo test` and
`cargo build`. A bare `cargo test` on a workspace stops at the first failing test binary and
hides every later one. After this feature a workspace is seeded with a whole-workspace test
command that runs every binary, one `cargo test -p <member>` per workspace member, and — only
when the project declares them — nextest and clippy commands.

**Why this priority**: it is in the chunk's acceptance, and it is what makes User Story 1's
"every failure across every binary" reachable from a seeded command.

**Independent Test**: `detectCommands` over a fixture workspace (glob members, an excluded
member, a root package) seeds the expected table; a single crate seeds exactly as before;
nextest/clippy seeds appear only with their declaration files.

**Acceptance Scenarios**:

1. **Given** a root `Cargo.toml` with `[workspace] members = ["crates/*"]`, **When** commands
   are detected, **Then** `cargo-test` is `cargo test --workspace --no-fail-fast`,
   `cargo-build` is `cargo build --workspace`, and each member gets
   `cargo-test-<name> = "cargo test -p <name>"` with its manifest as provenance.
2. **Given** a member listed under `exclude`, **When** commands are detected, **Then** it is
   not seeded.
3. **Given** a single-crate project (no `[workspace]`), **When** commands are detected,
   **Then** the seeds are `cargo test` and `cargo build` exactly as today.
4. **Given** `.config/nextest.toml` exists, **When** commands are detected, **Then**
   `cargo-nextest` is seeded; **Given** `rust-toolchain.toml` lists the `clippy` component,
   **Then** `cargo-clippy` is seeded; absent those files, neither is.
5. **Given** an existing `[commands]` table, **When** `init` runs, **Then** it is left
   untouched (unchanged behavior).

---

### User Story 4 - Timeouts and long runs stay data (Priority: P2)

A long `cargo test` exceeds the timeout. The tool already returns the partial output as data
(`timedOut: true`, exit code null, a note) rather than an error. This story verifies that
contract holds through the new parsers and states plainly that it is not extended.

**Independent Test**: the existing timeout pin passes unchanged against the new adapter set.

**Acceptance Scenarios**:

1. **Given** a run that timed out, **When** it is reported, **Then** the row is the partial
   head+tail with `timedOut: true` and the note — unchanged.

---

### Edge Cases

- A test whose stdout block contains lines that look like another test's header: blocks are
  delimited only by `---- <name> stdout ----` and `failures:`, as today.
- Windows paths everywhere (`crates\alpha\src\lib.rs`): normalized to forward slashes in
  `file`, in trace location lines and in diagnostics.
- Backtrace frames under the crate directory print as `.\src\lib.rs:11` (relative to the
  crate, not the workspace): kept as printed; only the panic location is workspace-relative.
- nextest statuses other than `FAIL` (`TIMEOUT`, `SIGABRT`, `SIGSEGV`, `ABORT`): rows whose
  assertion is the status when no panic message exists; `PASS`, `SLOW`, `LEAK` and
  `FLAKY` (passed on retry) are not failures; `TRY n FAIL` lines belong to the final status.
- nextest cancelled the run (`Cancelling due to test failure: N tests still running`): the
  summary notes the cancellation.
- Both runners write compile warnings above the tests: ignored by the test parsers; the
  diagnostics extractor only claims a test command's output when the build failed.
- JSON output interleaved with human lines (cargo's own `Finished`/`error: could not compile`
  on stderr): non-JSON lines are ignored in JSON mode.
- A workspace with more members than a table should carry: at most 32 per-member seeds, and
  `init` says how many were not seeded.
- Glob members beyond `<dir>/*` (`**`, character classes): not expanded; recorded in research.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `cargo test` output MUST be parsed so that every failing test yields one failure
  with its name, its workspace-relative forward-slash file, its panic message (whether or not
  it begins with `assertion`) and a trace whose first line is `<file>:<line>:<col>`.
- **FR-002**: `cargo nextest run` output MUST be parsed by a dedicated bundled adapter: one
  failure per `FAIL`-class line named `<binary-id> <test>`, the panic message and location
  from the test's stderr block, no duplicates from the trailing summary, and nextest's
  `Summary` line as the summary.
- **FR-003**: Multi-binary `cargo test` output MUST attribute failures to their test binary,
  keep same-named tests distinct, aggregate the summary across binaries and carry cargo's
  rerun hints.
- **FR-004**: Backtraces MUST be trimmed to the panic location plus frames whose paths are
  inside the workspace; standard-library and toolchain frames MUST be dropped; the trace MUST
  stay within the existing line cap.
- **FR-005**: `cargo build`, `cargo check` and `cargo clippy` output MUST become a
  `diagnostics` row (a new row kind beside `report` and `output`) carrying level, code,
  message, file, line, column and detail per diagnostic, from rustc JSON when the command uses
  `--message-format=json` and from the rendered form otherwise; both forms MUST yield equal
  rows for the same build; duplicates MUST collapse.
- **FR-006**: The diagnostics row MUST be trimmed to the budget with errors kept before
  warnings and a truncation manifest on any cut.
- **FR-007**: A `cargo test` / `cargo nextest` run that failed to compile MUST answer with the
  compile diagnostics; a green build MUST answer with an empty diagnostics row; a non-zero exit
  with no recognizable structure MUST fall back to head+tail.
- **FR-008**: The public `workbench:runner@0.1.0` interface MUST NOT change; diagnostics are a
  host-side (TypeScript) extraction seam and never cross the plugin boundary.
- **FR-009**: `detectCommands` MUST seed a Cargo workspace with `cargo test --workspace
  --no-fail-fast`, `cargo build --workspace`, and one `cargo test -p <member>` per member
  (members read textually from `[workspace] members`/`exclude`, `<dir>/*` globs expanded from
  the filesystem, a root `[package]` included), capped at 32 with the remainder reported; a
  single crate MUST seed exactly as today.
- **FR-010**: `cargo-nextest` MUST be seeded only when `.config/nextest.toml` exists and
  `cargo-clippy` only when `rust-toolchain.toml` lists the `clippy` component; detection MUST
  still execute nothing.
- **FR-011**: The timeout contract (partial output as data, `timedOut: true`, a note) MUST hold
  unchanged.
- **FR-012**: Every parser MUST be pinned by checked-in goldens captured from real runs, and a
  toolchain-gated test MUST run the real `cargo test`, `cargo nextest run` (when installed) and
  `cargo build` over a fixture so the goldens cannot go stale silently.
- **FR-013**: The `run` tool's schema description MUST name the diagnostics row; the routing
  skill MUST stay consistent with it (`lint:skill` decides).

### Key Entities

- **Failure (quartet)**: unchanged — test name, optional file, assertion, trimmed trace.
- **Diagnostic**: level, optional code, message, optional file/line/column, detail (the
  snippet and help/notes).
- **Diagnostics row**: the new `run` row kind carrying diagnostics plus the run facts.
- **Diagnostic extractor**: the host-side seam (`canExtract`, `extract`) consulted before
  runner adapters.
- **Workspace member**: a Cargo package named by the root manifest, seeded as a command.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Goldens for `cargo test` (single binary, plain panic, no-fail-fast multi-binary,
  backtrace) and `cargo nextest run` (multi-binary, cancelled) produce the expected failure
  rows byte-for-byte — pinned.
- **SC-002**: Goldens for `cargo build` (JSON and human, errors + warnings, green) and `cargo
  clippy` (JSON and human) produce the expected diagnostics rows; JSON and human forms are
  equal for the same build; the budget cut carries a manifest — pinned.
- **SC-003**: A compile-failed `cargo test` answers diagnostics; an unrecognizable failure
  answers head+tail — pinned.
- **SC-004**: `detectCommands` over the fixture workspace seeds the expected table; the single
  crate seeds as before; nextest/clippy seeds are declaration-gated; `init` still never clobbers
  — pinned.
- **SC-005**: The existing timeout pin, the six-adapter claim pin and the runner-plugin golden
  suite pass unchanged apart from the new adapter's presence.
- **SC-006**: The toolchain-gated live test ran on the build machine over the fixture workspace
  and matched the goldens' shape (recorded in the ship trace).
- **SC-007**: `wit/baseline.json` unchanged; full gate sweep green.

## Assumptions

- rustc 1.95 prints `thread '<name>' (<id>) panicked at <path>:<line>:<col>:` with the message
  on the following line; older toolchains print the message on the same line after the colon —
  both shapes are accepted.
- cargo-nextest 0.9.x writes its report to stderr with `FAIL [ time ] (i/n) <binary-id>
  <test>` lines and `stdout ───`/`stderr ───` blocks; the `Summary [ time ] N tests run: …`
  line closes the run. Verified on 0.9.120 during shaping.
- rustc's JSON diagnostic shape (`reason: compiler-message`, `message.{level, code, message,
  spans, children, rendered}`) is stable across the 1.x series.
- cargo-nextest may be absent on CI; its live test skips with a reason; the goldens pin the
  parser everywhere.
