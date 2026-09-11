# Feature Specification: First-Party WASM Language Pack

**Feature Branch**: `002-wasm-lang-pack`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "First-party WASM language pack on the public lang-pack interface (chunk:m5-lang-pack, SDD §8.3). A non-bundled language (candidate: gleam) becomes fully searchable and editable in workbench via a plugin pack implementing the already-blessed workbench:lang-pack@0.1.0 WIT: the pack component carries the tree-sitter grammar compiled to WASM plus tag queries and file-class heuristics; the host drives the grammar via tree-sitter's WasmStore in the engine child, registering the language into a dynamic registry consulted by languageForPath/parse/extract, with pack-language symbol extraction via tag queries rather than the static per-language tables. The twelve bundled grammars stay native with parse performance pinned unchanged, measured. The blessing/promotion path to native execution is exercised end to end: the blessing gate selects the execution lane — verified/sideload packs run the grammar in the WasmStore (the accepted 2-5x lane); a blessed pack whose language the engine child also carries as a native grammar crate is promoted to native execution, with a tier-flip test pinning identical extraction across lanes. The pack passes the TCK unmodified (m5-gate criterion) and declares zero capabilities."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A long-tail language becomes searchable and editable (Priority: P1)

A developer works in a codebase written in a language workbench does not bundle
(the first-party pack targets one such language). After installing the language
pack plugin, the developer's agent can search that codebase with symbol-aware
results — functions, types, and imports are extracted and ranked like any
bundled language — and can edit files in that language with the same
syntax-verified write guarantees the bundled languages get: a syntactically
broken edit is rejected in the same tool call.

**Why this priority**: This is the chunk's reason to exist — "a non-bundled
language is fully searchable and editable via the WASM pack" is acceptance
criterion 1, and without it the pack is decoration.

**Independent Test**: Install the pack in a fixture workspace containing
sources in the pack language; run search and confirm symbol-classified,
ranked results; attempt a syntax-breaking edit and confirm it is rejected
with a parser message; attempt a valid edit and confirm it lands.

**Acceptance Scenarios**:

1. **Given** a workspace containing files in the pack's language and the pack installed, **When** the agent searches for a symbol defined in those files, **Then** results include symbol-kind-classified matches from those files, ranked and grouped like bundled-language results.
2. **Given** the pack installed, **When** the agent submits an edit that introduces a syntax error in a pack-language file, **Then** the write is rejected in the same call with a parser message, and the file is unchanged.
3. **Given** the pack installed, **When** the agent submits a syntactically valid edit to a pack-language file, **Then** the edit lands atomically and the response reports success.
4. **Given** the pack is NOT installed, **When** the agent searches the same workspace, **Then** pack-language files still yield lexical (non-symbol) results — absence of the pack degrades, never fails.
5. **Given** the pack's file-class heuristic claims a path, **When** ranking classifies that file, **Then** the pack's classification (source/test/vendor) is honored; where the pack answers `none`, the host's built-in heuristics decide.

---

### User Story 2 - The blessing gate selects the execution lane, exercised end to end (Priority: P2)

An operator installs the same pack at different trust tiers. At sideload or
verified tier, the pack's grammar executes in the isolated WASM lane — the
documented, accepted 2–5x parse cost of the long tail. When the pack is
blessed, its language is promoted to native execution inside the engine child.
Both lanes produce identical search results and identical edit verification
verdicts; only the execution substrate (and its cost) changes.

**Why this priority**: "The blessing/promotion path to native execution is
exercised end to end" is acceptance criterion 3 and the part of SDD §8.3 no
prior chunk has touched. It is second because it presupposes story 1's lanes.

**Independent Test**: Run the same fixture extraction with the pack at
verified tier (WASM lane) and at blessed tier (native lane); diff the
extractions for equality; confirm the lane actually differs via its reported
provenance.

**Acceptance Scenarios**:

