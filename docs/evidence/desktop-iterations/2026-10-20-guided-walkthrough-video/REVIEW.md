# Guided walkthrough — video refinement

Task: watch the completed app, seek to a recorded moment and return to the guided walkthrough.

Opened all four captures at original resolution. WebKit 27.2, 1440 × 1000 desktop and 390 × 844 compact, light/dark, reduced motion. browser-checks.json records exact source hashes, capture times, image hashes and dimensions. This iteration changes only the video sizing and the full-size media link. The [navigation/content review](../2026-10-20-guided-walkthrough-final/REVIEW.md) covers the other unchanged views.

| Capture | Observations | Verdict |
| --- | --- | --- |
| recording-video-wide-light.png | Recording fills the available width; conversation and message are readable. Native controls remain usable. The full-size link now opens the video rather than the static capture. | Pass |
| recording-video-wide-dark.png | Dark surrounding interface keeps contrast; the original light-theme video is preserved. All chapter and return controls are clear. | Pass |
| recording-video-compact-light.png | Chapter/return controls and native transport controls fit without overlap. Whole desktop movie is a small overview on a phone; its detailed text is available through the full-size video and the readable compact chapter captures. | Pass for playback and overview; desktop recording is not a compact app recording |
| recording-video-compact-dark.png | Same usable compact controls and full-size link; original video appearance stays unchanged. Chapter navigation and source disclosure remain readable. | Pass with the same desktop-video limitation |

All four browser profiles also verify real seeking and decoded nonblack video pixels, all six chapters, source/check inspection, keyboard operation and the loading/no-JavaScript fallbacks. The original VP9 bytes, build recording and downloadable source are unchanged. Native fullscreen and assistive-technology behavior were not tested. The website does not claim a second mobile video recording.
