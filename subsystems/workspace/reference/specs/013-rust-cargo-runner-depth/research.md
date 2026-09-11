# Phase 0 Research: cargo Runner Depth

Grounded in `src/tools/run.ts` (`RunnerAdapter {canParse, parseOutput}` mirroring
`wit/runner.wit` verbatim; `cargoAdapter` claims `cargo test|nextest` and parses `test X ...
FAILED` + `---- X stdout ----` blocks; adapters are consulted only when the run did not time
out; the timeout path already returns partial head+tail with `timedOut: true` and a note),
`src/config/detect.ts` (Cargo.toml ⇒ `cargo-test`/`cargo-build`; pure reads),
`src/main.ts` `composeRun` (Bun.spawn with cwd = root, kill at the timeout, partial streams
returned) and `init` (never clobbers an existing `[commands]`), `wit/README.md` evolution rule
3 (a record field addition is a MAJOR), `schemas/envelope.schema.json` (`results.items: true`
— row shapes are not schema-frozen), `test/run-tool.test.ts` (the CARGO_OUTPUT fixture, the
timeout pin, the claim pin, the detect and init pins), `test/support/golden/` (the ranking and
runner-plugin golden convention, `UPDATE_GOLDEN=1`), and live probes on a three-crate scratch
workspace with rustc/cargo 1.95.0 and cargo-nextest 0.9.120 — captured verbatim for the
goldens. Pinned in think:172/173.

## R1 — Diagnostics are a host-side row kind; the runner interface stays at 0.1.0

**Decision**: a new `run` row kind `diagnostics` beside `report` and `output`, produced by a
new TypeScript-only seam `DiagnosticExtractor { name, canExtract(commandLine),
extract(output) → Diagnostic[] | undefined }` bundled in `src/tools/run-cargo.ts` and
consulted by `runRun` BEFORE the runner adapters. `Diagnostic = { level, code?, message,
file?, line?, column?, detail }`. Nothing crosses the plugin boundary; `wit/runner.wit` and
`wit/baseline.json` are untouched.

**Rationale**: `run-report` cannot grow a `diagnostics` field at 0.1.0 (README rule 3: a
record field is part of the canonical ABI signature), the chunk says the interface stays
unchanged, and a compiler diagnostic is not a test failure — forcing it into the quartet would
put a rustc error code where a test name belongs. Rows are the tool's vocabulary, not the
WIT's, and the envelope does not freeze them.

**Alternatives considered**: mapping diagnostics onto `structured-failure` (rejected: `file`
loses the line, `testName` becomes a level tag); bumping to `workbench:runner@1.0.0` (rejected:
a major for a first-party convenience, and third-party runner adapters would be cut off from
diagnostics anyway since they parse test output).

## R2 — Which cargo commands the extractor claims, and when it declines

**Decision**: the cargo extractor claims any line whose program is `cargo` with a `build`,
`check` or `clippy` subcommand, and also `test`/`nextest`. For build/check/clippy it always
answers: diagnostics (possibly zero — a green build is an empty diagnostics row with summary
"no diagnostics"), or `undefined` when the exit is non-zero and no diagnostic at all was
recognized (head+tail then answers honestly). For `test`/`nextest` it answers ONLY when there
are error-level diagnostics and no test execution lines (`running N tests`, `test … ...`,
nextest `Starting`) — a compile failure — and declines otherwise so the runner adapters parse.

**Rationale**: the seeded `cargo build` already exists in every table `init` ever wrote; a
parser that only understood `--message-format=json` would leave them as head+tail. A test run
that did not compile has nothing for a test parser; its errors are the answer.

## R3 — Two input forms, one output: rustc JSON and the rendered form yield equal rows

**Decision**: in JSON mode (the command line contains `--message-format=json`), each stdout
line that parses as JSON with `reason === "compiler-message"` and `message.level` in
{error, warning} becomes one diagnostic: `code = message.code?.code`, `file/line/column` from
the primary span (`is_primary`, else the first span), `detail` = the `rendered` text minus its
header and location lines, capped at 12 lines, with `children` folded as `help: …`/`note: …`
lines when the rendered text does not already carry them. `failure-note` level messages
("aborting due to N previous errors", "For more information…") are not diagnostics. In
rendered mode, a header `^(error|warning)(\[([A-Za-z0-9_]+)\])?: (.+)$` opens a diagnostic,
an immediately following ` --> <path>:<line>:<col>` supplies the location, and the block runs
to the next blank line. `error: could not compile …`, `error: aborting due to …`, `warning:
\`x\` (lib) generated N warning…` are summary lines, never diagnostics; other location-less
`error:` lines (linker, missing subcommand, manifest errors) are diagnostics without a file.
Dedupe by (level, code, message, file, line, column) — cargo emits a lib warning again for the
lib-test build. Paths normalized to forward slashes. Order: emission order. Summary: computed
`"<e> error(s), <w> warning(s)"` in both modes so equality is byte-exact.

