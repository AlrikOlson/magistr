# Codex terminal formatting — final fallback review

User task: read the same familiar, correctly indented terminal when playback is unavailable. All four frames were opened at original resolution and inspected in full, including text detail, spacing, contrast, list alignment, controls and surrounding page layout.

| Capture | User task | Observations | Verdict |
| --- | --- | --- | --- |
| terminal-static-wide-light.png | Read recorded output without JavaScript | Call/result gutters and bullet text align. The header logo loads. The retained explanation, download links and transcript are legible. | Pass |
| terminal-static-wide-dark.png | Recover from a failed recording request | The terminal output and recovery notice remain readable, with the logo loaded and no inactive playback controls. | Pass |
| terminal-static-compact-light.png | Read the fallback at 390 px | Wrapped bullets now continue beneath their text. All content fits, and the explanation and transcript follow in a clear order. | Pass |
| terminal-static-compact-dark.png | Recover on a narrow dark screen | Hanging indents, text contrast and recovery notice remain readable. The logo and download links display correctly; no horizontal overflow. | Pass |

Metadata, renderer version, source hashes, viewport, appearance and capture times are in [browser-checks.json](browser-checks.json). The reproducible focused capture is [capture.mjs](capture.mjs).

Overall visual verdict: pass. These frames replace the failed static fallback capture in [iteration 02](../2026-10-21-codex-terminal-format-02/review.md). Its 22 passing interactive/reflow captures remain the evidence for the unchanged terminal renderer. Iteration 03 exposed an SVG MIME omission in the focused capture server, corrected here; it did not require an application change. Both product defects found during review—control placement and fallback list indentation—are resolved.

The final scope is the website terminal's presentation. Build-time code highlighting emits structured rows; viewport-aware playback applies Codex gutters, word wrapping and real source line numbers. No dependency was added, recorded event changed, prompt invented, or native Magistr terminal modified.

Validation: website build and post-build checks; 13 recording/highlighting tests, including all 65 actions at four terminal widths in both themes; headless WebKit interactions, live resize and quick scrubbing; focused fallback checks. Limits: no native OS-input or assistive-technology testing; narrow terminals wrap long indivisible code tokens and filenames. Full records remain available in the details and transcript.

Post-capture source-only cleanup: restored the final newline in highlight-recording.mjs and terminal-playback.ts. No statement, generated markup, style or runtime behavior changed; the capture hashes identify the inspected versions before that whitespace cleanup.
