# Desktop GUI iterations — 2026-10-15

Thirty delivered interface chunks cover project navigation, conversations, documents, settings, requests, roadmap and reasoning views, execution records and feedback. Each row links the accepted review for that iteration; failed attempts remain alongside it. These 302 accepted captures are historical evidence at their recorded source hashes, not a claim that every image depicts the final combined build.

Storybook now owns the named interface states used for screenshot capture. Stories render the production desktop UI with isolated observations, check the expected state before capture, and record their story ID, caption, task and visible context. The capture test also checks that a changed screen is rejected. Visual inspection remains a separate recorded gate.

Run `npm --prefix crates/magistr-desktop run storybook` to browse the stories. See [desktop verification](../../../development.md#desktop-verification) for focused capture and evidence commands.

| Chunk | Delivered change | Accepted evidence |
|---|---|---|
| 1 | Keep project destinations together in one toolbar | [Review and 10 captures](01-project-bar/review.md) |
| 2 | Make waiting requests a compact actionable queue | [Review and 4 captures](02-attention-queue/review.md) |
| 3 | Tighten project and conversation navigation | [Review and 4 captures](03-project-rail/review.md) |
| 4 | Align project facts beside their actions | [Review and 4 captures](04-project-directory/revision-2/review.md) |
| 5 | Turn the project opening area into a compact task strip | [Review and 8 captures](05-project-overview/review.md) |
| 6 | Balance conversation titles and nearby controls | [Review and 8 captures](06-conversation-heading/review.md) |
| 7 | Group draft controls around the message editor | [Review and 8 captures](07-composer-controls/review.md) |
| 8 | Use a compact readable hierarchy for agent replies | [Review and 8 captures](08-reply-rhythm/revision-2/review.md) |
| 9 | Present sent messages as compact conversation entries | [Review and 4 captures](09-authored-messages/review.md) |
| 10 | Compact source toolbars and align code gutters | [Review and 8 captures](10-source-documents/revision-2/review.md) |
| 11 | Align dense tables for comparison | [Review and 8 captures](11-document-tables/review.md) |
| 12 | Make tool activity easier to scan | [Review and 8 captures](12-tool-timeline/review.md) |
| 13 | Reduce the space used by reply navigation | [Review and 12 captures](13-reply-outline/revision-2/review.md) |
| 14 | Show pending files in compact attachment rows | [Review and 8 captures](14-attachment-list/revision-5/review.md) |
| 15 | Align agent settings and usage in the details panel | [Review and 8 captures](15-agent-details/revision-3/review.md) |
| 16 | Group appearance and keyboard settings more tightly | [Review and 8 captures](16-appearance-settings/revision-3/review.md) |
| 17 | Improve the saved conversation catalog | [Review and 8 captures](17-saved-history/revision-3/review.md) |
| 18 | Align conversation search results and states | [Review and 8 captures](18-conversation-finder/review.md) |
| 19 | Compact project opening and setup guidance | [Review and 16 captures](19-project-admission/revision-2/review.md) |
| 20 | Keep permission choices close to their request | [Review and 8 captures](20-permission-choices/review.md) |
| 21 | Align roadmap search and track controls | [Review and 8 captures](21-roadmap-controls/review.md) |
| 22 | Group canvas navigation controls compactly | [Review and 20 captures](22-canvas-controls/revision-3/review.md) |
| 23 | Improve the selected work summary | [Review and 8 captures](23-item-inspector/revision-2/review.md) |
| 24 | Align required and dependent work | [Review and 16 captures](24-dependency-rows/review.md) |
| 25 | Make authored requirements and checklist marks compact | [Review and 8 captures](25-requirement-lists/review.md) |
| 26 | Clarify investigation selection and search | [Review and 12 captures](26-reasoning-controls/review.md) |
| 27 | Group reasoning evidence for efficient reading | [Review and 12 captures](27-reasoning-evidence/review.md) |
| 28 | Tighten current work and task summaries | [Review and 12 captures](28-execution-plan/review.md) |
| 29 | Align verification and activity records | [Review and 24 captures](29-check-activity/review.md) |
| 30 | Improve feedback browsing and detail hierarchy | [Review and 24 captures](30-feedback-records/revision-2/review.md) |

Verification used focused headless WebKit interactions and manual review at 1280 × 820 and 760 × 550 in dark and light appearances, with affected dense, empty, selected and expanded states. Focused interface-index, visual-evidence and whitespace checks passed for each chunk. The feedback captures in chunk 30 also inspect the research disclosure that shares the check-row styling changed in chunk 29. Native OS input, assistive technology and Rust command integration were not exercised in this batch.

## Corrected screenshot captions

The first five iterations used internal scene names as filenames. These did not consistently describe the visible screen: `welcome` is the Projects directory, and `projects` is a directory with a sidebar at desktop size but an expanded project navigation panel at compact size. The captions below correct that naming defect. Each original image was reopened and inspected. Original image bytes, capture receipts, review notes and failed captures are preserved.

The iteration folder names describe the change being reviewed, not necessarily the entire screen pictured. Future captures use verified Storybook story IDs and carry the story name, caption, user task, visible heading and selected view in their receipts.

| Original capture | Visible screen and state | Size / appearance | Review target | Original verdict |
|---|---|---|---|---|
| [01-project-bar/03-reasoning-compact.png](01-project-bar/03-reasoning-compact.png) | Reasoning canvas with an investigation branch and its return to the main reasoning. | 760 × 550 / light | Keep project destinations together in one toolbar | pass |
| [01-project-bar/03-shell-compact-dark.png](01-project-bar/03-shell-compact-dark.png) | Desktop roadmap with five work items and two dependency connections. | 760 × 550 / dark | Keep project destinations together in one toolbar | pass |
| [01-project-bar/03-shell-compact-light.png](01-project-bar/03-shell-compact-light.png) | Desktop roadmap with five work items and two dependency connections. | 760 × 550 / light | Keep project destinations together in one toolbar | pass |
| [01-project-bar/03-shell-conversation.png](01-project-bar/03-shell-conversation.png) | Release plan conversation, agent reply and unsent message. | 1280 × 820 / light | Keep project destinations together in one toolbar | pass |
| [01-project-bar/03-shell-roadmap-dark.png](01-project-bar/03-shell-roadmap-dark.png) | Desktop roadmap with five work items and two dependency connections. | 1280 × 820 / dark | Keep project destinations together in one toolbar | pass |
| [01-project-bar/03-shell-roadmap-light.png](01-project-bar/03-shell-roadmap-light.png) | Desktop roadmap with five work items and two dependency connections. | 1280 × 820 / light | Keep project destinations together in one toolbar | pass |
| [01-project-bar/conversation-compact-dark.png](01-project-bar/conversation-compact-dark.png) | Release plan conversation, agent reply and unsent message. | 760 × 550 / dark | Keep project destinations together in one toolbar | pass |
| [01-project-bar/conversation-compact-light.png](01-project-bar/conversation-compact-light.png) | Release plan conversation, agent reply and unsent message. | 760 × 550 / light | Keep project destinations together in one toolbar | pass |
| [01-project-bar/conversation-desktop-dark.png](01-project-bar/conversation-desktop-dark.png) | Release plan conversation, agent reply and unsent message. | 1280 × 820 / dark | Keep project destinations together in one toolbar | pass |
| [01-project-bar/conversation-desktop-light.png](01-project-bar/conversation-desktop-light.png) | Release plan conversation, agent reply and unsent message. | 1280 × 820 / light | Keep project destinations together in one toolbar | pass |
| [02-attention-queue/projects-compact-dark.png](02-attention-queue/projects-compact-dark.png) | Expanded project navigation panel with a waiting request; the project directory is hidden. | 760 × 550 / dark | Make waiting requests a compact actionable queue | pass |
| [02-attention-queue/projects-compact-light.png](02-attention-queue/projects-compact-light.png) | Expanded project navigation panel with a waiting request; the project directory is hidden. | 760 × 550 / light | Make waiting requests a compact actionable queue | pass |
| [02-attention-queue/projects-desktop-dark.png](02-attention-queue/projects-desktop-dark.png) | Projects directory with project navigation and a waiting request in the sidebar. | 1280 × 820 / dark | Make waiting requests a compact actionable queue | pass |
| [02-attention-queue/projects-desktop-light.png](02-attention-queue/projects-desktop-light.png) | Projects directory with project navigation and a waiting request in the sidebar. | 1280 × 820 / light | Make waiting requests a compact actionable queue | pass |
| [03-project-rail/projects-compact-dark.png](03-project-rail/projects-compact-dark.png) | Expanded project navigation panel with a waiting request; the project directory is hidden. | 760 × 550 / dark | Tighten project and conversation navigation | pass |
| [03-project-rail/projects-compact-light.png](03-project-rail/projects-compact-light.png) | Expanded project navigation panel with a waiting request; the project directory is hidden. | 760 × 550 / light | Tighten project and conversation navigation | pass |
| [03-project-rail/projects-desktop-dark.png](03-project-rail/projects-desktop-dark.png) | Projects directory with project navigation and a waiting request in the sidebar. | 1280 × 820 / dark | Tighten project and conversation navigation | pass |
| [03-project-rail/projects-desktop-light.png](03-project-rail/projects-desktop-light.png) | Projects directory with project navigation and a waiting request in the sidebar. | 1280 × 820 / light | Tighten project and conversation navigation | pass |
| [04-project-directory/welcome-compact-dark.png](04-project-directory/welcome-compact-dark.png) | Projects directory in list view; project counts and conversation rows. | 760 × 550 / dark | Align project facts beside their actions | fail: singular count label; corrected in revision-2 |
| [04-project-directory/welcome-compact-light.png](04-project-directory/welcome-compact-light.png) | Projects directory in list view; project counts and conversation rows. | 760 × 550 / light | Align project facts beside their actions | fail: singular count label; corrected in revision-2 |
| [04-project-directory/welcome-desktop-dark.png](04-project-directory/welcome-desktop-dark.png) | Projects directory in list view; project counts and conversation rows. | 1280 × 820 / dark | Align project facts beside their actions | fail: singular count label; corrected in revision-2 |
| [04-project-directory/welcome-desktop-light.png](04-project-directory/welcome-desktop-light.png) | Projects directory in list view; project counts and conversation rows. | 1280 × 820 / light | Align project facts beside their actions | fail: singular count label; corrected in revision-2 |
| [05-project-overview/welcome-compact-dark.png](05-project-overview/welcome-compact-dark.png) | Projects directory in list view; project setup explanation collapsed. | 760 × 550 / dark | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-compact-light.png](05-project-overview/welcome-compact-light.png) | Projects directory in list view; project setup explanation collapsed. | 760 × 550 / light | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-desktop-dark.png](05-project-overview/welcome-desktop-dark.png) | Projects directory in list view; project setup explanation collapsed. | 1280 × 820 / dark | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-desktop-light.png](05-project-overview/welcome-desktop-light.png) | Projects directory in list view; project setup explanation collapsed. | 1280 × 820 / light | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-setup-compact-dark.png](05-project-overview/welcome-setup-compact-dark.png) | Projects directory in list view; project setup explanation expanded. | 760 × 550 / dark | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-setup-compact-light.png](05-project-overview/welcome-setup-compact-light.png) | Projects directory in list view; project setup explanation expanded. | 760 × 550 / light | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-setup-desktop-dark.png](05-project-overview/welcome-setup-desktop-dark.png) | Projects directory in list view; project setup explanation expanded. | 1280 × 820 / dark | Turn the project opening area into a compact task strip | pass |
| [05-project-overview/welcome-setup-desktop-light.png](05-project-overview/welcome-setup-desktop-light.png) | Projects directory in list view; project setup explanation expanded. | 1280 × 820 / light | Turn the project opening area into a compact task strip | pass |
| [04-project-directory/revision-2/welcome-compact-dark.png](04-project-directory/revision-2/welcome-compact-dark.png) | Projects directory in list view; project counts and conversation rows. | 760 × 550 / dark | Align project facts beside their actions | pass |
| [04-project-directory/revision-2/welcome-compact-light.png](04-project-directory/revision-2/welcome-compact-light.png) | Projects directory in list view; project counts and conversation rows. | 760 × 550 / light | Align project facts beside their actions | pass |
| [04-project-directory/revision-2/welcome-desktop-dark.png](04-project-directory/revision-2/welcome-desktop-dark.png) | Projects directory in list view; project counts and conversation rows. | 1280 × 820 / dark | Align project facts beside their actions | pass |
| [04-project-directory/revision-2/welcome-desktop-light.png](04-project-directory/revision-2/welcome-desktop-light.png) | Projects directory in list view; project counts and conversation rows. | 1280 × 820 / light | Align project facts beside their actions | pass |