**Rationale**: the rendered text inside the JSON IS the human form, so equality is a real
test, not a coincidence; computing the summary rather than scraping cargo's makes the two
modes comparable.

## R4 — The diagnostics row trims to budget with errors first and a manifest

**Decision**: measure the row as `runRun` already measures rows (JSON length / 4); drop
diagnostics from the tail of the warnings first, then errors, until it fits; any cut yields the
envelope's truncation manifest (`dropped`, `reason: "budget"`, narrowing hints "raise budget",
"fix the errors first — warnings were dropped first", cursor `diagnostic:<kept>`).

**Rationale**: Constitution II — clippy over a real workspace emits hundreds of warnings; a
row that never cuts is a dump, a cut without a manifest is silent. The `report` row's
`failures` list is equally unbounded today and is NOT changed here (filed:
chunk:run-report-budget-trim).

## R5 — `cargo test` fixes: message on the next line, forward slashes, per-binary attribution, aggregated summary

**Decision**: inside a `---- <name> stdout ----` block, the panic line is
`panicked at <path>:<line>:<col>:` optionally followed by the message on the same line (older
toolchains) or on the next non-empty line (rustc 1.95); the assertion is that message; `file`
is the path with `\` → `/`; the trace's first line is `<file>:<line>:<col>`, followed by the
message's continuation lines (`left: …`/`right: …`) and the trimmed backtrace (R7). Each
`Running unittests … (target/…/deps/<stem>-<hash>[.exe])` / `Running tests/x.rs (…)` line
names the test binary; when the output holds more than one binary, block keys and `testName`
are `<stem> <name>` (nextest's own `binary-id test` shape), otherwise `testName` stays the bare
name (the existing fixture and pin are unchanged). Summary: one `test result:` line ⇒ that
line verbatim (unchanged today); several ⇒ `"<p> passed; <f> failed; <i> ignored across <n>
binaries"` plus `" — rerun: <hint>[, <hint>]"` from the `to rerun pass \`…\`` lines.

**Rationale**: every one of these was observed in the probe: a `panic!("boom 42")` produced an
empty assertion, `crates\alpha\src\lib.rs` reached the row unnormalized, and a `--workspace
--no-fail-fast` run's summary was the last binary's `ok. 0 passed` while three tests had
failed. Prefixing only when several binaries are present keeps single-crate output — the
common case and the pinned one — byte-identical.

## R6 — A dedicated `nextest` adapter, placed before `cargo` in trust order

**Decision**: `nextestAdapter` (name `nextest`) claims lines with both `cargo` and `nextest`;
`cargoAdapter.canParse` is narrowed to `cargo test` (nextest was claimed by it and then fell
to head+tail — the claim was hollow). nextest writes everything to stderr; the adapter reads
stdout+stderr joined as every adapter does. A failure line is
`^\s*(?:TRY \d+ )?(FAIL|TIMEOUT|SIGABRT|SIGSEGV|SIGKILL|SIGILL|ABORT)\s+\[\s*[\d.]+s\]\s+(?:\(\d+/\d+\)\s+)?(\S+)\s+(\S+)$`
(binary-id, test); `TRY n` lines are ignored (the final status line follows); `FLAKY`, `PASS`,
`SLOW`, `LEAK` are not failures. The block after a failure line — `  stdout ───` and
`  stderr ───` sections, lines indented four spaces — supplies the panic line (R5's rules
apply, indentation stripped); when no panic line exists the assertion is the status word. Lines
after `Summary [` are the repeat list and are skipped; the `Summary …` line, trimmed, is the
summary, with `" (cancelled: N still running)"` appended when a `Cancelling due to test
failure: N tests still running` line was seen. `testName = "<binary-id> <test>"`.