1. **Given** the pack installed at verified or sideload tier, **When** a pack-language file is parsed, **Then** execution uses the isolated WASM lane and the result is correct.
2. **Given** the pack blessed and the engine carrying a native grammar for the pack's language, **When** the same file is parsed, **Then** execution uses the native lane.
3. **Given** the same fixture set, **When** extraction runs in both lanes, **Then** symbol and import extractions are identical across lanes.
4. **Given** any tier, **When** the host reports the pack's status, **Then** the active execution lane is visible (observable provenance, not inference).

---

### User Story 3 - The hot path stays untouched, measured (Priority: P3)

A user whose workspace contains only bundled-language code sees no parse
performance change from the existence of the lang-pack feature. The bundled
grammars remain native, and the claim is pinned by measurement, not assertion.

**Why this priority**: Acceptance criterion 2 protects the v1.0 decision that
killed runtime WASM grammars in the hot path. It is third because it is a
regression guard rather than new capability.

**Independent Test**: A benchmark parses a fixed bundled-language corpus
before and after the feature lands; the delta is within noise, and the
registry lookup is shown to occur only after a bundled-table miss.

**Acceptance Scenarios**:

1. **Given** a bundled-language file, **When** it is parsed with the lang-pack feature present (pack installed or not), **Then** the native grammar is used directly and measured parse time is unchanged within measurement noise.
2. **Given** the pack installed, **When** a bundled-language file is parsed, **Then** the pack is never consulted for it — bundled identifiers shadow pack claims.

---

### User Story 4 - The pack is an ordinary, conformant plugin (Priority: P2)

The pack passes the plugin conformance kit (TCK) unmodified, declares zero
capabilities, and is subject to the same least-privilege refusals as every
other plugin. It is not a private special case.

