# Static terminal review — third capture

All four PNGs were opened at original resolution and inspected in full. User task: read the Codex-style terminal when JavaScript or the playback request is unavailable.

| File | Observation | Verdict |
| --- | --- | --- |
| terminal-static-wide-light.png | Call/result gutter and typography read correctly; header logo is broken in this capture. | Fail |
| terminal-static-wide-dark.png | Dark fallback and recovery notice read correctly; header logo is broken in this capture. | Fail |
| terminal-static-compact-light.png | Bullet continuations now align beneath the text, resolving iteration 02's defect. Header logo is broken in this capture. | Fail |
| terminal-static-compact-dark.png | Hanging bullets, contrast and recovery notice are readable at 390 px. Header logo is broken in this capture. | Fail |

The focused capture server omitted the SVG MIME type. This is a capture-harness defect: the unchanged production server and iteration 02 served the logo successfully. Preserve these images and recapture using the corrected harness; do not represent these frames as clean site evidence. Metadata, capture time, renderer, viewport and application source hashes are in browser-checks.json. No application source changed after this capture.
