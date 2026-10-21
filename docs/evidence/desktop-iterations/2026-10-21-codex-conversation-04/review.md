# Conversation iteration 04

All six original PNGs were opened and inspected at full frame and readable detail on 2026-10-21. Metadata, viewport, appearance, capture time, source hashes and renderer are in browser-checks.json. WebKit checks passed in 38.75 seconds.

| Capture | Human task and concrete observations | Verdict |
| --- | --- | --- |
| conversation-start-wide-light.png | Read the opening request. Prompt gutter, typography, contrast, controls and annotation are clear; the content fits its viewport. | Pass |
| stream-paused-wide-light.png | Open and close details during a paused reply. The same partial text remains; focus returns visibly to Read message, and the transport stays in place. | Pass |
| conversation-start-wide-dark.png | Read the opening request. Prompt gutter, typography, contrast, controls and annotation are clear; the content fits its viewport. | Pass |
| conversation-start-compact-light.png | Read the opening request. Prompt gutter, typography, contrast, controls and annotation are clear; the content fits its viewport. | Pass |
| conversation-start-compact-dark.png | Read the opening request. Prompt gutter, typography, contrast, controls and annotation are clear; the content fits its viewport. | Pass |
| stream-paused-compact-dark.png | Open and close details during a paused reply. The same partial text remains; focus returns visibly to Read message, and the transport stays in place. | Pass |

Opening the inspector no longer counts as a theme change. Source, diff, roadmap, reasoning, check, complete-conversation and fallback layouts are covered by the individually inspected iteration 02 captures. The final endpoint pause/resume regression is covered in iteration 05.

Headless WebKit only. Native OS input and assistive technology were not tested.