**Why this priority**: The m5-gate criterion ("every first-party plugin passes
the TCK unmodified") makes this non-negotiable for the milestone; it rides
alongside story 1's implementation rather than after it.

**Acceptance Scenarios**:

1. **Given** the pack's component, **When** the TCK runs over it, **Then** it passes with no pack-specific accommodations.
2. **Given** the pack's manifest, **When** capabilities are resolved, **Then** the declared set is empty and the sandbox instantiates it with nothing granted.

---

### Edge Cases

- Pack claims an extension a bundled grammar already owns (e.g. a pack claiming "rs"): the bundled grammar wins unconditionally; the pack's claim is ignored with an observable note.
- Two installed packs claim the same extension: deterministic resolution (first by stable install order), with an observable note naming the loser.
- The pack's grammar bytes are malformed or fail WASM-lane loading: the pack is marked unhealthy (existing breaker semantics), pack-language files degrade to lexical-only results, nothing crashes.
- The pack's tag queries are syntactically invalid: extraction for that language degrades to no-symbols (lexical only), edit verification still works if the grammar loads; the failure is observable.
- A pack-language file with syntax errors is indexed: extraction recovers what it can (same recovery contract as bundled languages) and search still returns intact symbols.
- The pack is uninstalled after its language was indexed: subsequent searches degrade to lexical for that language; no stale symbol results survive a re-index.
- Blessed pack whose language has NO native grammar in the engine child: blessing is accepted but execution stays in the WASM lane (promotion requires the native grammar to exist); the status surface says so.
- Grammar executes pathologically (adversarial or buggy grammar loops): the WASM lane bounds execution (fuel/epoch semantics consistent with the existing sandbox posture); the native lane is reserved for blessed (audited) packs — that asymmetry is the security rationale for the tier gate.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A pack implementing the public lang-pack interface (workbench:lang-pack@0.1.0, unchanged) MUST be loadable as an installed plugin, providing language identity, grammar bytes, tag queries, and a file-class heuristic.
- **FR-002**: A language served by an installed pack MUST be recognized by path (its claimed extensions) everywhere the host resolves languages today — search indexing, edit parse-verification, and file classification — without the pack language appearing in any hard-coded table.
- **FR-003**: Symbol and import extraction for pack languages MUST be driven by the pack's tag queries; the host's static per-language extraction tables MUST NOT need modification to support a pack.
- **FR-004**: Edits to pack-language files MUST pass through the same verified-write pipeline as bundled languages: a syntax-breaking edit is rejected in the same call with a parser message (Constitution III).
- **FR-005**: The twelve bundled grammars MUST remain on their existing native path; bundled-language identifiers and extensions MUST shadow any pack claim.
- **FR-006**: At sideload and verified tiers, pack grammar execution MUST occur in an isolated WASM lane with bounded execution; the accepted cost is 2–5x native.
- **FR-007**: A blessed pack whose language the engine child also carries as a native grammar MUST execute on the native lane; the tier decision MUST be enforced at load time, and the active lane MUST be observable.
- **FR-008**: Extraction results MUST be identical across the WASM and native lanes for the same input (pinned by test over a fixture corpus).
- **FR-009**: Absence, failure, or uninstallation of a pack MUST degrade the affected language to lexical-only service — never an error surface for unrelated queries and never a crash (Constitution I).
- **FR-010**: The pack MUST pass the TCK unmodified and declare zero capabilities; the host MUST instantiate it with nothing granted.
- **FR-011**: Bundled-grammar parse performance MUST be measured before/after and pinned unchanged within measurement noise; the measurement MUST be reproducible from the repository.
- **FR-012**: All pack-facing failure modes (bad grammar, bad queries, extension conflicts, blessing without native grammar) MUST surface observable notes or existing-taxonomy errors, never silent behavior changes.

### Key Entities

- **Language pack**: An installed plugin component carrying language identity (name, extensions), grammar bytes, tag queries, and a file-class heuristic. Declares zero capabilities.
- **Dynamic language registry**: The host-side table of pack-provided languages, consulted only after the bundled table misses; each entry knows its execution lane and health.
- **Execution lane**: WASM (isolated, bounded, any tier) or native (blessed tier + native grammar present). Selected at load time by the blessing gate; observable.
- **Tag queries**: The pack-supplied query text that maps parse trees to symbol/import extractions for the Tier-1 indexer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a fixture workspace of the pack's language, symbol search returns kind-classified results for 100% of fixture-defined symbols that the pack's tag queries cover, and a syntax-breaking edit is rejected in the same tool call.
- **SC-002**: Bundled-language parse benchmark delta after the feature lands is within measurement noise (pinned threshold recorded with the benchmark), and the pack is provably never consulted for bundled-language files.
- **SC-003**: The same fixture corpus extracted in the WASM lane and the native lane yields byte-identical extraction results, and the tier-flip test demonstrates the lane change end to end (verified/sideload → WASM; blessed → native).
- **SC-004**: The TCK passes over the pack's component unmodified, and the pack's effective capability set resolves to empty.
- **SC-005**: With the pack absent or unhealthy, searches over pack-language files still return lexical results with zero errors surfaced to unrelated queries.

## Assumptions

- The public interface workbench:lang-pack@0.1.0 as blessed in the WIT baseline is sufficient and is consumed unchanged; no WIT vocabulary changes are in scope.
- The first-party pack targets one non-bundled language with a maintained tree-sitter grammar that also has a published native grammar crate (candidate: gleam), so the promotion lane can be demonstrated; final language choice is a plan-level decision with recorded criteria.
- The pack's grammar-wasm artifact is built out-of-band and committed to the repository, following the committed-component precedent of the existing first-party plugins; the build procedure is documented and reproducible.
- "Promotion to native execution" is mechanically realized as: the blessing gate selecting a native grammar crate carried by the engine child for that language. True build-time adoption of arbitrary blessed packs into native code is a registry/distribution concern out of scope for this feature.
- Search indexing for pack languages accepts tag-query-driven extraction fidelity; parity with the hand-tuned static tables of bundled languages is not required beyond what the queries express.
- Ranking file-class integration uses the pack's classify-file only where the host's own heuristics would otherwise decide by default.
