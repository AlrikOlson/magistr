# Contracts: Resolver Language-Server Session Pool

What this feature ADDS to each contract surface. Nothing existing changes shape.

## 1. `wit/subprocess.wit` — `workbench:subprocess@0.1.0 → @0.2.0` (additive)

```wit
package workbench:subprocess@0.2.0;

interface subprocess {
  // unchanged: open-error, io-error, specs, workspace-root, open, write-frame, read-frame, close

  /// True when `session` is a warm session opened by an earlier guest invoke and handed back
  /// by the host's pool. A guest whose protocol forbids repeating its handshake skips it when
  /// this is true. Always false for a guest importing subprocess@0.1.0 (per-invoke sessions).
  reused: func(session: u32) -> bool;

  /// Return the session to the host's pool without killing its server process. Sessions still
  /// open when the invoke ends normally are returned the same way; sessions held by an invoke
  /// that ends by fuel exhaustion, trap, or memory cap are killed instead. `close` still kills.
  release: func(session: u32);
}
```

Contract rules:
- Existing function signatures, types, and variant cases are frozen (wit/README rules 2-3);
  busy-lease refusal reuses `open-error.spawn-refused` with a message.
- The package stamp moves `0.1.0 → 0.2.0`; `crates/workbench-core/tests/wit_contracts.rs`
  must pass WITHOUT `WIT_BASELINE_BLESS` (within-major move) — if it demands a bless, the
  change is not additive and must be reworked.
- `plugins/lsp-resolver/guest/wit/deps/subprocess/subprocess.wit` stays byte-identical to
  `wit/subprocess.wit` (pinned by test/resolver-plugin.test.ts).
- The guest world imports `workbench:subprocess/subprocess@0.2.0`; `plugin.toml`
  `[capabilities] imports` names the same string.

## 2. Host linking — two names, two semantics

- The sandbox defines the subprocess instance under the exact import name the component
  carries: `…@0.1.0` ⇒ per-invoke sessions (today's `SubprocessPool`, `reused`/`release`
  not linked); `…@0.2.0` ⇒ lease view over the process-global `ServerPool`.
- Capability check (Rust): an import is allowed iff the effective capability list (from the
  host) contains it verbatim — unchanged. The host passes the manifest's import names.
- `is_subprocess_import(name)` (Rust) / `isSubprocessImport(name)` (TS): true for
  `workbench:subprocess/subprocess@0.<minor>.<patch>`; used for the blessed-tier gate and
  for "define the subprocess instance" decisions. `CAP_SUBPROCESS` / `SUBPROCESS_IMPORT`
  become the current (`@0.2.0`) name.

## 3. Grant matching (TS, `effectiveCapabilities`)

- A manifest import is effective when some grant names the same `package/interface@major`.
  Exact-string match remains a special case of this rule. The blessed-tier refusal note is
  unchanged in meaning and applies to any subprocess version.

## 4. Native binding (`crates/workbench-core/src/bindings.rs`) — additive

- `SandboxResolverOutcome` gains `sessions_opened: u32`, `sessions_reused: u32` (napi object
  fields; TS loader type `CoreSandboxResolverOutcome` gains the same, optional in the
  adapter-facing `ResolverGuestOutcome`).
- New `sandbox_server_pool_shutdown()` (kills every pooled session; idempotent) and
  `sandbox_server_pool_stats() -> { sessions: { languageId, root, leased, idleMs }[] }`
  (for tests and diagnostics). Both `#[cfg(feature = "sandbox")]`; the loader exposes them as
  optional methods like `sandboxResolveReferences`.

## 5. Engine child (`src/adapters/engine-ipc/engine-child.ts`) — additive

- `sandbox/resolver` result carries `sessions` through unchanged.
- After the frame loop ends (`await Promise.all([...inFlight])`), the child calls
  `binding.sandboxServerPoolShutdown?.()` before returning.
- No new IPC method is required; `IPC_VERSION` unchanged (additive result field).

## 6. Failure taxonomy

- No new error code. Busy and spawn failures are guest-visible `open-error` values that the
  guest degrades to an empty per-language answer; the host sees `ok` with fewer locations.
- Resolver unavailability remains fidelity degradation, never an error envelope (feature 001
  FR-007).

## 7. Tests that pin this contract

| Claim | Test |
|---|---|
| reuse across invokes, handshake once | `test/resolver-plugin.test.ts` pool chapter over `fake-lsp.ts slow-init/count`; Rust unit test over a scripted server in `sandbox.rs` |
| non-ok lease end kills | Rust unit tests: fuel-exhaust and trap guests with a leased session; process gone |
| busy lease refused typed | Rust unit test: two concurrent invokes, second sees `spawn-refused` |
| idle drain / empty buffer on reuse | Rust unit test: server emits notifications between invokes; second invoke's first `read-frame` is its own reply |
| shutdown kills | binding `sandboxServerPoolShutdown` test; engine-child exit test (stdin closed ⇒ server pid gone) |
| abrupt child death kills (Windows/Linux), skip on darwin | `test/engine-child.test.ts` or new `test/server-pool.test.ts`: spawn a child, warm a session with `ignore-eof` fake, `proc.kill()`, assert pid gone; `describe.if(platform !== "darwin")` with message |
| idle TTL | Rust unit test with an injectable TTL/clock |
| 0.1.0 guest unchanged | Rust unit test with a WAT/minimal component importing `@0.1.0`: per-invoke kill semantics, `reused` absent |
| grants match within major | `test/plugin-*.test.ts` / resolver-plugin adapter chapter over `effectiveCapabilities` |
| WIT additive | `cargo test --test wit_contracts` (no bless) |
| statelessness / escape / TCK unchanged | existing suites |
| real-server cold vs warm number | environment-gated scenario naming `rust-analyzer` and `cargo` |