**Rationale**: nextest's format shares nothing with libtest's beyond the inner `test x ...
FAILED` lines (which are indented and per-binary-filtered, so useless for attribution); its
FAIL lines already carry the binary id, which is exactly the disambiguation R5 has to
reconstruct for `cargo test`.

## R7 — Backtrace trimming keeps the workspace's frames

**Decision**: from `stack backtrace:` onward, frames arrive as `   N: <symbol>` followed by
`             at <path>:<line>[:<col>]`. A frame is kept when its `at` path does not start
with `/rustc/` and does not contain `.rustup`, `toolchains`, `.cargo/registry` or
`.cargo\registry`; `note: Some details are omitted…` and `note: run with RUST_BACKTRACE…` are
dropped. The trace (location line + message continuation + kept frames) is capped at 8 lines
by the existing `trimTrace`. Without a backtrace nothing changes except the location line.

**Rationale**: the probe's backtrace was 12 lines of which two (`alpha::tests::fails_panic at
.\src\lib.rs:11` and its closure) pointed at the workspace; "trimmed to the load-bearing
frames" is the WIT's own description of the field.

## R8 — Seeding: whole-workspace with `--no-fail-fast`, one command per member, declaration-gated extras

**Decision**: `detectCommands` distinguishes a workspace root (root `Cargo.toml` has a
`[workspace]` table) from a single crate. Workspace: `cargo-test = "cargo test --workspace
--no-fail-fast"`, `cargo-build = "cargo build --workspace"`, then `cargo-test-<name> = "cargo
test -p <name>"` for each member in name order, capped at 32 with the remainder counted in a
provenance note `init` prints. Members: `[workspace] members` and `exclude` string arrays read
textually (section-split, the manifest idiom of `map`, specs/010 and 012); a member entry
ending in `/*` is expanded by `readdirSync` to the subdirectories that contain a `Cargo.toml`;
other glob forms are left unexpanded and reported in provenance; a root `[package]` is a member
too; each member's `[package] name` is read from its manifest. Single crate: `cargo test` and
`cargo build`, unchanged. `cargo-nextest = "cargo nextest run [--workspace] --no-fail-fast"`
only when `.config/nextest.toml` exists; `cargo-clippy = "cargo clippy [--workspace]
--all-targets"` only when `rust-toolchain.toml` lists `clippy` under `components`. Names
`cargo-test`/`cargo-build` are kept so existing pins and human tables keep meaning the same
thing.

**Rationale**: the probe showed `cargo test --workspace` stopping at alpha's failing binary
with beta's tests never run; a seed that hides failures is worse than none. nextest and clippy
are not part of a default toolchain; the pytest precedent seeds only what the project declares,
and detection may not execute anything to find out.

## R9 — Timeout: already shipped, verified, not extended

**Decision**: no change. The existing pin ("a timeout is data") stays; the adapters remain
unconsulted on a timed-out run. Feeding partial output to the runner adapters would need an
exit code for "killed", which the WIT's `s32` cannot express without inventing a convention —
recorded as not done rather than shipped as a guess.

## R10 — Placement, goldens, live proof, schema and skill

**Decision**: parsers and the extractor live in a new pure module `src/tools/run-cargo.ts`
(`cargoAdapter`, `nextestAdapter`, `cargoDiagnosticExtractor`, `trimBacktrace`,
`normalizePath`); `run.ts` keeps the vocabulary (`Diagnostic`, `RunDiagnosticsRow`,
`DiagnosticExtractor`, `BUNDLED_DIAGNOSTIC_EXTRACTORS`) and gains an `extractors` parameter
on `runRun` defaulting to the bundled list. Goldens under `test/support/golden/cargo/`: the
captured probe outputs as `<case>.txt` inputs (absolute scratch paths in JSON lines rewritten
to `/ws`) and `<case>.expected.json` outputs, regenerated with `UPDATE_GOLDEN=1` (the ranking
convention). A toolchain-gated test builds a temp two-crate fixture, runs the real `cargo
test --workspace --no-fail-fast`, `cargo nextest run --workspace` (skipped with a reason when
nextest is absent) and `cargo build --message-format=json` through a scripted executor, and
asserts the row shape (failure names, files, non-empty assertions, diagnostic codes) — the
goldens' staleness guard. `schemas/tools/run.input.schema.json` description gains one sentence
naming the diagnostics row and nextest (`bun run codegen`); the routing skill's `run` bullet
gains the same fact; `lint:skill` decides whether more is needed.
