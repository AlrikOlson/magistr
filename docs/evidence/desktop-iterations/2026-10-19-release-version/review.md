# Release version display review

Task: display the canonical Magistr release version on the home page and installation page.
The version remains `0.1.0`; the change replaces independently maintained literals with generated release metadata.

Captured on 2026-10-19T22:45:51.529Z through 2026-10-19T22:45:57.231Z.
Renderer: Playwright WebKit 27.2, headless, reduced motion, device scale 1.
Wide viewport: 1440 × 1000. Compact viewport: 390 × 844. Both light and dark appearances.
[Capture metadata](captures.json) records each source hash, image hash, viewport and capture time.
All eight files below were opened from this directory at their original dimensions and visually inspected.

| Capture | Observations | Verdict |
| --- | --- | --- |
| [home-wide-light.png](home-wide-light.png) | Version line is readable above the heading; its green dot and blue text have clear spacing. Heading, screenshot, caption and action buttons remain aligned with no overlap. | Pass |
| [home-wide-dark.png](home-wide-dark.png) | Version and body text retain contrast against the dark surface. The image, caption and two buttons keep the same hierarchy and spacing. | Pass |
| [installation-wide-light.png](installation-wide-light.png) | The inline version reads v0.1.0 without added spaces or missing punctuation. Paragraph wrapping, selected navigation, callout and code block are clear and separated. | Pass |
| [installation-wide-dark.png](installation-wide-dark.png) | Inline version and surrounding paragraph remain legible; sidebar selection and links are distinct. Heading and callout do not collide. | Pass |
| [home-compact-light.png](home-compact-light.png) | Version fits on one line. Heading wraps cleanly; full-width actions, navigation controls and page margins remain usable. | Pass |
| [home-compact-dark.png](home-compact-dark.png) | Version remains readable above the wrapped heading; both action labels contrast clearly and controls do not overlap. | Pass |
| [installation-compact-light.png](installation-compact-light.png) | The version appears intact in the introductory paragraph. Sidebar becomes a vertical section, with clear spacing before the heading and readable text within the viewport. | Pass |
| [installation-compact-dark.png](installation-compact-dark.png) | Selected navigation, breadcrumbs, heading and version paragraph retain contrast. Paragraph wraps naturally without horizontal clipping. | Pass |

The changed version displays and their surrounding layout pass visual review. No connection diagrams or routing changed.
Captures cover the affected initial views; content below these viewports and native desktop/assistive-technology interaction were not part of this metadata change. The embedded desktop illustration continues below the compact home viewport as normal page content.
