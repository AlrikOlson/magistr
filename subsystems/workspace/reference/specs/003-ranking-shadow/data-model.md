# Data Model: Ranking-Signal Plugins in Shadow Mode

Entities from the spec, resolved to concrete shapes. Wire vocabulary comes from
`wit/ranking.wit` (blessed, unchanged); storage conventions from the existing
`.workbench/` state files.

## RankingSignal (runtime registration)

One installed plugin exporting `workbench:ranking@0.1.0`, discovered by the boot lane
(`ranking-` package-name convention, mirroring `lang-pack-`).

| Field | Type | Notes |
|---|---|---|
| name | string | plugin inventory name (namespace root for features) |
| componentHash | string (sha256) | identity for health + promotion records |
| tier | "blessed" \| "verified" \| "sideload" | existing inventory field; execution trust only |
| promoted | boolean | derived from PromotionRecord at boot and on change; default false |
| healthy | boolean | existing disk-backed breaker verdict |

**Invariants**: `promoted` never implies anything about `tier` and vice versa (R5). An
unhealthy or absent signal behaves as if it contributes nothing, promoted or not (FR-009).

## FeatureContribution

Per-hit output of one signal, host-namespaced before it reaches any scorer or log.

| Field | Type | Notes |
|---|---|---|
| name | string | `<plugin-name>/<feature-name>`; host prepends the namespace (FR-011) |
| value | f64 | must be finite; non-finite or \|v\| > 1e6 rejected at the host boundary |

**Validation**: rejection is per-contribution, silent to the search caller, counted toward
the plugin's health only when an entire batch is malformed.

## HitKey (feature-cache identity)

`contribute`'s input carries no query text (R1), so contributions cache by hit identity.

| Field | Type | Notes |
|---|---|---|
| file | string | workspace-relative path |
| line, column | u32 | 1-based, as `hit-context` |
| excerpt | string | the raw excerpt, NUL-joined into the key; a changed line is a new key (content-change invalidation without hashing on the hot path) |

## FeatureCacheEntry (in-memory, per process)

| Field | Type | Notes |
|---|---|---|
| key | HitKey | |
| features | FeatureContribution[] | per signal, namespaced |
| at | timestamp | LRU eviction; capacity-bounded (default 50k entries) |

**Invariants**: read path is a synchronous map lookup only (SC-005). Populated exclusively
by the shadow scorer (post-response) — both for shadow and promoted signals, since the
shadow log wants contributions either way.

## ShadowLogEntry (`.workbench/ranking-shadow.jsonl`)

One appended line per shadow-scored search.

| Field | Type | Notes |
|---|---|---|
| at | ISO timestamp | |
| queryHash | string | sha256 prefix — raw query text is NOT stored |
| servedOrder | string[] | file list in served (first-party) order |
| contributions | per-signal map: HitKey → FeatureContribution[] | what each signal said |
| applied | string[] | names of signals whose features were actually applied (promoted ∩ cached) |

**Invariants**: file capped (default 5 MB), oldest-entry eviction on append; append failure
is swallowed (FR-004). Local only.

## PromotionRecord (`.workbench/ranking-promotions.json`)

| Field | Type | Notes |
|---|---|---|
| name | string | plugin name |
| componentHash | string | promotion binds to the exact artifact (R5) |
| promoted | boolean | |
| at | ISO timestamp | provenance of the last transition |

**State transitions**: absent → shadow (install default, FR-003); shadow → promoted
(explicit user action, FR-005); promoted → shadow (explicit demotion, or automatically when
componentHash changes — a new artifact re-earns promotion). No other transitions exist.

## RANKING_WEIGHTS.plugin (in `src/tools/ranking.ts`)

| Field | Type | Notes |
|---|---|---|
| `<namespaced feature name>` | number | scalar weight per feature; part of the ONE table |

**Invariants**: the only path by which any plugin value reaches a score (FR-002); covered
by the eval weights-hash gate (Constitution VI). An unlisted feature name has weight zero
even when promoted and cached — weights are opt-in per feature.
