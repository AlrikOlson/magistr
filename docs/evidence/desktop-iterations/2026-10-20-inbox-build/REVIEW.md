# Support inbox visual review

Task: build and record a real local support inbox with mail intake, search, assignment and conflict handling.

Opened every PNG below at full frame and readable detail. Renderer: headless WebKit; viewports 1280 × 900 and 390 × 900; light and dark; reduced motion. Exact timestamps, source hashes, image hashes and video provenance are in [captures.json](captures.json). This is browser verification, not a native OS or assistive-technology run.

Verdict: **Fail**. Native select controls render about 22px high, below the intended usable target. The conflict notice asks for a reload that has already happened. All images were reviewed; correction and fresh captures are required. The next iteration is 2026-10-20-inbox-build-refined.

| Capture | User task and observations | Verdict |
| --- | --- | --- |
| inbox-empty-wide-light.png | Start an inbox. Empty-state explanation, search labels and sample-mail action are readable; list and detail areas have distinct boundaries. Select controls are too short. | Fail |
| inbox-assigned-wide-light.png | Find Friday's request and assign Maya. Selected row, assignee, open status and full message agree; controls sit together above the message. Select controls are too short. | Fail |
| inbox-finished-wide-light.png | Read a reopened thread after a teammate update. Three conversation rows, Leo assignment and two ordered messages are legible; search focus is visible and content wraps cleanly. Select controls are too short. | Fail |
| inbox-empty-wide-dark.png | Start an inbox. Empty-state explanation, search labels and sample-mail action are readable; list and detail areas have distinct boundaries. Select controls are too short. | Fail |
| inbox-assigned-wide-dark.png | Find Friday's request and assign Maya. Selected row, assignee, open status and full message agree; controls sit together above the message. Select controls are too short. | Fail |
| inbox-finished-wide-dark.png | Read a reopened thread after a teammate update. Three conversation rows, Leo assignment and two ordered messages are legible; search focus is visible and content wraps cleanly. Select controls are too short. | Fail |
| inbox-empty-compact-light.png | Start an inbox. Empty-state explanation, search labels and sample-mail action are readable; list and detail areas have distinct boundaries. Select controls are too short. | Fail |
| inbox-assigned-compact-light.png | Find Friday's request and assign Maya. Selected row, assignee, open status and full message agree; controls sit together above the message. Select controls are too short. | Fail |
| inbox-finished-compact-light.png | Read a reopened thread after a teammate update. Three conversation rows, Leo assignment and two ordered messages are legible; search focus is visible and content wraps cleanly. Select controls are too short. | Fail |
| inbox-empty-compact-dark.png | Start an inbox. Empty-state explanation, search labels and sample-mail action are readable; list and detail areas have distinct boundaries. Select controls are too short. | Fail |
| inbox-assigned-compact-dark.png | Find Friday's request and assign Maya. Selected row, assignee, open status and full message agree; controls sit together above the message. Select controls are too short. | Fail |
| inbox-finished-compact-dark.png | Read a reopened thread after a teammate update. Three conversation rows, Leo assignment and two ordered messages are legible; search focus is visible and content wraps cleanly. Select controls are too short. | Fail |
| inbox-conflict-wide-light.png | Leo's saved update is retained, but the notice incorrectly asks for another reload. Select controls are too short. | Fail |
| inbox-search-empty-wide-light.png | Search with no matches keeps the search field focused and explains how to recover. Select controls are too short. | Fail |

The WebM is the unmodified headless browser recording of the actual local server, with fictional customer data and no audio. The checks use real HTTP requests and persisted SQLite state, not mocked responses. Browser test output records the video hash. The final clip is 1280 × 900, VP9, 11.92 seconds; native playback and assistive-technology coverage are not claimed.
