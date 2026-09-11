# The public WIT interfaces (SDD §8.2)

The five plugin surfaces plus the one host-capability package, each a
self-contained WIT package with its own independent version stamp:

| Package | Surface | SDD |
|---|---|---|
| `workbench:lang-pack` | grammar + tag queries + file-class heuristics | §8.3 Language packs |
| `workbench:resolver` | `resolve-references`, `rename`, `diagnostics` — the Tier-2 upgrade path | §8.3 Resolvers |
| `workbench:runner` | structured-failure parsers for `run` output | §8.3 Runner adapters, §9.6 |
| `workbench:tool` | new namespaced MCP tools | §8.3 Tools |
| `workbench:ranking` | feature contributions to the ranking scorer | §8.3 Ranking signals, §7.4 |
| `workbench:subprocess` | the narrow host capability reserved to blessed plugins — spec-id addressed, framed-stdio language-server sessions; 0.2.0 adds `reused`/`release` for the engine-child server pool (specs/007) | §8.3 Resolvers |

The vocabulary is a deliberate re-export of the internal port vocabulary
(SDD §6.4): `source-location` is `SourceLocation` (src/ports/types.ts),
`file-class` and `kind-class` are `FileClass`/`KindClass`
(src/tools/ranking.ts), the runner's `structured-failure` is §9.6's quartet
verbatim, and `workbench:tool`'s host imports are the same live-read /
queued-write lanes the `script` guest gets (§9.7). First-party and
third-party code speak the same language; the SDK cannot drift from reality
because first-party features are built on it (M5 gate).

## Design decisions

- **Primitives are duplicated per package, not shared.** A common types
  package would couple the five version stamps; independence of the stamps is
  the point. The compatibility gate freezes each copy anyway.
- **`workbench:tool` payloads are envelope-versioned JSON strings.** The JSON
  Schemas under `schemas/` are the one source of truth for request and
  envelope shape (§4.2); re-modeling them as WIT types would freeze a second
  copy of the highest-risk contract (§14). WIT carries the transport, the
  schemas carry the shape, and the host seals every result through the
  standard envelope (redaction included).
- **World import names are the future grant vocabulary.** The sandbox host
  grants imports by name, default-deny, before instantiation
  (crates/workbench-core/src/sandbox.rs); capability manifests
  (chunk m4-capability-manifests) will name exactly these imports.

## Evolution rules

A public WIT mistake is carried for years (SDD §14, highest severity) —
so evolution is mechanical, not judgment:

1. **Per-interface, independent semver.** Every package carries its own
   `@major.minor.patch` stamp. Bumping one never obliges bumping another.
2. **Within a major: additive only.** New interfaces, new functions, new
   types, and new world items may be added — alongside a version bump. What
   exists is frozen: no removal, no rename, no signature change.
3. **Record fields and enum/variant cases are frozen too.** The canonical ABI
   makes them part of the signature; a "harmless" added field is a breaking
   ABI change, so it counts as a signature change and needs a major.
4. **Any vocabulary change requires the stamp to move.** Same-version content
   drift is refused by the gate.
5. **A major bump is a deliberate act.** It requires re-blessing the
   baseline, never happens as a side effect of an edit.

## Enforcement

`crates/workbench-core/tests/wit_contracts.rs` parses every package with the
real component-model parser (`wit-parser`), renders the whole vocabulary to a
canonical form, and enforces the rules above against `baseline.json` — the
blessed snapshot. Removals, renames, signature changes, version regressions,
same-version drift, and un-blessed major bumps all go red.

`test/wit-contracts.test.ts` holds the other end of the contract: the 1:1
crosswalk between the WIT vocabulary and the live internal TypeScript
vocabulary it re-exports.

To bless a deliberate change:

```
WIT_BASELINE_BLESS=1 cargo test --test wit_contracts
```

and commit `baseline.json` with the change that motivated it.
