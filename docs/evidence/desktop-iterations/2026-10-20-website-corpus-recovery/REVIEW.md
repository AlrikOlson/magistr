# Website search and 404 recovery review

User task: finish the corpus-driven website revision, including secondary routes and failure states.

The earlier [revision review](../2026-10-20-website-corpus-revision/REVIEW.md) found an unnecessary 404 preamble and decorative internal-link arrows. This iteration removes them and checks the revised shared typography in documentation search.

[Capture metadata](browser-checks.json) contains source hashes, timestamps, image hashes and viewports for all 20 frames. Captures use Playwright WebKit 27.2, darwin-arm64, reduced motion and scale 1, at 1440×1000 and 390×844 in both appearances. Every listed image was opened from this folder and inspected at readable detail on 2026-10-20 around 23:58 UTC. Original pending markers are retained; this document records the separate inspection.

| Capture | User task / state | Observations | Verdict |
| --- | --- | --- | --- |
| [installation-search-wide-light.png](installation-search-wide-light.png) | Documentation search for repair | The query and matching guide result are readable, focus is visible, and the result does not collide with guide navigation. | Pass |
| [installation-search-empty-wide-light.png](installation-search-empty-wide-light.png) | No results preserve a clear way to search again | The empty-result message is readable beneath the focused field. Guide navigation stays usable. | Pass |
| [installation-search-unavailable-wide-light.png](installation-search-unavailable-wide-light.png) | Search failure retains navigation and explains recovery | The failed-search message fits below the focused field and points to still-visible guide navigation; no overlap or clipping. | Pass |
| [concepts-search-idle-wide-light.png](concepts-search-idle-wide-light.png) | Search is ready without starting its index | The idle search field remains visible in the desktop sidebar while the guide is scrolled to its end. The main content and footer remain readable. | Pass |
| [not-found-wide-light.png](not-found-wide-light.png) | not-found full page | The error is stated directly. Both recovery actions are readable and fit without arrows or a duplicate preamble. Footer links wrap cleanly. | Pass |
| [installation-search-wide-dark.png](installation-search-wide-dark.png) | Documentation search for repair | The query and matching guide result are readable, focus is visible, and the result does not collide with guide navigation. | Pass |
| [installation-search-empty-wide-dark.png](installation-search-empty-wide-dark.png) | No results preserve a clear way to search again | The empty-result message is readable beneath the focused field. Guide navigation stays usable. | Pass |
| [installation-search-unavailable-wide-dark.png](installation-search-unavailable-wide-dark.png) | Search failure retains navigation and explains recovery | The failed-search message fits below the focused field and points to still-visible guide navigation; no overlap or clipping. | Pass |
| [concepts-search-idle-wide-dark.png](concepts-search-idle-wide-dark.png) | Search is ready without starting its index | The idle search field remains visible in the desktop sidebar while the guide is scrolled to its end. The main content and footer remain readable. | Pass |
| [not-found-wide-dark.png](not-found-wide-dark.png) | not-found full page | The error is stated directly. Both recovery actions are readable and fit without arrows or a duplicate preamble. Footer links wrap cleanly. | Pass |
| [installation-search-compact-light.png](installation-search-compact-light.png) | Documentation search for repair | The query and matching guide result are readable, focus is visible, and the result does not collide with guide navigation. | Pass |
| [installation-search-empty-compact-light.png](installation-search-empty-compact-light.png) | No results preserve a clear way to search again | The empty-result message is readable beneath the focused field. Guide navigation stays usable. | Pass |
| [installation-search-unavailable-compact-light.png](installation-search-unavailable-compact-light.png) | Search failure retains navigation and explains recovery | The failed-search message fits below the focused field and points to still-visible guide navigation; no overlap or clipping. | Pass |
| [concepts-search-idle-compact-light.png](concepts-search-idle-compact-light.png) | Search is ready without starting its index | This frame shows the end of the guide because the compact sidebar scrolls with the document. Storage text and footer are readable. The idle search field is visible in the companion concepts-intro capture from the revision set. | Pass |
| [not-found-compact-light.png](not-found-compact-light.png) | not-found full page | The error is stated directly. Both recovery actions are readable and fit without arrows or a duplicate preamble. Footer links wrap cleanly. | Pass |
| [installation-search-compact-dark.png](installation-search-compact-dark.png) | Documentation search for repair | The query and matching guide result are readable, focus is visible, and the result does not collide with guide navigation. | Pass |
| [installation-search-empty-compact-dark.png](installation-search-empty-compact-dark.png) | No results preserve a clear way to search again | The empty-result message is readable beneath the focused field. Guide navigation stays usable. | Pass |
| [installation-search-unavailable-compact-dark.png](installation-search-unavailable-compact-dark.png) | Search failure retains navigation and explains recovery | The failed-search message fits below the focused field and points to still-visible guide navigation; no overlap or clipping. | Pass |
| [concepts-search-idle-compact-dark.png](concepts-search-idle-compact-dark.png) | Search is ready without starting its index | This frame shows the end of the guide because the compact sidebar scrolls with the document. Storage text and footer are readable. The idle search field is visible in the companion concepts-intro capture from the revision set. | Pass |
| [not-found-compact-dark.png](not-found-compact-dark.png) | not-found full page | The error is stated directly. Both recovery actions are readable and fit without arrows or a duplicate preamble. Footer links wrap cleanly. | Pass |

All four earlier 404 failures are resolved. The build/content check passed (`e50887f2-8d99-48ab-8d8b-d49ec9318086`), and the complete browser interaction checks passed while capturing these states (`36c4a31d-f46f-4a06-ae0a-0019230f2ab5`).

Limits: local headless WebKit; no deployment, OS-input or assistive-technology claim. The compact idle-search screenshot is accurately described above rather than treated as visible evidence of a field outside its frame.
