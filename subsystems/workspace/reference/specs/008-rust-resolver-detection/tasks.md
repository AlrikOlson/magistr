# Tasks: Rust in the Resolver Detection Table

**Input**: Design documents from `specs/008-rust-resolver-detection/` (plan.md, research.md
R1–R7, quickstart.md). Substrate pinned in think:154. Builds on the shipped warm pool
(chunk:backlog-resolver-server-pool); does not touch the resolver interface, guest, or pool.

**Tests**: included — detection is pinned both directions by units; the real path is an
environment-gated pin (Constitution VI).

**Organization**: phases map to the spec's user stories.

## Phase 1: Foundational

- [x] T001 In `src/protocol/resolver-plugin.ts` generalize the injected `BinaryLookup` into a
      `LanguageServerResolver` seam: keep `detectLanguageServers(root, resolver?)`, default
      resolver spawn-backed. Add a `resolveRustAnalyzer(root)` helper (real impl:
      `Bun.spawnSync(["rustup","which","rust-analyzer"], { cwd: root })` → trim stdout on exit
      0; else `Bun.which("rust-analyzer")` verified by `<bin> --version` exit 0; else null).
      Keep the TypeScript entry and its `BinaryLookup` behavior byte-for-byte (FR-007).

## Phase 2: User Story 1 + 3 — Rust entry, toolchain-honored (P1/P2)

- [x] T002 [US1] In `detectLanguageServers`, when `existsSync(join(root,"Cargo.toml"))` and the
      resolver returns a runnable path, push `{ languageId: "rust", bin, args: [], transport:
      "stdio", source: "detected" }`. Order: keep TS first, Rust second (deterministic table).
- [x] T003 [US1/US2/US3] Unit chapter "rust detection" in `test/resolver-plugin.test.ts` over an
      injected resolver + temp workspaces: Cargo.toml+path ⇒ one rust entry; no Cargo.toml ⇒
      none; resolver null ⇒ none; TS+Cargo ⇒ both entries in order. Assert the rust entry's
      `bin` is exactly the injected path and `args` is `[]` (SC-001).

## Phase 3: User Story 1 — real fixture references + the number (P1)

- [x] T004 [US1] Gated "rust fixture" chapter: `describe.if(rustupResolves())` (a helper that
      runs `rustup which rust-analyzer` and checks exit 0), skip message naming rustup +
      rust-analyzer. Build a temp fixture crate — a trait with a method, an impl, a generic fn
      that calls the method, and a `macro_rules!`/derive producing an item — run the REAL
      `detectLanguageServers(root)`, assert exactly one rust entry, then drive the full
      `PluginResolver` (real binding + real guest, budget generous for the cold call, polling
      the warm session until Tier-2 arrives) and assert `resolver-lsp`/`high` edges at the real
      sites; print + record cold/warm latency (SC-002, SC-004). Shut the pool down after.
- [x] T005 [US1] Trait-method / generic reference assertion inside T004: the references include
      a call site a name-based symbolic pass would miss (the generic call through the trait
      method), proving Tier-2 precision, not just presence.

## Phase 4: User Story 2 — no regression (P1)

- [x] T006 [US2] "no regression" assertion: references over the fixture with NO resolver
      configured equal the answer when the rust entry's resolver is absent/unhealthy — the
      byte-identical tree-sitter floor (SC-003). Reuse the existing search-merge pin shape.

## Phase 5: Polish

- [x] T007 Update `skills/workbench-routing/SKILL.md` only if a routed surface changed (it
      should not; `bun run lint:skill` decides), plan.md Deviations, tick this file.
- [x] T008 Server-verified gate sweep via `ship_check` (bun test, tsc, lint quartet, cargo
      test), `ship_finalize`, `roadmap_complete_chunk`, commit. (cargo test is a required gate
      even though this feature is TS-only — the tree must stay green.)
