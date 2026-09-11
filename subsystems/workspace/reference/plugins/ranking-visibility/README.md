# ranking-visibility

The first-party visibility ranking signal (chunk:m5-ranking-shadow, feature
003-ranking-shadow): the first ranking-signal plugin on the public
`workbench:ranking@0.1.0` interface — the same interface, admission checks,
and TCK a third-party signal would face.

## The one feature

`exported` (host-visible as `ranking-visibility/exported`):

- **1.0** — a declaration hit whose excerpt carries an export/public
  visibility marker (`export`, `pub`, `public`) among its first four word
  tokens (modifiers stack: `export default async function`, `public static
  final class`).
- **0.0** — any other declaration hit.
- **nothing** — non-declaration hits (calls, imports, comments, strings),
  even when the excerpt mentions a marker.

Exported-ness is a relevance fact the first-party weight table (kind class,
file class, path) does not already encode, and it is a pure function of the
`hit-context` — this plugin declares ZERO capabilities.

## Lifecycle

Freshly installed, the signal runs in **shadow mode**: contributions are
logged to `.workbench/ranking-shadow.jsonl` and never applied. The user
promotes it via `.workbench/ranking-promotions.json` (promotion binds to the
component hash — new bytes re-earn promotion in shadow). Its feature affects
scores only through `RANKING_WEIGHTS.plugin["ranking-visibility/exported"]`
in `src/tools/ranking.ts`, so any weight change rides the eval harness's
no-number-no-merge gate. Measure before promoting:

```sh
bun run eval:rank --signal plugins/ranking-visibility
```

## Rebuilding

The committed `guest.wasm` is the shipped artifact. After changing
`guest/src` or `guest/wit`, run `build.ps1` (or `build.sh`) and commit the
new bytes. The `guest/wit/deps/ranking/ranking.wit` copy must stay byte-equal
to the top-level `wit/ranking.wit`.
