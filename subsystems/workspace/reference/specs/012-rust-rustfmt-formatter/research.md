# Phase 0 Research: rustfmt Formatter

Grounded in `src/ports/formatter.ts` (`FormatterPort.format({path, content}) → {content,
changed}`), `src/adapters/biome/formatter.ts` (one spawn per call over stdin/stdout, cwd = root,
10 s timeout, non-covered extensions pass through without a spawn, failure = throw),
`src/tools/write-engine.ts` (the format step: throw ⇒ note + verified unformatted content;
parse-breaking output ⇒ fallback; `format:false` skips), `src/main.ts` `composeWrite` (one
`BiomeFormatter` for everything), `src/protocol/resolver-plugin.ts` `resolveRustAnalyzer`
(`rustup which` with cwd = root, then a `--version`-confirmed PATH candidate),
`rust-toolchain.toml` (1.95.0 + rustfmt), root `Cargo.toml` (`[workspace.package] edition =
"2024"`, crates inherit), and a live probe: `echo 'fn main(){let x=1;}' | rustfmt --edition
2024` formats to stdout, exit 0; a parse error exits 1. Pinned in think:168/169.

## R1 — A routing FormatterPort; Biome and the pipeline untouched

**Decision**: `composeWrite` composes `routingFormatter({ rust: new RustfmtFormatter({root}),
fallback: new BiomeFormatter({root}) })` — `.rs` → rustfmt, everything else → the existing
Biome adapter (which already passes through what it does not cover). The write engine's format
step is unchanged except for the note wording (R6).

**Rationale**: the pipeline takes one `FormatterPort`; routing by extension is the smallest
composition that keeps Biome byte-for-byte and lets a third language join later.

**Alternatives considered**: teaching `BiomeFormatter` about rustfmt (rejected: two tools in
one adapter); a formatter list in the write engine (rejected: the engine should not know how
many formatters exist).

## R2 — Resolve rustfmt like rust-analyzer: toolchain manager first, PATH second, runnable or absent

**Decision**: `rustup which rustfmt` with cwd = workspace root (honors `rust-toolchain.toml`;
fails when the component is missing for that toolchain), else `Bun.which("rustfmt")` confirmed
by `--version` exit 0; memoized per adapter instance; injectable for tests.

**Rationale**: specs/008 already established that the rustup proxy for an uninstalled
component exists on PATH and errors when run; the same rule applies. The pin is honored for
free because rustup resolves per cwd.

## R3 — Pass the edition; read it textually from the nearest manifest

**Decision**: the adapter finds the nearest `Cargo.toml` above the file (within the root),
reads `[package] edition = "…"`; if absent or `edition.workspace = true`, reads the root
`Cargo.toml`'s `[workspace.package] edition`; default `2021`. Passed as `--edition <e>`.
Section-split regex, no TOML parser (the manifest-reading idiom of `map` and specs/010).
Memoized per manifest path for the adapter's lifetime.

**Rationale**: standalone rustfmt does not read Cargo.toml; without `--edition` a 2024-edition
crate's newer syntax can fail to parse, which would turn every such edit into the degraded
path. `cargo fmt` passes the edition; the adapter must too.

## R4 — cwd = the file's directory, so config discovery matches `cargo fmt`

**Decision**: spawn rustfmt with cwd = `dirname(root/path)`; input on stdin, output on stdout.

**Rationale**: with stdin input rustfmt has no file path to walk from, so it discovers
`rustfmt.toml`/`.rustfmt.toml` from the working directory upward; starting at the file's
directory finds a crate-local or root config exactly as `cargo fmt` would for that file.

**Alternatives considered**: `--config-path` (rejected: would need our own discovery);
cwd = root (rejected: misses crate-local configs in a workspace).

## R5 — Failure is a throw with a useful message; timeout 10 s like Biome

**Decision**: unresolvable binary ⇒ throw "rustfmt is not installed for this workspace's
toolchain (rustup component add rustfmt)"; non-zero exit ⇒ throw with rustfmt's first stderr
line; timeout (10 s) ⇒ kill + throw; spawn error ⇒ throw. The pipeline converts every throw
into the verified unformatted content plus a note.

## R6 — The pipeline's note carries the formatter's message

**Decision**: `write-engine.ts` note becomes `<path>: the formatter failed (<message>); applied
the verified unformatted content`. Additive wording; the existing Biome and write-engine tests
that match on "the formatter failed" keep matching.

**Rationale**: "not installed" and "rejected the content" call for different actions; a note
that cannot tell them apart is the kind of degradation Constitution II calls silent.

## R7 — The number: cold and warm per-edit overhead, toolchain-gated

**Decision**: a test gated on a runnable rustfmt formats a ~20-line file twice through the
adapter, records cold (includes resolution) and warm ms, and prints `[012 SC-004] …`; the
values are recorded via `ship_record`. No tight assertion (machine-dependent); the bound is the
10 s timeout.

## R8 — Tests: injected binaries for every degradation path; real rustfmt only where required

**Decision**: `RustfmtFormatter` takes `{ root, binary?, resolver?, timeoutMs? }` like the
Biome adapter's `binary` seam. Degradation pins use a fake binary script (exit non-zero; sleep
past a short timeout) and a missing path — no toolchain. Formatting, edition and rustfmt.toml
pins gate on the real binary.

## R9 — Code placement

**Decision**: `src/adapters/rustfmt/{formatter.ts,index.ts}` (adapter layer; imports ports
only) exporting `RustfmtFormatter`, `resolveRustfmt`, `editionFor`; `src/adapters/formatter-router.ts`?
No — keep the router beside the adapters it routes: `src/adapters/rustfmt/index.ts` also
exports `routingFormatter(rules)`; `composeWrite` in `main.ts` wires it. Tests in
`test/rustfmt-formatter.test.ts`; `test/write-engine.test.ts` gains the note-wording pin.
