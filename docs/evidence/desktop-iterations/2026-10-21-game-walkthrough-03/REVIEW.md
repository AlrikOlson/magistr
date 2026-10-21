# Walkthrough iteration 03

Task: follow a real Bevy project and inspect its roadmap, reasoning, work and checks. All 47 original frames below were opened and inspected in full. Metadata is in browser-checks.json. Desktop is 1280 × 1000; compact is 390 × 844. Headless WebKit only; native input and assistive technology were not tested.

**Iteration failed:** compact maps leave too much empty space; the desktop chunk reader joins adjacent dependency links; Escape did not close mobile navigation when pointer clicking left focus outside the nav. These findings remain blocking until corrected captures pass.

| Frame | Observations | Verdict |
| --- | --- | --- |
| home-wide-light.png | Single-color headline, favorite-agent subtitle and concrete game brief are readable; hierarchy and spacing are clear. | Pass |
| session-wide-light.png | One recorded tool action and its criteria fit with readable output; playback controls stay separate. | Pass |
| roadmap-wide-light.png | Four deliverables fit at 100%. Correct ports connect the rules to board/opponent and both to rematches; no crossings or overlap. | Pass |
| roadmap-reader-wide-light.png | Title and criteria are readable, but the two dependency links run together without spacing. | Fail |
| reasoning-wide-light.png | Investigation and conclusion fit at readable zoom with visible branch metadata and attached relationships. | Pass |
| reasoning-reader-wide-light.png | Finding, context and rationale use readable paragraphs in a scrollable reader; return control stays first. | Pass |
| work-wide-light.png | Four completed implementation tasks remain separate from roadmap deliverables and have clear statuses. | Pass |
| checks-wide-light.png | Executed checks and reviews are distinguishable; prior failures remain visible in the scrollable history. | Pass |
| check-reader-wide-light.png | The failed command and compiler output are readable and wrap within the reader. | Pass |
| followthrough-wide-light.png | Specific feature links and supporting explanations align and reflow clearly. | Pass |
| install-wide-light.png | Agent support and source-preview availability are explicit; the command block and install link are readable. | Pass |
| docs-installation-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-workflow-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-roadmap-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-reasoning-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-chunks-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-verification-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-concepts-wide-light.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| home-wide-dark.png | Single-color headline, favorite-agent subtitle and concrete game brief are readable; hierarchy and spacing are clear. | Pass |
| session-wide-dark.png | One recorded tool action and its criteria fit with readable output; playback controls stay separate. | Pass |
| roadmap-wide-dark.png | Four deliverables fit at 100%. Correct ports connect the rules to board/opponent and both to rematches; no crossings or overlap. | Pass |
| roadmap-reader-wide-dark.png | Title and criteria are readable, but the two dependency links run together without spacing. | Fail |
| reasoning-wide-dark.png | Investigation and conclusion fit at readable zoom with visible branch metadata and attached relationships. | Pass |
| reasoning-reader-wide-dark.png | Finding, context and rationale use readable paragraphs in a scrollable reader; return control stays first. | Pass |
| work-wide-dark.png | Four completed implementation tasks remain separate from roadmap deliverables and have clear statuses. | Pass |
| checks-wide-dark.png | Executed checks and reviews are distinguishable; prior failures remain visible in the scrollable history. | Pass |
| check-reader-wide-dark.png | The failed command and compiler output are readable and wrap within the reader. | Pass |
| followthrough-wide-dark.png | Specific feature links and supporting explanations align and reflow clearly. | Pass |
| install-wide-dark.png | Agent support and source-preview availability are explicit; the command block and install link are readable. | Pass |
| docs-installation-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-workflow-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-roadmap-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-reasoning-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-chunks-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-verification-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| docs-concepts-wide-dark.png | The complete guide has readable headings, paragraphs, examples and navigation. Tables and code stay within the content column; contrast and spacing are consistent. | Pass |
| home-compact-light.png | Single-color headline, favorite-agent subtitle and concrete game brief are readable; hierarchy and spacing are clear. | Pass |
| session-compact-light.png | One recorded tool action and its criteria fit with readable output; playback controls stay separate. | Pass |
| roadmap-compact-light.png | The records remain readable as a list, but the panel leaves excessive empty space below them. | Fail |
| roadmap-reader-compact-light.png | Chunk title and criteria wrap clearly; the dependency links are below this initial scroll position. | Pass |
| reasoning-compact-light.png | The records remain readable as a list, but the panel leaves excessive empty space below them. | Fail |
| reasoning-reader-compact-light.png | Finding, context and rationale use readable paragraphs in a scrollable reader; return control stays first. | Pass |
| work-compact-light.png | Four completed implementation tasks remain separate from roadmap deliverables and have clear statuses. | Pass |
| checks-compact-light.png | Executed checks and reviews are distinguishable; prior failures remain visible in the scrollable history. | Pass |
| check-reader-compact-light.png | The failed command and compiler output are readable and wrap within the reader. | Pass |
| followthrough-compact-light.png | Specific feature links and supporting explanations align and reflow clearly. | Pass |
| install-compact-light.png | Agent support and source-preview availability are explicit; the command block and install link are readable. | Pass |
