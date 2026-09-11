# Feature Specification: Ranking-Signal Plugins in Shadow Mode

**Feature Branch**: `003-ranking-shadow`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "Ranking-signal plugins in shadow mode: plumb the blessed workbench:ranking@0.1.0 interface end to end so a ranking plugin can contribute features into the stage-5 scorer (never final orderings, first-party combiner preserved), with a newly installed signal running in shadow mode (logged to a local on-disk shadow log, not applied) until the user explicitly promotes it, promotion persisted per-signal and orthogonal to the blessing tier, shadow impact measurable as an MRR delta via the existing offline eval harness, and a first-party zero-capability ranking-visibility plugin shipped on the interface passing the TCK unmodified. The sandbox is never on the search response path: shadow scoring is fire-and-forget post-response, promoted features apply via an async-warmed feature cache."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Features only, first-party combiner preserved (Priority: P1)

A user installs a ranking-signal plugin. Whatever the plugin does, search result ordering is decided exclusively by the first-party combining model: the plugin can hand the scorer named numeric feature values for individual hits, and nothing else. A plugin cannot reorder results, cannot suppress results, cannot inject results, and cannot set a final score. With no ranking plugin installed, ordering is bit-for-bit what it was before this feature existed.

**Why this priority**: This is the leash the roadmap chunk exists to build (chunk acceptance 1, SDD §7.4/§8.3). If a plugin can influence ordering outside the combiner, the no-number-no-merge discipline is lost at the ecosystem boundary and the product's core quality loop is forfeit.

**Independent Test**: Golden ranking tests pass unchanged with no plugin installed; with a signal installed and promoted, ordering differences are provably attributable to combiner-weighted feature values (removing the weight restores the baseline order exactly).

**Acceptance Scenarios**:

1. **Given** no ranking plugin is installed, **When** any search runs, **Then** result ordering is identical to the pre-feature pipeline (pinned by existing golden tests, which must not change).
2. **Given** a promoted ranking signal, **When** a search runs, **Then** any ordering difference from the baseline is produced solely by the first-party combiner weighting the plugin's named feature values.
3. **Given** a plugin that returns malformed contributions (non-finite values, unnamed features, oversized lists), **When** scoring occurs, **Then** the malformed contributions are discarded and ordering falls back to the first-party baseline for the affected hits.

---

### User Story 2 - Shadow by default, applied only on explicit promotion (Priority: P2)

A newly installed ranking signal starts in shadow mode: its contributions are computed and logged locally, but served result orderings do not change at all. The user explicitly promotes the signal when satisfied; promotion is persisted per signal, survives restarts, and is reversible (demotion returns the signal to shadow). Promotion is independent of the trust tier that governs how the plugin executes: a blessed plugin still starts in shadow; a merely verified plugin can be promoted.

**Why this priority**: Chunk acceptance 2. Shadow-first is what makes installing a third-party signal safe enough to be routine.

**Independent Test**: Install the first-party signal; run searches; served orderings are unchanged while the shadow log accumulates entries. Promote; orderings may change. Demote; orderings return to baseline.

**Acceptance Scenarios**:

1. **Given** a freshly installed ranking signal, **When** searches run, **Then** served orderings are identical to the no-plugin baseline and shadow-log entries accumulate for those searches.
2. **Given** a shadow signal, **When** the user promotes it, **Then** the promotion is persisted, survives a server restart, and the signal's features begin to apply.
3. **Given** a promoted signal, **When** the user demotes it, **Then** ordering returns to the first-party baseline and the persisted state records shadow mode again.
4. **Given** a promoted signal whose plugin is removed or becomes unhealthy, **When** searches run, **Then** ordering degrades to the first-party baseline with no error surfaced to the search caller.

---

### User Story 3 - Shadow impact measurable as an MRR delta before promotion (Priority: P3)

Before promoting a signal, the user can see what it would do to retrieval quality: the offline eval harness reports the labeled-corpus MRR (and Hit@1/Hit@5) with and without the signal's features applied, as a before/after pair. The promotion decision is made against a number, extending the project's no-number-no-merge rule to ranking signals.

**Why this priority**: Chunk acceptance 3. Without the number, promotion is vibes, and the whole shadow apparatus is theater.

**Independent Test**: Run the eval harness in signal mode against the first-party visibility signal; it prints baseline MRR and with-signal MRR over the same labeled corpus.

**Acceptance Scenarios**:

1. **Given** the labeled eval corpus and an installed shadow signal, **When** the user runs the eval harness in signal mode, **Then** it reports before/after MRR, Hit@1, and Hit@5 over the same corpus in one run.
2. **Given** a signal whose features never fire on the corpus, **When** the eval runs, **Then** the before/after numbers are equal and reported as such (a zero delta is a valid, visible result).

---

### User Story 4 - A first-party signal ships on the public interface (Priority: P4)

A first-party ranking-visibility signal ships as a plugin on the public interface — the same artifact shape, admission checks, and conformance suite any third party would face. It declares zero capabilities and passes the TCK unmodified. It marks hits whose excerpt is an exported/public declaration, a fact the first-party weight table does not already encode.

**Why this priority**: The M5 gate requires every first-party plugin to ride the public SDK ("no first-party plugin relies on an interface a third party could not use"). Shipping the signal first-party is the forcing function for interface quality.

**Independent Test**: The TCK runs over the shipped plugin artifact unmodified and passes; its effective capability set resolves empty.

**Acceptance Scenarios**:

