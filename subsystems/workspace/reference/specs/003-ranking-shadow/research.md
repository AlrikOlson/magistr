# Phase 0 Research: Ranking-Signal Plugins in Shadow Mode

Decisions pinned in think:128; grounded against the real interface, pipeline, eval harness,
and plugin-lane precedents. No unresolved NEEDS CLARIFICATION items remain.

## R1 — WIT surface: consume `workbench:ranking@0.1.0` unchanged

**Decision**: The blessed interface is sufficient as-is: `contribute: func(hit-context) ->
list<feature>`, with `kind-class`/`file-class` re-exporting the §6.4 vocabulary and
`feature` being `(name, f64)`. No WIT change; `wit/baseline.json` stays byte-equal.

**Rationale**: The interface already encodes the leash exactly — a plugin sees per-hit
facts and returns named numbers; there is no way to express an ordering through it. Same
posture as features 001/002 ("consumed unchanged" is itself a requirement, FR-010).

**Alternatives considered**: adding a batched `contribute-many` to the WIT (rejected: the
HOST batches over the canonical ABI by calling `contribute` per hit inside one instance
invocation — the wire cost lives in our own typed lane, not the contract; changing a
blessed stamp for an optimization would be the first baseline break for no functional
gain); adding query text to `hit-context` (rejected: not needed by the visibility signal,
widens the privacy surface of the shadow log, and is additive-later if a signal earns it).

## R2 — Application seam: the sandbox is never on the search response path

**Decision**: Two lanes, both off the response path. **Shadow lane**: after the response is
enveloped, a fire-and-forget batched `contribute` over the served candidates' hit-contexts;
results go to the shadow log and the feature cache. **Promoted lane**: `rankGroups`'s
caller consults the in-memory feature cache synchronously (plain map lookup); a miss
contributes zero. Promoted features therefore apply from warm state prepared by prior
shadow-lane runs, and no search ever waits on a plugin.

**Rationale**: Constitution I, and the recorded precedent: chunk
`backlog-classify-index-wiring` was deferred exactly because "a per-request sandbox call in
the search hot loop violates Constitution I". `contribute`'s inputs (file, line, column,
excerpt, kind-class, file-class) contain no query text, so contributions are cacheable by
hit identity — the cache is sound, not an approximation.

**Alternatives considered**: awaited batched call with a hard time budget (rejected: puts
the sandbox on the response path, contradicting the recorded Constitution-I precedent);
index-time computation (rejected: hit excerpts/kind-classes are produced at query time;
index-time would need a parallel extraction pass for marginal cache-warmth gain).

**Honest cost, recorded**: a promoted signal's features only affect a hit the *second* time
that hit is served (cold cache ⇒ zero contribution). Deterministic given cache state;
pinned as documented behavior in the shadow/promotion suite.

## R3 — Feature weighting: new keys in `RANKING_WEIGHTS`, combiner untouched

**Decision**: `RANKING_WEIGHTS` gains a `plugin` entry mapping namespaced feature names
(`<plugin-name>/<feature>`) to scalar weights (e.g. `"ranking-visibility/exported": 0.5`).
`orderGroups`' score becomes `bestHitScore + pathRelevance + Σ weight·featureValue` for
cached, promoted, weighted features. `ranking.ts` stays a pure module: the caller resolves
the cache into a plain `pluginFeatures` lookup passed as an argument.

**Rationale**: keeps "every tunable number lives in one table" literally true, which makes
Constitution VI self-enforcing — the eval baseline hashes `RANKING_WEIGHTS`, so adding or
changing a plugin weight without re-recording MRR fails CI today, with zero new mechanism.

**Alternatives considered**: per-signal weight in the promotion record (rejected: numbers
outside the one table dodge the weights-hash gate — precisely the failure §7.4 forbids);
plugin-supplied weights (rejected outright: that is the plugin setting scores).

## R4 — Shadow log: local, size-capped JSONL in `.workbench/`

