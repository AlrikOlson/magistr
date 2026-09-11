# Phase 1 Data Model: cargo Runner Depth

No stored data changes. No WIT change (`workbench:runner@0.1.0` frozen; `wit/baseline.json`
untouched). No config schema change (`[commands]` is a dynamic table).

## New `run` vocabulary (src/tools/run.ts)

```
Diagnostic
  level     "error" | "warning"
  code?     string            E0308 | unused_variables | clippy::needless_return
  message   string            the diagnostic's own message, verbatim
  file?     string            workspace-relative, forward slashes (primary span)
  line?     number            1-based
  column?   number            1-based
  detail    string            rendered snippet + help/note lines, ≤ 12 lines

RunDiagnosticsRow
  kind          "diagnostics"
  command       string        the declared key
  resolvedLine  string
  extractor     string        "cargo"
  exitCode      number | null
  timedOut      false
  durationMs    number
  diagnostics   Diagnostic[]  errors first in emission order, then warnings
  summary       string        "<e> error(s), <w> warning(s)" | "no diagnostics"

RunRow = RunReportRow | RunOutputRow | RunDiagnosticsRow

DiagnosticExtractor
  name        string
  canExtract  (commandLine) → boolean
  extract     (output: RunOutput) → Diagnostic[] | undefined   // undefined = decline

BUNDLED_DIAGNOSTIC_EXTRACTORS = [cargoDiagnosticExtractor]

runRun(deps, input, v, adapters = BUNDLED_RUNNER_ADAPTERS, extractors = BUNDLED_DIAGNOSTIC_EXTRACTORS)
  order: timeout ⇒ output row (unchanged)
         first claiming extractor that returns Diagnostic[] ⇒ diagnostics row (budget-trimmed, manifest on cut)
         first claiming adapter that returns a report ⇒ report row (unchanged)
         else ⇒ output row (unchanged)
```

## Bundled adapters (src/tools/run-cargo.ts)

```
BUNDLED_RUNNER_ADAPTERS = [bun, vitest, jest, pytest, nextest, cargo, go]
  nextest.canParse  : cargo ∧ nextest
  cargo.canParse    : cargo ∧ test  (nextest no longer claimed by cargo)

StructuredFailure (unchanged shape; new content rules)
  testName   cargo: "<name>" (one binary) | "<stem> <name>" (several)
             nextest: "<binary-id> <name>"
  file       workspace-relative path of the panic, forward slashes
  assertion  the panic message (same line or next line), or the nextest status word
  trace      "<file>:<line>:<col>" ⏎ message continuation (left/right) ⏎ kept backtrace frames, ≤ 8 lines

summary    cargo: the one `test result:` line verbatim | "<p> passed; <f> failed; <i> ignored across <n> binaries[ — rerun: …]"
           nextest: the `Summary [ … ] …` line[ (cancelled: N still running)]
```

## Seeding (src/config/detect.ts)

```
CargoLayout
  kind      "workspace" | "crate"
  members   { name: string, dir: string }[]     name order; root [package] included
  skipped   string[]                            unexpanded glob entries
  nextest   boolean                             .config/nextest.toml exists
  clippy    boolean                             rust-toolchain.toml components include "clippy"

seeds (workspace)
  cargo-test          cargo test --workspace --no-fail-fast          Cargo.toml [workspace]
  cargo-build         cargo build --workspace                        Cargo.toml [workspace]
  cargo-test-<name>   cargo test -p <name>                           <member>/Cargo.toml   (≤ 32; remainder counted)
  cargo-nextest       cargo nextest run --workspace --no-fail-fast   .config/nextest.toml  (when present)
  cargo-clippy        cargo clippy --workspace --all-targets         rust-toolchain.toml   (when clippy listed)

seeds (single crate)
  cargo-test          cargo test                                     Cargo.toml   (unchanged)
  cargo-build         cargo build                                    Cargo.toml   (unchanged)
  cargo-nextest       cargo nextest run --no-fail-fast               .config/nextest.toml  (when present)
  cargo-clippy        cargo clippy --all-targets                     rust-toolchain.toml   (when clippy listed)
```

## Goldens (test/support/golden/cargo/)

```
cargo-test-single.txt / .expected.json         one binary, assertion + plain panic + assert!
cargo-test-workspace.txt / .expected.json      --no-fail-fast, three binaries, rerun hints
cargo-test-backtrace.txt / .expected.json      RUST_BACKTRACE=1, frames trimmed
nextest-workspace.txt / .expected.json         four FAIL across two binaries, cancelled, repeat list
build-json-warn-error.txt / .expected.json     compiler-message lines, dedupe, error + warning
build-human-warn-error.txt / .expected.json    the same build rendered — equal to the JSON case
build-green.txt / .expected.json               exit 0, empty diagnostics row
clippy-json.txt / .expected.json               lint code
clippy-human.txt / .expected.json              equal to clippy-json
```
