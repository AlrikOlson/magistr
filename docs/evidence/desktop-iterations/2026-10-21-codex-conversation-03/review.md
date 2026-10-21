# Conversation iteration 03

All six PNGs were opened and inspected at full frame and readable detail on 2026-10-21. Viewports, appearances, source hashes, renderer and capture times are in browser-checks.json. Browser checks passed in 40.8 seconds, including ten immediate pause/resume cycles during initialization, all 98 stationary-control positions in four profiles, and horizontal source scrolling on phones.

| Capture | Task and observations | Verdict |
| --- | --- | --- |
| conversation-start-wide-light.png | Start the session. The prompt gutter, complete request, transport and annotation are readable without clipping. | Pass |
| stream-paused-wide-light.png | Pause an agent reply. Human request remains visible above the partial reply; controls, speaker label and text stay in place. | Pass |
| conversation-start-wide-dark.png | Start the session. The prompt gutter, complete request, transport and annotation are readable without clipping. | Pass |
| conversation-start-compact-light.png | Start the session. The prompt gutter, complete request, transport and annotation are readable without clipping. | Pass |
| conversation-start-compact-dark.png | Start the session. The prompt gutter, complete request, transport and annotation are readable without clipping. | Pass |
| stream-paused-compact-dark.png | Pause an agent reply. Human request remains visible above the partial reply; controls, speaker label and text stay in place. | Pass |

The acknowledgement race is corrected: only one write can be pending, including during frame setup. Static dialog layouts remain as reviewed in iteration 02. Code review identified one further interruption issue: adding the dialog’s scroll-lock class also triggered the theme observer, completing paused text. Iteration 04 isolates actual theme changes and checks that opening and closing a dialog preserves partial text.

Limitations: headless WebKit; native OS input and assistive technology not tested.
