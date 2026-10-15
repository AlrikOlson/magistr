# Desktop studio delivery: twenty additional chunks

Delivered on October 15, 2026 as native scope `desktop-studio-design-20`. These are **twenty additional commits after `d3062284`**, following the [previous design-depth delivery](design-depth-delivery.md).

The current desktop pairs a quieter midnight/paper canvas with Manrope controls, Newsreader display type, illustrated project covers, an interactive spatial map and a sculptural activity shader. Reading, source review, writing, image inspection and measured activity each have a distinct composition.

| # | Delivered change | Commit | Preview |
| --- | --- | --- | --- |
| 1 | Bundled Manrope and Newsreader, midnight/paper colors and local font loading. | `eeaf7035` | [View](welcome-light.png) |
| 2 | Compact navigation frame and open conversation canvas. | `4ed31be9` | [View](workspace-dark.png) |
| 3 | Geometric project identities and compact conversation tracks. | `0ff83ba6` | [View](minimum-projects-dark.png) |
| 4 | Panoramic project entrance and editorial hierarchy. | `8cd4757e` | [View](welcome-dark.png) |
| 5 | Sculptural WebGL enamel arcs, physical gaps and pointer lighting. | `de9d1622` | [View](welcome-dark.png) |
| 6 | Actual activity counts, shape markers and proportional spectrum. | `ed1c05af` | [View](welcome-light.png) |
| 7 | Navigable spatial project map with bounded canonical catalog nodes. | `087a2fc0` | [View](project-map-dark.png) |
| 8 | Illustrated project cover boards beside live conversation lists. | `5b5b0da0` | [View](directory-light.png) |
| 9 | Compact conversation masthead and continuous location bar. | `771d9cc0` | [View](workspace-light.png) |
| 10 | Open reading pages, authored margins and display quotations. | `14cbcd86` | [View](editorial-light.png) |
| 11 | Source folios, precise gutter and a bounded source overview. | `07f2315e` | [View](source-folio-light.png) |
| 12 | Paired patch sheets with actual added/removed change bands. | `6ecf5571` | [View](patch-comparison-dark.png) |
| 13 | Numbered execution timeline and separated source/result sheets. | `4674b6b2` | [View](execution-light.png) |
| 14 | Compact composer desk, integrated controls and attachment tray. | `9275d2e6` | [View](attachments-dark.png) |
| 15 | Expanded manuscript canvas using the same native editor. | `38a2ed8d` | [View](writing-dark.png) |
| 16 | Large current-attempt instrument and paired work-launch choices. | `363cf523` | [View](work-paused-dark.png) |
| 17 | Agent inspector with a calibrated linear capacity meter. | `8aa9e815` | [View](inspector-light.png) |
| 18 | Ruled decision sheets and image stage with adaptive contact sheet. | `6974179d` | [View](image-stage-light.png) |
| 19 | Open saved library, project-grouped search and shared settings typography. | `e688d3be` | [View](history-dark.png) |
| 20 | Adaptive compact/wide compositions, matching theme previews and final verification. | This delivery commit | [View](workspace-compact-final-light.png) |

## Ownership and bounds

The list and project map share the existing catalog grouping and addressed selection path. The map presents at most eight projects and six conversations per project and links back to the complete list. It owns no session data or subscriptions.

The shader derives every segment from the accepted catalog. It has a 64-step ray-march limit and a 384-pixel drawing-buffer cap, no continuous animation loop, and at most one queued frame for pointer input. Reduced motion cancels pointer response; context loss preserves the CSS chart and numeric legend. The drawing-buffer resize checks both dimensions, including a square 300-pixel canvas.

Source overview marks derive only from line length and indent, remain outside selectable text, and stop at 120 marks. Source copying remains exact. Patch bands use actual changed lines. Work marks identify attempts, never inferred completed items. Context usage retains its semantic meter and explicit missing/zero values.

The bundled Latin fonts include their SIL licenses. No font or graphics request is made at runtime. The existing native textarea, focus, IME behavior, draft writer and application owners remain authoritative.

## Verification

- 52 desktop WebKit scenarios passed, covering selection and drafts, addressed navigation, project map bounds, exact source and patch copying, keyboard focus, image source lifetime, context loss and reduced motion.
- 14 focused JavaScript presentation tests passed.
- The Rust bundled-asset test passed with the project-map module and both font files included.
- Desktop Rust formatting and Clippy passed. The formatter corrected a missing final newline before its successful rerun.
- Structure and screenshot gates passed: interface inventory, semantic contrast in both appearances, icon provenance, source-bound captures and retained native evidence integrity.
- The final layout run records 99 captures. The [review](review.json) names 18 images inspected in the closing pass; earlier chunks also received focused rendering reviews. The native minimum is 760 × 550, with wide captures at 1920 × 1080.

Verification used background WebKit. Native OS-input journeys, VoiceOver, Windows rendering, packaging and release checks were not rerun. Existing native receipts remain bound to their original sources. Nothing was pushed or published, and the unrelated workflow-skill edit was preserved.