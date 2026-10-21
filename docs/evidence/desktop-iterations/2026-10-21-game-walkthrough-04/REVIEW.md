# Walkthrough iteration 04

Task: follow the real Bevy build, inspect its saved work, and read all seven guides. All 74 original frames below were opened and inspected in full. Viewports, appearances, renderer, source hashes and capture times are in browser-checks.json. That metadata explicitly records the interrupted harness run; it is not a passing test receipt.

The compact map spacing and Escape behavior are corrected. Two desktop reader frames still join dependency links; iteration 05 corrects those. The 320-pixel frame reveals a tab extending beyond its border, requiring a further focused correction. The remaining views pass. Headless WebKit only; native input and assistive technology were not tested.

| Frame | Observations | Verdict |
| --- | --- | --- |
| home-wide-light.png | Single-color headline, favorite-agent subtitle and concrete game brief have a clear hierarchy. Text and actions fit without overlap. | Pass |
| session-wide-light.png | The recorded action, criteria and open-record link are readable. Playback stays in its own row. | Pass |
| roadmap-wide-light.png | Four deliverables fit at readable zoom. Rules connect to board and opponent; those join at rematches. Ports attach correctly without crossings. | Pass |
| roadmap-reader-wide-light.png | The title and acceptance criteria are readable. Dependency buttons run together on desktop; this defect is corrected in iteration 05. | Fail |
| reasoning-wide-light.png | The investigation and conclusion fit at readable zoom; branch metadata and relationship endpoints remain legible and unambiguous. | Pass |
| reasoning-reader-wide-light.png | The finding and context are readable, with a clear return action and scrollable continuation below. | Pass |
| work-wide-light.png | Four completed implementation tasks have readable titles and separate status labels. | Pass |
| checks-wide-light.png | Executed checks and written reviews are distinguishable. Earlier failures remain visible in a scrollable history. | Pass |
| check-reader-wide-light.png | The failed command and actual compiler output wrap inside the reader. Return and playback controls remain separate. | Pass |
| followthrough-wide-light.png | Feature links and concrete explanations align clearly. The hierarchy survives reflow without crowded text. | Pass |
| install-wide-light.png | Agent support and preview availability are clear. Installation copy and command controls retain readable contrast. | Pass |
| docs-installation-wide-light.png | Setup commands wrap within their blocks; the agent sections and repair/upgrade instructions remain readable with consistent heading spacing. | Pass |
| docs-workflow-wide-light.png | The complete project sequence has clear headings, a four-piece plan, and readable examples. Previous/next links are distinct. | Pass |
| docs-roadmap-wide-light.png | The three-column deliverable/dependency/result table remains readable. Explanations and navigation stay within the content column. | Pass |
| docs-reasoning-wide-light.png | The losing move sequence and outcome table are readable. Investigation, evidence and conclusion headings make the example easy to follow. | Pass |
| docs-chunks-wide-light.png | Agent commands, completion criteria and longer-run instructions have clear hierarchy. Code wraps without overflowing. | Pass |
| docs-verification-wide-light.png | Executed results, visual review and current-input sections remain distinct. Commands and links have adequate contrast. | Pass |
| docs-concepts-wide-light.png | Saved records, source tools, sessions and desktop behavior have clear headings. Inline code and navigation wrap within the page. | Pass |
| home-wide-dark.png | Single-color headline, favorite-agent subtitle and concrete game brief have a clear hierarchy. Text and actions fit without overlap. | Pass |
| session-wide-dark.png | The recorded action, criteria and open-record link are readable. Playback stays in its own row. | Pass |
| roadmap-wide-dark.png | Four deliverables fit at readable zoom. Rules connect to board and opponent; those join at rematches. Ports attach correctly without crossings. | Pass |
| roadmap-reader-wide-dark.png | The title and acceptance criteria are readable. Dependency buttons run together on desktop; this defect is corrected in iteration 05. | Fail |
| reasoning-wide-dark.png | The investigation and conclusion fit at readable zoom; branch metadata and relationship endpoints remain legible and unambiguous. | Pass |
| reasoning-reader-wide-dark.png | The finding and context are readable, with a clear return action and scrollable continuation below. | Pass |
| work-wide-dark.png | Four completed implementation tasks have readable titles and separate status labels. | Pass |
| checks-wide-dark.png | Executed checks and written reviews are distinguishable. Earlier failures remain visible in a scrollable history. | Pass |
| check-reader-wide-dark.png | The failed command and actual compiler output wrap inside the reader. Return and playback controls remain separate. | Pass |
| followthrough-wide-dark.png | Feature links and concrete explanations align clearly. The hierarchy survives reflow without crowded text. | Pass |
| install-wide-dark.png | Agent support and preview availability are clear. Installation copy and command controls retain readable contrast. | Pass |
| docs-installation-wide-dark.png | Setup commands wrap within their blocks; the agent sections and repair/upgrade instructions remain readable with consistent heading spacing. | Pass |
| docs-workflow-wide-dark.png | The complete project sequence has clear headings, a four-piece plan, and readable examples. Previous/next links are distinct. | Pass |
| docs-roadmap-wide-dark.png | The three-column deliverable/dependency/result table remains readable. Explanations and navigation stay within the content column. | Pass |
| docs-reasoning-wide-dark.png | The losing move sequence and outcome table are readable. Investigation, evidence and conclusion headings make the example easy to follow. | Pass |
| docs-chunks-wide-dark.png | Agent commands, completion criteria and longer-run instructions have clear hierarchy. Code wraps without overflowing. | Pass |
| docs-verification-wide-dark.png | Executed results, visual review and current-input sections remain distinct. Commands and links have adequate contrast. | Pass |
| docs-concepts-wide-dark.png | Saved records, source tools, sessions and desktop behavior have clear headings. Inline code and navigation wrap within the page. | Pass |
| home-compact-light.png | Single-color headline, favorite-agent subtitle and concrete game brief have a clear hierarchy. Text and actions fit without overlap. | Pass |
| session-compact-light.png | The recorded action, criteria and open-record link are readable. Playback stays in its own row. | Pass |
| roadmap-compact-light.png | Four readable deliverables and statuses fit in the shorter list panel. The excessive empty space from iteration 03 is removed. | Pass |
| roadmap-reader-compact-light.png | Chunk title and acceptance criteria wrap clearly; dependency links continue below the initial scroll position. | Pass |
| reasoning-compact-light.png | The question, finding and conclusion remain readable in a compact list. The shorter panel brings playback closer to the records. | Pass |
| reasoning-reader-compact-light.png | The finding and context are readable, with a clear return action and scrollable continuation below. | Pass |
| work-compact-light.png | Four completed implementation tasks have readable titles and separate status labels. | Pass |
| checks-compact-light.png | Executed checks and written reviews are distinguishable. Earlier failures remain visible in a scrollable history. | Pass |
| check-reader-compact-light.png | The failed command and actual compiler output wrap inside the reader. Return and playback controls remain separate. | Pass |
| followthrough-compact-light.png | Feature links and concrete explanations align clearly. The hierarchy survives reflow without crowded text. | Pass |
| install-compact-light.png | Agent support and preview availability are clear. Installation copy and command controls retain readable contrast. | Pass |
| docs-installation-compact-light.png | Setup commands wrap within their blocks; the agent sections and repair/upgrade instructions remain readable with consistent heading spacing. | Pass |
| docs-workflow-compact-light.png | The complete project sequence has clear headings, a four-piece plan, and readable examples. Previous/next links are distinct. | Pass |
| docs-roadmap-compact-light.png | The three-column deliverable/dependency/result table remains readable. Explanations and navigation stay within the content column. | Pass |
| docs-reasoning-compact-light.png | The losing move sequence and outcome table are readable. Investigation, evidence and conclusion headings make the example easy to follow. | Pass |
| docs-chunks-compact-light.png | Agent commands, completion criteria and longer-run instructions have clear hierarchy. Code wraps without overflowing. | Pass |
| docs-verification-compact-light.png | Executed results, visual review and current-input sections remain distinct. Commands and links have adequate contrast. | Pass |
| docs-concepts-compact-light.png | Saved records, source tools, sessions and desktop behavior have clear headings. Inline code and navigation wrap within the page. | Pass |
| home-compact-dark.png | Single-color headline, favorite-agent subtitle and concrete game brief have a clear hierarchy. Text and actions fit without overlap. | Pass |
| session-compact-dark.png | The recorded action, criteria and open-record link are readable. Playback stays in its own row. | Pass |
| roadmap-compact-dark.png | Four readable deliverables and statuses fit in the shorter list panel. The excessive empty space from iteration 03 is removed. | Pass |
| roadmap-reader-compact-dark.png | Chunk title and acceptance criteria wrap clearly; dependency links continue below the initial scroll position. | Pass |
| reasoning-compact-dark.png | The question, finding and conclusion remain readable in a compact list. The shorter panel brings playback closer to the records. | Pass |
| reasoning-reader-compact-dark.png | The finding and context are readable, with a clear return action and scrollable continuation below. | Pass |
| work-compact-dark.png | Four completed implementation tasks have readable titles and separate status labels. | Pass |
| checks-compact-dark.png | Executed checks and written reviews are distinguishable. Earlier failures remain visible in a scrollable history. | Pass |
| check-reader-compact-dark.png | The failed command and actual compiler output wrap inside the reader. Return and playback controls remain separate. | Pass |
| followthrough-compact-dark.png | Feature links and concrete explanations align clearly. The hierarchy survives reflow without crowded text. | Pass |
| install-compact-dark.png | Agent support and preview availability are clear. Installation copy and command controls retain readable contrast. | Pass |
| docs-installation-compact-dark.png | Setup commands wrap within their blocks; the agent sections and repair/upgrade instructions remain readable with consistent heading spacing. | Pass |
| docs-workflow-compact-dark.png | The complete project sequence has clear headings, a four-piece plan, and readable examples. Previous/next links are distinct. | Pass |
| docs-roadmap-compact-dark.png | The three-column deliverable/dependency/result table remains readable. Explanations and navigation stay within the content column. | Pass |
| docs-reasoning-compact-dark.png | The losing move sequence and outcome table are readable. Investigation, evidence and conclusion headings make the example easy to follow. | Pass |
| docs-chunks-compact-dark.png | Agent commands, completion criteria and longer-run instructions have clear hierarchy. Code wraps without overflowing. | Pass |
| docs-verification-compact-dark.png | Executed results, visual review and current-input sections remain distinct. Commands and links have adequate contrast. | Pass |
| docs-concepts-compact-dark.png | Saved records, source tools, sessions and desktop behavior have clear headings. Inline code and navigation wrap within the page. | Pass |
| reflow-320.png | Records and transport reflow, but the Checks tab extends past the session border. This is a blocking narrow-width defect. | Fail |
| reflow-820.png | The four records, navigation and transport fit with clear alignment and readable spacing. | Pass |
