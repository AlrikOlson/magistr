# Conversation iteration 01

Reviewed all original PNGs at full frame and readable detail on 2026-10-21. Renderer, viewport, appearance, capture times and source hashes are in browser-checks.json. This run stopped on the navigation stability assertion.

| Capture | Task and observations | Verdict |
| --- | --- | --- |
| conversation-start-wide-light.png | Read the opening human request. Correct prompt gutter, legible text, transport above content. Its next button still shifts when the elapsed label gains a digit, found by the full-session sweep. | Fail |
| conversation-followup-wide-light.png | Read the version follow-up in its original position. Previous test output remains in scrollback; human message is distinct. Same transport defect. | Fail |
| diff-reader-wide-light.png | Inspect real changes. Good color contrast and tab placement; keyboard focus scrolls the filename out of view. The header needs the file path. | Fail |
| source-reader-wide-light.png | Switch to the full file and copy it. Clear active tab, readable Rust and complete source. | Pass |
| roadmap-wide-light.png | Inspect the actual four game pieces and prerequisites. Nodes and dependency routes are legible, and connections meet the ports. | Pass |
| reasoning-wide-light.png | Inspect the opponent investigation. The question, finding and conclusion remain connected and readable. | Pass |
| check-reader-wide-light.png | Read the failed command and output. Clear heading and readable command/output; scrolling stays in the dialog. | Pass |

Overall: failed. Fix fixed-width time display and persistent filename before continuing. The remaining themes, compact views, motion and fallback captures were not reached. Headless WebKit does not verify native input or assistive technology.
