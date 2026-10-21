# Documentation search copy review

User task: make all website copy and documentation match everyday use in plain language.

All 16 frames were opened from this folder and inspected at readable size. [browser-checks.json](browser-checks.json) records exact capture times, source hashes, viewport sizes, appearances, renderer and image hashes. Captured 2026-10-20 at 23:40 UTC with Playwright WebKit 27.2, 1440×1000 and 390×844, light and dark, scale 1, reduced motion. The report's pending markers are unchanged; this is the separate visual review.

The revised messages are “No results. Try a different word.” and “Search couldn’t load. Try reloading, or use the guide links below.” Inspection covered wrapping, contrast, input focus, clear control, result placement and guide access.

| Capture | Task/state | Concrete observation | Verdict |
| --- | --- | --- | --- |
| [installation-search-wide-light.png](installation-search-wide-light.png) | Documentation search for repair | The repair query shows the Installation result immediately below the focused field. Result, selected guide and page content remain visually distinct. | Pass |
| [installation-search-empty-wide-light.png](installation-search-empty-wide-light.png) | No results preserve a clear way to search again | The empty-result sentence fits below the focused field; the clear control and guide links remain visible. | Pass |
| [installation-search-unavailable-wide-light.png](installation-search-unavailable-wide-light.png) | Search failure retains navigation and explains recovery | The recovery message wraps below the focused search field, remains legible, and leaves the guide links available without overlap. | Pass |
| [concepts-search-idle-wide-light.png](concepts-search-idle-wide-light.png) | Search is ready without starting its index | The idle search field is visible in the sidebar beside the guide; the guide text and footer remain aligned. | Pass |
| [installation-search-wide-dark.png](installation-search-wide-dark.png) | Documentation search for repair | The repair query shows the Installation result immediately below the focused field. Result, selected guide and page content remain visually distinct. | Pass |
| [installation-search-empty-wide-dark.png](installation-search-empty-wide-dark.png) | No results preserve a clear way to search again | The empty-result sentence fits below the focused field; the clear control and guide links remain visible. | Pass |
| [installation-search-unavailable-wide-dark.png](installation-search-unavailable-wide-dark.png) | Search failure retains navigation and explains recovery | The recovery message wraps below the focused search field, remains legible, and leaves the guide links available without overlap. | Pass |
| [concepts-search-idle-wide-dark.png](concepts-search-idle-wide-dark.png) | Search is ready without starting its index | The idle search field is visible in the sidebar beside the guide; the guide text and footer remain aligned. | Pass |
| [installation-search-compact-light.png](installation-search-compact-light.png) | Documentation search for repair | The repair query shows the Installation result immediately below the focused field. Result, selected guide and page content remain visually distinct. | Pass |
| [installation-search-empty-compact-light.png](installation-search-empty-compact-light.png) | No results preserve a clear way to search again | The empty-result sentence fits below the focused field; the clear control and guide links remain visible. | Pass |
| [installation-search-unavailable-compact-light.png](installation-search-unavailable-compact-light.png) | Search failure retains navigation and explains recovery | The recovery message wraps below the focused search field, remains legible, and leaves the guide links available without overlap. | Pass |
| [concepts-search-idle-compact-light.png](concepts-search-idle-compact-light.png) | Search is ready without starting its index | This frame is scrolled to the bottom of the guide after navigation: storage text and footer links fit. It does not show the idle search input; that is visible in the concepts intro frames in the companion copy set. | Pass |
| [installation-search-compact-dark.png](installation-search-compact-dark.png) | Documentation search for repair | The repair query shows the Installation result immediately below the focused field. Result, selected guide and page content remain visually distinct. | Pass |
| [installation-search-empty-compact-dark.png](installation-search-empty-compact-dark.png) | No results preserve a clear way to search again | The empty-result sentence fits below the focused field; the clear control and guide links remain visible. | Pass |
| [installation-search-unavailable-compact-dark.png](installation-search-unavailable-compact-dark.png) | Search failure retains navigation and explains recovery | The recovery message wraps below the focused search field, remains legible, and leaves the guide links available without overlap. | Pass |
| [concepts-search-idle-compact-dark.png](concepts-search-idle-compact-dark.png) | Search is ready without starting its index | This frame is scrolled to the bottom of the guide after navigation: storage text and footer links fit. It does not show the idle search input; that is visible in the concepts intro frames in the companion copy set. | Pass |

Overall verdict: Pass. The compact idle-named captures show the page bottom, not the search input; use [the companion copy review](../2026-10-20-website-everyday-use/REVIEW.md) for idle input appearance. No blocking overlap, clipping or unreadable message was found.

Executed interaction receipt: `e36913fe-0d4d-407d-b085-5ef17d000a15`, passed. The later web-manifest description correction changes metadata only; no captured search content or styling changed. These are local headless website checks, not a deployment receipt or native assistive-technology check.
