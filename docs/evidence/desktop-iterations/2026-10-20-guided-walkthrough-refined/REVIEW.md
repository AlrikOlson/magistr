# Guided walkthrough — second iteration

Opened all 30 saved PNGs at original resolution. The task is to follow the actual inbox implementation and inspect its evidence. Per-capture dimensions, renderer, appearance, source hashes and timestamps are in browser-checks.json. Desktop 1440 × 1000, compact 390 × 844, reflow 320 × 844, tablet 820 × 1000; WebKit 27.2, reduced motion.

Overall: FAIL despite passing interaction assertions. Fix compact control geometry, place chapter navigation above changing content, frame actual captures around the relevant work, and verify a decoded video frame. The test server lacked byte-range support; the original video itself contains the correct recorded workflow. Do not replace or transcode the original video to work around the server.

| Capture | Observation | Verdict |
| --- | --- | --- |
| recording-overview-wide-light.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-chapter-0-wide-light.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-chapter-1-wide-light.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-chapter-2-wide-light.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-overview-compact-dark.png | Primary start button collapses into an overlapping sliver. The capture emphasizes the app header while the conversation sits below the visible frame. | Fail |
| recording-chapter-0-compact-dark.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-chapter-1-compact-dark.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-chapter-2-compact-dark.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-chapter-3-compact-dark.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-chapter-4-compact-dark.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-review-before-compact-dark.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-chapter-3-wide-light.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-chapter-4-wide-light.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-review-before-wide-light.png | Actual capture and review labels are truthful, but the compact frame hides the controls/conversation being explained; chapter navigation sits below the long stage. | Fail |
| recording-action-14-wide-light.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-keyboard-wide-light.png | Selected chapter keeps its outline, but the captured component includes an overlapping sticky page header; capture alignment needs correction. | Fail |
| recording-video-wide-light.png | Video screenshot is black. A successful seek property assertion did not establish a decoded frame. Original recording was independently decoded and inspected at 4 seconds. | Fail |
| recording-overview-wide-dark.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-overview-compact-light.png | Primary start button collapses into an overlapping sliver. The capture emphasizes the app header while the conversation sits below the visible frame. | Fail |
| recording-320-light.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-320-dark.png | Exact excerpt and line numbers are readable and wrap without page overflow; compact check summary could use its full width. | Pass |
| recording-action-35-compact-dark.png | Source diff wraps and downloads remain legible, but the sticky page header overlaps the disclosure heading in this capture. | Fail |
| recording-820-light.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| recording-820-dark.png | Readable typography, aligned controls and clear hierarchy in this captured state. | Pass |
| home-intro-wide-light.png | Hero and revised example introduction wrap cleanly; navigation and installation action remain clear. | Pass |
| home-intro-wide-dark.png | Hero and revised example introduction wrap cleanly; navigation and installation action remain clear. | Pass |
| home-intro-compact-light.png | Hero and revised example introduction wrap cleanly; navigation and installation action remain clear. | Pass |
| home-intro-compact-dark.png | Hero and revised example introduction wrap cleanly; navigation and installation action remain clear. | Pass |
| recording-unavailable.png | Readable original capture, download links and truthful fallback; no inert playback controls. | Pass |
| recording-without-javascript.png | Readable original capture, download links and truthful fallback; no inert playback controls. | Pass |

No native OS-input or assistive-technology testing was performed. These captures are superseded by the corrected iteration.
