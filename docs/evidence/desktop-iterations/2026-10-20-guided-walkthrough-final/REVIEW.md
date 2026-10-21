# Guided walkthrough — navigation and content review

Task: understand the real inbox build, move between meaningful chapters, compare the review fix, and inspect exact source or checks when wanted.

All 30 PNGs were opened at original resolution and inspected. WebKit 27.2; desktop 1440 × 1000, compact 390 × 844, reflow 320 × 844 and tablet 820 × 1000; light/dark and reduced motion. Exact source hashes, dimensions and capture times are in browser-checks.json.

The navigation, source, review comparison, framing, fallback and responsive controls pass. The single movie capture fails because its fixed height makes the desktop recording too small. That bounded issue is corrected and recaptured in [the video iteration](../2026-10-20-guided-walkthrough-video/REVIEW.md); all other views here remain the inspected evidence for their unchanged behavior.

| Capture | Observations | Verdict |
| --- | --- | --- |
| recording-overview-wide-light.png | Actual conversation is prominent; all six chapters and the start action remain readable. Compact playback controls now occupy separate rows. | Pass |
| recording-overview-compact-light.png | Actual conversation is prominent; all six chapters and the start action remain readable. Compact playback controls now occupy separate rows. | Pass |
| recording-video-wide-light.png | Actual video now decodes at four seconds, but the fixed-height player shrinks it into a narrow column. Replaced by the subsequent dedicated video iteration. | Fail |
| recording-keyboard-wide-light.png | Chapter selection retains a visible outline above the content; no page header covers it. | Pass |
| recording-chapter-4-wide-light.png | The original before/after capture shows the menu-size difference at readable scale; review state uses explicit labels and retained failures. | Pass |
| recording-chapter-0-wide-light.png | Brief and saved five-part plan remain legible, with actual planned/completed states. Navigation stays above the changing content. | Pass |
| recording-chapter-1-wide-light.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-chapter-2-wide-light.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-chapter-3-wide-light.png | The original first inbox capture is framed around the selected conversation and assignment. Full capture and scrolling remain available. | Pass |
| recording-review-before-wide-light.png | The original before/after capture shows the menu-size difference at readable scale; review state uses explicit labels and retained failures. | Pass |
| recording-action-14-wide-light.png | Original output/diff, status, timestamp and downloads are readable; disclosure heading is no longer obscured by the sticky header. | Pass |
| recording-overview-wide-dark.png | Actual conversation is prominent; all six chapters and the start action remain readable. Compact playback controls now occupy separate rows. | Pass |
| recording-overview-compact-dark.png | Actual conversation is prominent; all six chapters and the start action remain readable. Compact playback controls now occupy separate rows. | Pass |
| recording-chapter-0-compact-dark.png | Brief and saved five-part plan remain legible, with actual planned/completed states. Navigation stays above the changing content. | Pass |
| recording-chapter-1-compact-dark.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-chapter-2-compact-dark.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-chapter-3-compact-dark.png | The original first inbox capture is framed around the selected conversation and assignment. Full capture and scrolling remain available. | Pass |
| recording-chapter-4-compact-dark.png | The original before/after capture shows the menu-size difference at readable scale; review state uses explicit labels and retained failures. | Pass |
| recording-review-before-compact-dark.png | The original before/after capture shows the menu-size difference at readable scale; review state uses explicit labels and retained failures. | Pass |
| recording-action-35-compact-dark.png | Original output/diff, status, timestamp and downloads are readable; disclosure heading is no longer obscured by the sticky header. | Pass |
| recording-320-light.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-320-dark.png | Exact source excerpt wraps with visible original line numbers; check summary and result link remain readable. Long source text increases height without page overflow. | Pass |
| recording-820-light.png | Brief and saved five-part plan remain legible, with actual planned/completed states. Navigation stays above the changing content. | Pass |
| recording-820-dark.png | Brief and saved five-part plan remain legible, with actual planned/completed states. Navigation stays above the changing content. | Pass |
| home-intro-wide-light.png | Product heading remains single-color; the example introduction and chapter entry align with the page in this appearance. | Pass |
| home-intro-wide-dark.png | Product heading remains single-color; the example introduction and chapter entry align with the page in this appearance. | Pass |
| home-intro-compact-light.png | Product heading remains single-color; the example introduction and chapter entry align with the page in this appearance. | Pass |
| home-intro-compact-dark.png | Product heading remains single-color; the example introduction and chapter entry align with the page in this appearance. | Pass |
| recording-unavailable.png | The original image and download/video links remain usable, with no inert controls; full action transcript is available without JavaScript. | Pass |
| recording-without-javascript.png | The original image and download/video links remain usable, with no inert controls; full action transcript is available without JavaScript. | Pass |

Framing scrolls unmodified recorded screenshots to the relevant controls/conversation; the full image is always linked. This is not a live app and none of the pictured inbox controls are presented as live controls. No native OS-input or assistive-technology verification. No user study or performance-outcome claim.
