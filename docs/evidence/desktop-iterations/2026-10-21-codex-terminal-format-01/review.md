# Codex terminal formatting — first capture

User task: make the emulated session look like the Codex CLI, with correct indentation and wrapping.

Renderer, capture times, appearance, viewports and source hashes are in [browser-checks.json](browser-checks.json). The browser interaction suite passed. These six files were opened at original resolution and inspected directly:

| Capture | Observation | Verdict |
| --- | --- | --- |
| terminal-plan-wide-light.png | Tool and result gutters align; title duplication is gone. The longer roadmap annotation stretches the row, leaving a visible strip beneath the terminal controls. Controls must sit at the bottom of their column. | Fail |
| terminal-diff-wide-light.png | Source line numbers, removed/added signs, syntax colors and background bands remain aligned. Full changes are available from the annotation. | Pass |
| terminal-diagnostic-wide-light.png | Shell continuations stay in the output gutter. Compiler location, source indentation and diagnostic marker remain related. | Pass |
| terminal-recheck-compact-light.png | Wrapped result and nested bullet align at 390 px; timeline and buttons fit. Long indivisible identifiers wrap as in a narrow terminal. | Pass |
| terminal-diff-compact-light.png | Only the first physical row of a changed source line has its number/sign; continuations align under code and preserve the change color. | Pass |
| terminal-diagnostic-compact-light.png | Bounded preview retains the failure and source location. The explicit details marker identifies omitted source. Controls remain accessible before the explanation. | Pass |

Overall: fail pending correction of the desktop control placement. Other captures in this folder were generated but were not visually reviewed; no verdict is claimed for them. The next capture also needs the opening reasoning action, matching the user's screenshot, in addition to the chapter's roadmap action.

Limitations: headless WebKit only; no native OS-input or assistive-technology testing.
