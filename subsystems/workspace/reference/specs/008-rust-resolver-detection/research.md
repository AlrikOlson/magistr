# Phase 0 Research: Rust in the Resolver Detection Table

Grounded in `src/protocol/resolver-plugin.ts` (`detectLanguageServers`, `BinaryLookup`,
`LanguageServerSpec`), `src/main.ts:519` (the one call site), `test/resolver-plugin.test.ts`
(the 007 real-server chapter that already drives rust-analyzer through the pool with a
hand-built spec), and a live probe of `rustup which rust-analyzer`. Pinned in think:154.

## R1 — Trigger: `Cargo.toml` at the workspace root

**Decision**: The Rust entry is produced when `Cargo.toml` exists at the workspace root — the
same shape as the TS entry's `tsconfig.json || package.json` check. A virtual workspace
manifest (`[workspace]` only) counts; rust-analyzer serves the workspace root either way.

**Rationale**: matches the existing heuristic's breadth-is-not-acceptance stance (feature 001
Assumptions); the map tool already treats `Cargo.toml` as the Cargo signal (test/map.test.ts).

**Alternatives**: walk upward for a workspace root (rejected: the workspace root IS the jail
root here; detection is scoped to it); require a `src/` dir (rejected: `[[bin]]`/`build.rs`-only
crates exist).

## R2 — Binary discovery: ask the toolchain manager first, verify runnability, never emit a doomed spec

**Decision**: Resolve rust-analyzer through an **injectable resolver** that, in the real
implementation:

1. runs `rustup which rust-analyzer` with `cwd = workspace root` — this returns the absolute
   binary for the workspace's active/pinned toolchain and **only** succeeds when the component
   is installed for it (probe confirmed: `…/toolchains/1.95.0-…/bin/rust-analyzer.exe`, and it
   honors a `rust-toolchain.toml` override in cwd);
2. failing that, tries a PATH `rust-analyzer` (`Bun.which`) **but** confirms it runs
   (`<bin> --version` exit 0) so the rustup **proxy** without the component — which prints
   "Unknown binary" and exits non-zero — is rejected (007 deviation 3);
3. returns the first runnable absolute path, or nothing.

Emit a Rust entry only when a path comes back.

**Rationale**: FR-003. The 007 unit already learned the proxy trap the hard way; encoding
"verify it runs" in detection means the pool never spawns a server that immediately dies.
`rustup which` is the honest toolchain-aware resolver and is what makes R3 free.

**Alternatives**: pass the bare proxy path and let the pool discover it is dead (rejected: one
wasted spawn + a breach per startup, and the breach expiry churns); hard-code
`~/.rustup/…` layout (rejected: reimplements rustup, breaks on standalone installs and
non-default rustup homes); require `RUST_ANALYZER` env (rejected: detection is zero-config,
Constitution I).

## R3 — Toolchain honesty via `rustup which` cwd

**Decision**: because resolution runs `rustup which` with `cwd = workspace root`, a
`rust-toolchain.toml` pin is honored for free — the resolved binary belongs to the pinned
toolchain. The absolute binary is handed to the pool and spawned **directly** (not via
`rustup run`); rust-analyzer locates its own sysroot at runtime, so the direct spawn answers
against the right toolchain. A pin whose toolchain lacks the component yields no path → no
entry (SC honest: the server is genuinely unavailable, tree-sitter floor holds).

**Rationale**: the probe showed `rustup which` in a dir with a `rust-toolchain.toml` override
resolves that toolchain's binary; passing the absolute path avoids a `rustup` process in the
hot spawn path.

**Alternatives**: spawn via `rustup run <tc> rust-analyzer` (rejected: puts a rustup shim in
every server process and the pool would key on the shim, not the real binary); parse
`rust-toolchain.toml` ourselves (rejected: reimplements toolchain resolution, misses
`override` and env precedence rustup already handles).

## R4 — Detection becomes IO/spawning; keep it injected and one-time

**Decision**: `detectLanguageServers` today is synchronous (`existsSync` + `Bun.which`). The
Rust path needs to run a process (`rustup which`, `--version`). Generalize the injected
`BinaryLookup` seam into a `LanguageServerResolver` dependency (the real one spawns; tests
inject a pure function). Detection stays a **one-time, startup-time** call (`main.ts:519`), off
every request path, so a couple of short exec calls at boot are acceptable (the TS entry pays
one `Bun.which`; Rust pays one `rustup which` + at most one `--version`).

**Rationale**: the m5 `BinaryLookup` injection precedent, widened; keeps every unit test
hermetic (FR-006) and the change additive (FR-007).

**Alternatives**: make `detectLanguageServers` async (rejected unless needed: the call site can
stay sync if resolution uses `Bun.spawnSync`, which is fine at startup and keeps the signature
simple); a background async detection that fills the table later (rejected: over-engineered for
a boot-time list; the pool already handles a server that is slow, not a table that is late).

## R5 — Scope is references; rename/diagnostics stay MVP-empty (amend the chunk)

**Decision**: `PluginResolver.resolve` upgrades `kind: "references"` only, and the bundled
guest's `rename()`/`diagnostics()` return empty for **every** language (not a Rust gap). So the
chunk's original acceptance "references **and rename** resolve with provenance resolver-lsp"
overreaches the shipped surface. **Amend `chunk:rust-resolver` to references-only** and **file
`chunk:resolver-rename-lane`** (guest `textDocument/rename` over LSP, host applying the edits
through the §7.5 write choke behind the §9.4 confirm gate — a cross-cutting resolver capability,
not Rust-specific) into Rust readiness.

**Rationale**: shape's job is to catch an acceptance the build cannot honestly satisfy; the
guest rename lane is real work with its own write-path integration and confirm-gate design.

**Alternatives**: stub a Rust-only rename in the guest (rejected: rename is language-agnostic
in the WIT surface; a Rust special-case would be exactly the private-hook shape Constitution V
forbids).

## R6 — Test rig: injected resolver for units, a real fixture crate for the gated pin

**Decision**: unit tests drive `detectLanguageServers` with an injected resolver over an
in-memory/temp workspace: Cargo.toml + resolver-returns-path ⇒ one Rust entry; no Cargo.toml ⇒
none; resolver-returns-null (proxy-without-component) ⇒ none; TS+Cargo ⇒ both. The
environment-gated pin builds a fixture crate (a trait with an impl method, a generic fn calling
it, and a `macro_rules!` or derive producing an item) in a temp dir, runs the real
`detectLanguageServers`, and asserts a warm references request returns `resolver-lsp` edges;
absent-rust-analyzer ⇒ byte-identical tree-sitter answer. Gated on `rustup which rust-analyzer`
succeeding, skip message naming rustup + rust-analyzer.

**Rationale**: feature 001 D7 (fake for determinism, real for the last mile) and the 007
real-server precedent, reused; the fixture is the Rust-specific addition.

## R7 — The recorded number

**Decision**: SC-004 reuses the 007 measurement shape — cold vs warm resolver-upgraded
references latency over the fixture, printed and recorded via `ship_record`, gated. It is the
same lane as 007's SC-005 but now reached through **detection** rather than a hand-built spec,
so it proves the end-to-end daily-driver path for Rust.

**Rationale**: Constitution VI; the chunk's fourth acceptance ("recorded p95 for a
resolver-upgraded references call on the warm pool").