**Decision**: `.workbench/ranking-shadow.jsonl`, one entry per shadow-scored search
(query-hash, served order, per-signal contributions per hit), capped (default 5 MB) with
oldest-entry eviction on append; never leaves the machine; append failures are swallowed
(logging must never fail a search).

**Rationale**: §8.3 says shadow signals are "logged to opt-in telemetry"; the telemetry
plane is m6-tracing's scope. A local on-disk log is the M5 stand-in that already satisfies
the chunk's acceptance ("logged, not applied") and gives promotion-decision replay data.
`.workbench/` is the established home for server-local state (`plugins.json`,
`plugin-health.json`).

**Alternatives considered**: waiting for m6-tracing (rejected: sequencing inversion — the
M5 gate depends on this chunk); SQLite (rejected: JSONL suffices for an append-only capped
log and adds no dependency).

## R5 — Promotion: persisted per-signal record, orthogonal to the blessing tier

**Decision**: `.workbench/ranking-promotions.json`, keyed like `plugin-health.json` by
plugin name + componentHash: `{promoted: bool, at, componentHash}`. Fresh install ⇒ absent
⇒ shadow. Promotion/demotion are explicit server-surface actions (status/config surface;
management UI out of scope). A changed componentHash invalidates the promotion (a new
artifact is a new signal and re-earns promotion in shadow).

**Rationale**: the tier answers "how much do we trust this code to *execute*" (lane
selection, subprocess capability); promotion answers "has this signal *earned* influence on
ranking". Conflating them would auto-apply any blessed signal — exactly what §8.3 forbids
("a newly installed signal runs in shadow mode ... until the user promotes it", regardless
of tier). componentHash keying reuses the health-record identity convention.

**Alternatives considered**: promotion as a field in `plugins.json` (rejected: that file is
the user-managed inventory read fresh each call; promotion is server-owned state with
provenance, belonging beside health records); tier-implies-promotion (rejected above).

## R6 — MRR delta: a signal mode on the existing offline eval harness

**Decision**: `scripts/eval-ranking.ts` gains a signal mode (`bun run eval:rank --signal
<plugin-dir>`): load the component through the same loader/sandbox lane the server uses,
compute contributions for every corpus candidate hit, then rank the labeled corpus twice —
first-party-only vs. with-features-applied — and print MRR/Hit@1/Hit@5 before/after in one
run. The default no-flag run is byte-identical to today (baseline gate untouched).

**Rationale**: chunk acceptance 3 says *measurable before promotion*; the labeled corpus +
recorded baseline already exist (`eval/ranking-corpus.jsonl`, `eval/ranking-baseline.json`)
and are the project's only source of relevance judgments until m6-eval-harness brings the
full evaluation plane live. Offline replay also avoids ever needing telemetry for the
promotion decision.

**Alternatives considered**: computing MRR deltas from the shadow log (rejected as the
*primary* instrument: the log has no relevance judgments, so it can show ordering *churn*
but not *quality*; kept as replay data only); blocking on m6-eval-harness (rejected:
sequencing inversion).

## R7 — First-party signal: `plugins/ranking-visibility`, zero capabilities

**Decision**: a no_std Rust carrier-recipe guest (think:119 recipe, as runner-go and the
lang-pack carrier) exporting `ranking`. One documented feature,
`ranking-visibility/exported`: 1.0 when the hit excerpt is a declaration carrying an
export/public marker (`export`, `pub`, `public`, `__all__`-style visibility tokens,
hand-rolled — no regex crate in no_std), else 0.0 (import/comment/string hits contribute
nothing). Zero declared capabilities; TCK must pass over the artifact unmodified.

**Rationale**: exported-ness is a real relevance signal the first-party table does not
encode (kind/file-class/path only), it is a pure function of `hit-context`, and it is
simple enough that the plumbing — not the signal — stays the chunk's substance. Zero-cap
keeps least-privilege trivially honest (runner-adapter precedent) and makes the TCK's
capability chapter meaningful on a third real pack.

**Alternatives considered**: recency/git-authorship signal (rejected: needs filesystem or
subprocess capability — wrong first example for the tightest-leash surface); query-token
density (rejected: `hit-context` has no query, by R1's design).