1. **Given** the shipped first-party signal artifact, **When** the TCK runs over it unmodified, **Then** all applicable chapters pass and the declared capability set is empty.
2. **Given** a hit whose excerpt declares an exported symbol, **When** the signal contributes, **Then** it emits its documented feature with a nonzero value; for a non-exported hit it emits zero or no feature.

---

### Edge Cases

- A plugin traps, exhausts fuel, or times out during shadow scoring: the search caller sees nothing; the failure is recorded against the plugin's health and repeated failure trips its breaker.
- The shadow log grows without bound: it is size-capped and oldest entries are discarded; capping never fails a search.
- A promoted signal's features are not yet computed for a hit (cold cache): the hit's plugin features contribute zero; ordering is the first-party baseline for that hit. No search ever waits on a plugin.
- Two installed signals emit the same feature name: features are namespaced per plugin, so contributions cannot collide or overwrite each other.
- Feature values are non-finite (NaN/Infinity) or absurdly large: rejected at the host boundary before they reach the scorer.
- The user promotes a signal that was never installed or has been removed: the promotion verb refuses with a clear error; persisted state is unchanged.
- Search runs while the plugin host is still booting: first-party baseline ordering, always.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A ranking plugin MUST be able to contribute only named numeric feature values for individual hits. No plugin path MAY set, veto, or reorder a final ordering, suppress or inject results, or bypass the first-party combiner.
- **FR-002**: The first-party combining model MUST remain the sole authority over ordering. Plugin features MUST enter scoring only through the first-party tunable weight table, and any change to that table MUST remain subject to the existing recorded-evaluation enforcement (no number, no merge).
- **FR-003**: A newly installed ranking signal MUST default to shadow mode: its contributions are computed and recorded but MUST NOT influence any served result ordering.
- **FR-004**: Shadow contributions MUST be recorded to a local, on-disk, size-bounded log that never leaves the machine. Recording MUST be fully asynchronous to the search response: no search response may wait on shadow scoring or logging.
- **FR-005**: Promotion MUST be an explicit per-signal user action, persisted across restarts, reversible (demotion), and orthogonal to the plugin trust tier (tier governs execution; promotion governs application of features).
- **FR-006**: Plugin execution MUST never sit on the search response path, shadow or promoted. Promoted features MUST apply through state prepared off the response path; where a feature value is unavailable, its contribution is zero and ordering degrades to the first-party baseline (Constitution I).
- **FR-007**: The impact of a shadow signal MUST be measurable before promotion as a before/after MRR (plus Hit@1/Hit@5) pair over the existing labeled eval corpus, in a single offline run.
- **FR-008**: A first-party ranking-visibility signal MUST ship on the public ranking interface with a declared capability set that resolves empty, and MUST pass the TCK unmodified.
- **FR-009**: Every plugin failure mode (trap, timeout, fuel exhaustion, dead sandbox lane, malformed output) MUST degrade to first-party-only ranking with no search outage and no error surfaced to the search caller; repeated failures MUST trip the plugin's existing health breaker.
- **FR-010**: The public interface vocabulary MUST be consumed as blessed: no change to the ranking interface or the interface baseline is in scope.
- **FR-011**: Feature names MUST be namespaced per contributing plugin so that two signals cannot collide on a name.

### Key Entities

- **Ranking signal**: an installed plugin exporting the public ranking interface; has an execution tier (existing) and a promotion state (new: shadow | promoted).
- **Feature contribution**: a per-hit list of (namespaced name, finite numeric value) pairs produced by a signal.
- **Shadow log entry**: a locally persisted record of one scored search — the query context, the served (first-party) ordering, and each signal's contributions — sufficient to replay ordering with features applied.
- **Promotion record**: the persisted per-signal promotion state plus provenance (when, by what action).
- **Feature cache entry**: an off-response-path store of computed contributions keyed by hit identity, consulted (never awaited) by the scorer for promoted signals.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With no ranking plugin installed, result ordering is unchanged: the existing golden ranking tests pass without modification.
- **SC-002**: With the first-party signal installed in shadow mode, served orderings are identical to baseline across the test corpus while shadow-log entries accumulate; after promotion, ordering changes are exactly reproduced by applying the logged features through the combiner; after demotion, baseline ordering returns.
- **SC-003**: The eval harness reports a before/after MRR / Hit@1 / Hit@5 pair for the first-party signal over the labeled corpus in one run, and the pair is recorded alongside the change that ships the signal's weight.
- **SC-004**: The TCK passes over the shipped first-party signal unmodified, and its effective capability set resolves empty.
- **SC-005**: No search response ever blocks on plugin execution: the response path contains no sandbox invocation in either mode, pinned by test.

## Assumptions

- The public interface `workbench:ranking@0.1.0` as blessed in the WIT baseline is sufficient and is consumed unchanged; no WIT vocabulary changes are in scope.
- The local on-disk shadow log is the M5 stand-in for §8.3's "opt-in telemetry"; wiring shadow data into the tracing/telemetry plane is M6 scope (m6-tracing), and registry admission requiring submitted eval numbers is M6 scope (m6-registry).
- The existing offline eval harness (labeled corpus + recorded baseline) is the measurement instrument for SC-003; bringing the full evaluation plane live remains m6-eval-harness scope.
- Promotion/demotion is exposed through the server's existing status/configuration surfaces; a polished management UI is out of scope.
- The blessing/promotion machinery for execution tiers (disk-backed health records, tier-gated lanes) is reused as-is; this feature adds promotion state beside it, not a new trust system.
