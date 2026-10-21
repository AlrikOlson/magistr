# Earlier simulation: tablet review

**Withdrawn; not shipped.** These six frames were opened at original resolution and inspected after the previous 820px geometry failure. All six pass the focused layout review. The owner subsequently clarified that the example must replay real work; the simulated design was removed and replaced with [the recorded build](../2026-10-20-recorded-workflow-refined/REVIEW.md).

The bounded tablet layout increased row height to fit long node labels. [Capture metadata](tablet-checks.json) records actual node bounds, viewport, appearance, renderer/version, timestamps and image/source hashes. Metadata review fields remain pending; this document records the separate inspection.

| Capture | Viewport / appearance | User task | Observations | Verdict |
| --- | --- | --- | --- | --- |
| [simulation-tablet-720-light.png](simulation-tablet-720-light.png) | 720 × 1100, light | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |
| [simulation-tablet-720-dark.png](simulation-tablet-720-dark.png) | 720 × 1100, dark | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |
| [simulation-tablet-820-light.png](simulation-tablet-820-light.png) | 820 × 1100, light | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |
| [simulation-tablet-820-dark.png](simulation-tablet-820-dark.png) | 820 × 1100, dark | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |
| [simulation-tablet-1024-light.png](simulation-tablet-1024-light.png) | 1024 × 1100, light | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |
| [simulation-tablet-1024-dark.png](simulation-tablet-1024-dark.png) | 1024 × 1100, dark | Inspect the paid-launch dependency map | All six nodes fit the map; subscription card stays above the legend. Lines meet card edges, route outside labels and merge clearly at launch. Body text and controls remain readable. | Pass (visual only) |

The checks used headless WebKit 27.2. These are historical layout checks of an illustrative example, not proof that the simulated project was implemented. Native browser, physical touch and assistive-technology behavior were not covered.
