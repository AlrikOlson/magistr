# Iteration review

Rejected: crowded prerequisite fan-in, same-column loops and clipped inspector context. Later fixes are in 15 and 16. Only observations actually made are listed as reviewed.

| Image | User task | Observation | Verdict |
| --- | --- | --- | --- |
| [01-inspector-compact](01-inspector-compact.png) | Inspect next work at minimum size. | The drawer preserves a visible prerequisite strip and a clear Back action. Detail content scrolls; the offscreen selected item is named in the inspector. This is a deliberate compact overlay, not a complete graph view. | pass |
| [01-inspector-light](01-inspector-light.png) | Inspect next work without losing its prerequisite. | The 390-pixel inspector leaves both prerequisite and selected work visible at actual size. The selected route has a visible Needed for label and separated endpoints; evidence and Back remain reachable. | pass |
| [01-roadmap-compact-dark](01-roadmap-compact-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [01-roadmap-compact-light](01-roadmap-compact-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [01-roadmap-dark](01-roadmap-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [01-roadmap-light](01-roadmap-light.png) | Choose next work and understand prerequisites. | Completed storage and its ready consumer share a horizontal route; the missing prerequisite is distinguished with a dashed piece and line. Text remains actual size and Next is identified without a second heading. | pass |
| [02-evidence-light](02-evidence-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [02-reasoning-compact-dark](02-reasoning-compact-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [02-reasoning-compact-light](02-reasoning-compact-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [02-reasoning-dark](02-reasoning-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [02-reasoning-light](02-reasoning-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [03-reasoning-compact](03-reasoning-compact.png) | Trace the investigation and its conclusion. | Both decision pieces remain at 100%; outgoing Investigate and returning Conclude have separate ports and clear arrows. Question and origin/conclusion controls sit above the branch without touching routes. | pass |
| [03-shell-compact-dark](03-shell-compact-dark.png) | Choose next work in a compact window. | Project name and all destination tabs fit one row. Readiness counts and five actual work pieces fit at 100%; two prerequisite arrows attach to distinct rows. No header or toolbar overlap. | pass |
| [03-shell-compact-light](03-shell-compact-light.png) | Choose next work in a compact window. | The same 100% arrangement retains legible titles, explicit Next badge and dashed unavailable prerequisite in light appearance; controls fit within the window. | pass |
| [03-shell-conversation](03-shell-conversation.png) | Return to the conversation and draft. | Project name, working copy and destination tabs are visible. The title and controls share one row; the composer remains within the window. Scrolled reading position is preserved. | pass |
| [03-shell-roadmap-dark](03-shell-roadmap-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [03-shell-roadmap-light](03-shell-roadmap-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-default-roadmap-compact-dark](04-default-roadmap-compact-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-default-roadmap-compact-light](04-default-roadmap-compact-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-dense-roadmap-arranged](04-dense-roadmap-arranged.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-dense-roadmap-default](04-dense-roadmap-default.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-dense-roadmap-overview](04-dense-roadmap-overview.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-nested-reasoning-compact](04-nested-reasoning-compact.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-nested-reasoning-dark](04-nested-reasoning-dark.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-nested-reasoning-default](04-nested-reasoning-default.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-nested-reasoning-light](04-nested-reasoning-light.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-nested-reasoning-selected](04-nested-reasoning-selected.png) | Not reviewed. | No inspection claimed. | unreviewed |
| [04-prerequisite-bundle](04-prerequisite-bundle.png) | Understand a work item with 500 prerequisites. | The seven rendered incoming routes converge within too little vertical space on the consumer. Individual arrows crowd together. Aggregate the large prerequisite set into one explicit bundle before acceptance. | fail |
