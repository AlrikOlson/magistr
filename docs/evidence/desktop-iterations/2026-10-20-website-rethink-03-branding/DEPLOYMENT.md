# Production deployment — Magistr website redesign

Status: **PASS**. The reviewed redesign is live at [magistr.dev](https://magistr.dev).

- Existing Cloudflare Pages project: `magistr`.
- Production deployment: `83a08a07-212a-4403-b8a0-f135132035e2` ([immutable URL](https://83a08a07.magistr.pages.dev)).
- Deployed source commit: `19ed15961bfdfa1ca412c91f65202cd259c1ee79` on `website/design-clarity-20261020`.
- Build: existing Astro/Tailwind stack and pinned Wrangler 4.151.2, deployed through the command documented in [the website README](../../../../apps/website/README.md).
- [PR #3](https://github.com/AlrikOlson/magistr/pull/3) remains open. Branch protection was preserved; no merge or package publication was performed.
- Byte comparison completed: 2026-10-20T17:07:02.372967+00:00.

The following evidence-only commit records the receipt and exact asset comparison. It changes no website source or build inputs; the deployed source remains the commit above.

## Production verification

All **42 served files** at the immutable deployment match the local reviewed build byte for byte. All **40 exact public page and asset URLs** checked at magistr.dev also match. Public URLs were extracted from the generated HTML and web manifest, including the actual versioned image URLs visitors receive. Two unreferenced artwork source/export files were verified at the immutable deployment only. See [deployment-assets.json](deployment-assets.json) for URLs, SHA-256 values and source identity.

A genuinely missing URL returns HTTP 404 and the reviewed 404 page. HTML revalidation, `X-Content-Type-Options: nosniff` and `X-Frame-Options: DENY` pass. Production WebKit checks pass in all four desktop/compact and light/dark profiles, including 320px reflow, tabs, menus, keyboard/focus behavior, theme persistence, copy feedback, no-JavaScript content and lazy-search loading, empty, failure and recovery behavior.

Native Magistr checks:

| Check | Result | Receipt |
| --- | --- | --- |
| Website build/regression checks, including asset identity | PASS | `e86a9f40-8a96-4210-905e-cef51f909e87` |
| Local WebKit checks after branding fix | PASS | `23d78d22-b61c-4068-96fd-48d1113587f9` |
| Required GUI visual review, iteration 03 | PASS | `bf58d8b1-e531-4db6-b6b0-122830193e83` |
| Public-link checker (14 tests) and diff check | PASS | `7bef9256-ecf7-42cb-8bbd-267b39bb0c2c` |
| Production build identity | PASS | `39ed750f-0e54-4a3f-a181-c795002d29dd` |
| Production website behavior | PASS | `80bec507-08aa-4f29-bbc2-415128c04835` |

The first production byte comparison exposed cached old branding at unversioned asset URLs. That failed result is retained in native state. The owning fix derives navigation, browser, manifest and social-preview URLs from each shipped file's content hash. The final checks above supersede the earlier failure. [Iteration 03](REVIEW.md) reviews the resulting navigation in all four routes and both appearances.

## Design evidence

[Research and cited rationale](../2026-10-20-website-rethink-research/RATIONALE.md) establish the workflow-layer positioning. [Iteration 01](../2026-10-20-website-rethink-01/REVIEW.md) retains observed defects. [Iteration 02](../2026-10-20-website-rethink-02/REVIEW.md) records their correction. All 284 browser captures across the three iterations and seven generated branding assets were individually opened and reviewed. Real application and agent captures retain their original provenance in `public/screenshots/captures.json`.

Representative comparison: [before desktop](../2026-10-20-website-rethink-research/magistr-before-desktop-intro.png), [after desktop](home-intro-wide-light.png), [after compact dark](home-intro-compact-dark.png).

## Limits

Verification used headless WebKit. No foreground/native-input or assistive-technology testing is claimed. The product remains a source preview requiring repository access; registry publication is pending and the initial CLI target is Apple Silicon macOS. The existing five CI checks passed at the starting commit. At this deployment receipt, the source-commit CI run has passed its anonymous-public-endpoint and dependency/security checks; Rust/website, macOS desktop and packaging jobs are still running. The PR records subsequent CI status.
