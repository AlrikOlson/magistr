# Terminal playback — iteration 01

Reviewed 2026-10-21 from the original PNGs in this folder. Capture details, source hashes, renderer and timestamps are in browser-checks.json. Task: follow the recorded Bevy implementation through an emulated terminal, then inspect syntax-highlighted source and actual changes.

| Capture | Inspection | Verdict |
| --- | --- | --- |
| terminal-plan-wide-light.png | Terminal rows and annotation read clearly; chronological scrollback is visible. The default xterm viewport leaves a black strip along the bottom in light mode. | Fail |
| terminal-diff-wide-light.png | Rust additions/removals align with the selected source action. The hunk header is nearly invisible; the viewport strip remains. | Fail |
| diff-reader-wide-light.png | Added/removed lines, source controls and hunk context are legible. Reader scrolls independently; no outer overflow. | Pass |
| source-reader-wide-light.png | Full-file syntax, indentation and copy state are readable. Header and controls remain distinct. | Pass |
| roadmap-wide-light.png | Three dependency columns, split and merge attachments are clear at 100%. The three small rectangles are the production renderer's checklist tally, verified in work_roadmap_surface.mjs, not missing glyphs. Keyboard focus was lost after opening a node and Escape failed. | Fail |

The browser run stopped at the Escape failure. This is not final evidence. Fix hunk contrast, viewport background and reader focus before recapturing. A unit test also caught an incorrect zero-line unified-diff hunk offset for new files.

No native OS-input or assistive-technology testing. The source/file bytes are unchanged.
