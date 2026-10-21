# Website and documentation trope corpus

Researched 2026-10-20 for Magistr after the owner rejected the split-color headings and asked for a broader examination of recurring AI-generated design and writing.

This corpus contains **67 patterns from 17 sources**. It covers visual treatment, composition, proof, controls, motion, behavior, copy and documentation. The findings below describe the first uncommitted website rewrite, not the old live site or a later corrected design.

## Method and limits

Searches combined AI-generated website/design homogenization, interface pattern catalogs, AI-writing observations, and primary documentation/usability guidance. Relevant search results were opened and inspected. Priority went to original authors, the vendor's own design guidance, original studies, working pattern catalogs and documentation owners. SEO articles that merely repeat a list were not used as corroboration.

“Common” here means recurring in the inspected sources. This is a source corpus and a review aid, **not a representative sample of AI-generated websites or a frequency ranking of the internet**. Catalog overlap does not prove independent observation. Anthropic's article and its current guidance are one source family. The slop-detect checklist derives from Krebs and is not independent confirmation of his scan. The other catalogs may also influence each other; their agreement is not a measured probability.

Sources attached to each row substantiate the pattern being discussed. The Magistr diagnosis, proposed correction and legitimate-use notes are this review's own analysis of the local source and captured pages. Documentation and accessibility guidance identifies general quality problems; it does not establish that those problems are uniquely or especially frequent in AI output. A common font, a useful numbered procedure, a dark theme, a card, a punctuation mark or an accent color is not inherently a defect.

The primary observational scan reported **1,590 Show HN submissions**, with **347** triggering four or more markers, **508** triggering two or three, and **735** triggering zero or one. Those are detector-defined visual groups, not known AI authorship. Do not repeat the derivative checklist's conflicting 67% figure. [K1](#k1), [K2](#k2)

The research framework discusses homogenization risks without measuring individual trope prevalence. The prototype study used **92 students and ten static prototypes**; its conclusion did not establish authorship as the determinant of quality. Neither study justifies a universal aesthetic ban. [R1](#r1), [R2](#r2)

WebAIM found low-contrast text on **83.9%** of its 2026 homepage sample. That is evidence of a widespread accessibility problem across the web, not evidence about AI authorship. Contrast must meet the applicable standard regardless of visual style. [U2](#u2), [U3](#u3)

## What the first review missed

The initial review checked readability and interaction but accepted an interchangeable technical/editorial treatment. The owner correctly identified the split-color headline. This broader review found 26 clear occurrences, four partial matches and nine areas needing contextual copy review; other patterns are explicitly marked absent or already improved.

The main cluster is visible in [the hero](desktop-iterations/2026-10-20-website-everyday-use/home-intro-wide-light.png), [the product pitch](desktop-iterations/2026-10-20-website-everyday-use/home-product-wide-light.png), and [the compact walkthrough](desktop-iterations/2026-10-20-website-everyday-use/home-workflow-0-compact-dark.png):

- A generic slogan receives oversized, tightly tracked, split-color typography.
- Numbered section labels, miniature monospace text, decorative identifiers, arrows and accent stripes make ordinary content look like technical records.
- A schematic, a walkthrough, a feature pitch, guide links and the footer repeatedly explain continuation.
- Layers of headings and nested framing postpone the real session evidence.
- Passing contrast, layout and keyboard checks were incorrectly treated as enough evidence of a good design direction.

The public documentation rewrite is more closely aligned with the owner's work: research, revise priorities, complete bounded work, inspect results and resume later. It still needs a careful copy pass for repeated openings, unnecessary introductions, overexplained controls and words such as “simply.”

Source locations are relative to `apps/website/src/` unless stated otherwise. The shared stylesheet owns type and spacing; page/component source owns structure and copy. Corrections belong there, not in an overriding second stylesheet.

## Visual

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T01. Phrase colored inside a headline | [A1](#a1), [K1](#k1), [P1](#p1) | **present**: index.astro: hero h1 span and product h2 span; og-image.svg | Use one text color; let the sentence explain the product. |
| T02. Default purple gradient | [A2](#a2), [K1](#k1), [P2](#p2) | **not observed**: No gradient hero in the reviewed build. | A brand gradient can be justified; changing hue alone does not solve generic composition. |
| T03. Cream-and-editorial costume | [A1](#a1), [P3](#p3) | **partial**: Cream surface and orange accent exist; no editorial serif is used. | Judge the whole composition. Do not invent a new palette merely to evade a checklist. |
| T04. Near-black with one hot accent | [A1](#a1), [P3](#p3) | **present**: Dark theme uses green-black surfaces and orange across unrelated roles. | Keep the supported dark appearance; reduce decorative use of the accent. |
| T05. Hairline editorial costume | [A1](#a1), [P3](#p3) | **present**: Square panels, fine rules, labels and numbered sections recur throughout. | Use separators where grouping needs them; remove ceremonial framing. |
| T06. Typeface chosen by reflex | [A2](#a2), [K1](#k1), [P2](#p2) | **review**: Instrument Sans and IBM Plex Mono are used; font popularity alone is no defect. | Keep readable type; remove the arbitrary use of monospace for ordinary prose labels. |

## Typography

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T07. Crushed heading tracking | [K2](#k2) | **present**: global.css sets hero tracking to -.065em. | Give headings normal word shapes and test narrow wrapping. |
| T08. Forced dramatic line breaks | [P1](#p1) | **present**: Hero, product and install titles contain hard-coded br tags. | Let content and available width determine the line breaks. |
| T09. Huge vague headings | [P1](#p1), [K2](#k2) | **present**: The hero says Pick up where you left off at up to 5.25rem. | State the concrete capability at a size proportionate to the information. |
| T10. Tiny monospace metadata | [A1](#a1), [P2](#p2) | **present**: hero-note, mono-label and record-label shrink ordinary labels. | Reserve monospace for source and commands; use readable body typography elsewhere. |
| T11. Eyebrow above every heading | [A1](#a1), [P1](#p1), [P3](#p3) | **present**: Hero plus four landing sections repeat this structure. | Remove redundant category labels; keep a heading that identifies its content. |
| T12. Decorative metadata punctuation | [A1](#a1) | **present**: M / 01 and numbered slash labels imitate technical records. | Delete invented identifiers and use normal text for actual version/platform information. |
| T13. Mechanical bold-label lists | [W1](#w1) | **not observed**: Public guide prose largely uses paragraphs; the skill table is a genuine lookup. | Use a list only when readers need parallel entries, not to manufacture emphasis. |

## Surfaces

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T14. Accent color everywhere | [P2](#p2) | **present**: Titles, numbers, arrows, stripes and ordinary labels share orange. | Reserve strong accent for actions and meaningful selected states. |
| T15. Glass and glow stacking | [K1](#k1), [P2](#p2), [P3](#p3) | **not observed**: No blurred panels or glow backgrounds in this iteration. | Do not add effects as a substitute for useful content. |
| T16. Background grids and blobs | [P2](#p2), [P3](#p3) | **not observed**: No decorative grid, particle field or 3D orb. | Use imagery only when it conveys product information. |
| T17. Colored callout stripes | [K1](#k1), [P2](#p2), [P3](#p3) | **present**: work-schematic has a top stripe; workflow-takeaway has a left stripe. | Keep semantic selection indicators; remove unrelated emphasis bars. |
| T18. Pills on every element | [P2](#p2), [P3](#p3) | **not observed**: Current controls use restrained square geometry. | Tags and filters can use pills; do not impose one shape on every role. |
| T19. Cards inside cards | [P1](#p1), [P3](#p3), [K2](#k2) | **present**: walkthrough encloses example-record, code divisions and repeated rules. | Flatten the example so the explanation and actual record carry the hierarchy. |

## Structure

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T20. Standard SaaS section sequence | [P2](#p2), [I1](#i1) | **partial**: There are no pricing or testimonial blocks, but the page repeats generic promise/feature/proof sections. | Order content by the visitor's questions about Magistr. |
| T21. Three-card feature quota | [P2](#p2), [P3](#p3) | **not observed**: The four use cases correspond to real activities; no three-card feature grid. | Choose the number of items from actual content. Three useful steps are allowed. |
| T22. Bento widgets without purpose | [P2](#p2), [P3](#p3), [K2](#k2) | **not observed**: No bento layout or decorative miniature dashboard. | Use a real product screen rather than invented widgets. |
| T23. Padding that stretches weak content | [P1](#p1), [P3](#p3) | **present**: section-block adds 6.5rem above and below each section; the hero adds 6rem. | Reduce empty geometry and put the evidence nearer the explanation. |
| T24. Identical section rhythm | [P2](#p2) | **present**: Repeated label, large heading, summary and bordered content. | Give each section only the structure its material requires. |
| T25. Stacked headings before substance | [P3](#p3) | **present**: The walkthrough has section label, h2, description, panel label, h3, tab label and h4. | Remove repeated labels and titles before the actual example. |
| T26. Decorative section numbering | [A1](#a1), [K1](#k1) | **present**: 01–04 number unrelated homepage sections. | Number actual procedures only; visitors need not read landing sections in sequence. |
| T27. Template footer inventory | [P2](#p2) | **not observed**: Footer links correspond to existing guides and repository. | Keep useful destinations; remove the repeated slogan and duplicate guide promotion. |
| T28. Proof buried below promises | [P1](#p1), [I1](#i1) | **present**: Original session screenshots appear after hero, schematic, walkthrough and product pitch. | Move actual sessions alongside the first explanation. |

## Proof

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T29. Unverified customer logo strip | [P2](#p2), [P3](#p3) | **not observed**: Codex and Claude Code are stated integrations, not claimed customers. | Keep integration names plainly labeled; never imply endorsement. |
| T30. Invented testimonial people | [P2](#p2), [P3](#p3) | **not observed**: Excerpts come from documented agent sessions and are labeled as such. | Preserve original attribution and distinguish demonstrations from endorsements. |
| T31. Unsupported metric banners | [K1](#k1), [P2](#p2), [P3](#p3) | **not observed**: The passing test count belongs to the recorded example; no growth or speed claim. | Retain result scope and date; do not turn one test into a general performance promise. |
| T32. Fake product or terminal screenshot | [P2](#p2), [P3](#p3), [I1](#i1) | **not observed**: Product captures are real and linked; the walkthrough is explicitly explanatory. | Keep actual evidence and its limitations. Do not make a schematic resemble live UI. |
| T33. Decoration presented as explanation | [I1](#i1) | **present**: Hero schematic repeats the same four verbs as the prose and later sections. | Remove the schematic; show what a later session actually reads. |
| T34. Meaningless upward chart | [P3](#p3) | **not observed**: No fabricated chart. | A chart needs real data, units and a question it answers. |

## Controls

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T35. Icons beside every label | [P3](#p3) | **not observed**: No broad icon carpet; theme and menu icons have control roles. | Keep conventional control icons with accessible names. |
| T36. Arrows appended everywhere | [A1](#a1), [P2](#p2), [P3](#p3) | **present**: Internal links and workflow tabs use decorative diagonal arrows. | Remove decoration; retain direction only when it communicates actual navigation. |

## Motion

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T37. Scroll reveals on all sections | [A1](#a1), [P2](#p2) | **not observed**: No repeated entry animation. | Keep content available without waiting for an effect. |
| T38. Hover motion on inert content | [A1](#a1), [P3](#p3) | **not observed**: Reviewed states do not lift noninteractive cards. | Reserve feedback for actual actions. |
| T39. Autoplay logos and counters | [P3](#p3) | **not observed**: No ticker or animated vanity counter. | Use static, attributed evidence unless motion explains a real change. |

## Behavior

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T40. Happy-path-only interface | [P2](#p2), [I1](#i1) | **not observed in tested scope**: Search empty/error, clipboard denial and mobile navigation were exercised. | Preserve recovery tests; do not infer full native or assistive-technology coverage. |
| T41. Different rules on each route | [I1](#i1) | **not observed**: Pages share Base, Nav, Footer and the same docs layout. | Continue changing common presentation at its owning component. |
| T42. Desktop columns merely stacked | [I1](#i1) | **partial**: 390 px pages fit, but the long walkthrough stack postpones proof. | Review reading order and length at compact width, not only overflow. |

## Accessibility

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T43. Low-contrast sophistication | [K1](#k1), [U2](#u2), [U3](#u3) | **not observed in checked roles**: Primary text/action contrast checks passed; no full accessibility certification. | Preserve measured contrast; restrained appearance must remain legible. |
| T44. Controls without usable semantics | [P1](#p1), [U2](#u2) | **not observed in tested scope**: Tabs have keyboard/focus checks and the menu restores focus. | Keep meaningful names and behavior; automation is only part of accessibility review. |

## Copy

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T45. Interchangeable product promise | [I1](#i1), [P3](#p3) | **present**: Pick up where you left off could describe many unrelated tools. | Name saved roadmap, decisions and checks, and the coding agents that use them. |
| T46. Promotional buzzword verbs | [P2](#p2), [P3](#p3), [D1](#d1) | **not observed**: The rewrite removed inflated workflow-layer phrasing and does not use revolutionize-style claims. | Describe a task and a concrete result. |
| T47. Manufactured not-X-but-Y contrast | [W1](#w1), [P3](#p3) | **review**: Some prose uses useful scope distinctions; each contrast needs a real reader misconception. | State the positive fact directly unless the distinction prevents an actual error. |
| T48. Triads used as cadence | [W1](#w1) | **present**: Decide what's next. Work through it. Come back tomorrow. is a staged three-part slogan. | Use ordinary explanatory sentences; retain actual three-part facts when needed. |
| T49. Choppy imperative cadence | [D1](#d1) | **present**: Hero schematic repeatedly fragments ordinary explanations into commands. | Write connected prose that explains the relationship between actions. |
| T50. Heading and paragraph echo | [P1](#p1), [P3](#p3), [D2](#d2) | **present**: Hero, schematic, product intro, guide links and footer repeat continuation claims. | Give each section a different informational job. |
| T51. Filler signposting | [D1](#d1), [W1](#w1) | **review**: Guide introductions and continuation sentences need a sentence-by-sentence pass. | Delete an introduction when the heading already says it. |
| T52. Overemphasis and title-case clutter | [W1](#w1) | **partial**: Prose headings use sentence case, but CSS converts many labels to uppercase. | Use emphasis for things the reader must distinguish, not every block. |
| T53. Forced cheerful assistant voice | [D1](#d1) | **not observed**: Error messages give recovery without cheers or banter. | Respect the reader's situation; friendliness does not require performance. |
| T54. Vague action labels | [P2](#p2), [I1](#i1) | **present**: Get started and See how it works hide their exact destinations. | Name setup instructions, the example or the guide the link opens. |
| T55. Calling difficult tasks simple | [D1](#d1) | **present**: workflow.mdx says users can simply describe a direct fix. | Remove simply; keep the useful instruction. |
| T56. Inflated significance | [W1](#w1) | **not observed**: No revolutionary or historic claims. | Make claims proportional to observed product behavior. |
| T57. Citation-shaped invented proof | [W1](#w1) | **not observed**: Demonstrations have local source/provenance records; corpus citations are checked at originals. | Cite the original evidence and disclose the sample or demonstrated scope. |
| T58. Chat-process residue | [W1](#w1) | **not observed**: No as-an-AI or authoring-process comments in public copy. | Write for the product's reader; keep research notes in evidence records. |

## Documentation

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T59. Tool inventory before user tasks | [D3](#d3), [A1](#a1) | **improved in first rewrite**: The new workflow guide follows research, planning, execution, review and continuation. | Keep technical names only where readers need to invoke a command. |
| T60. Tutorial and reference mixed together | [D3](#d3) | **review**: The new guide is a workflow overview; installation and concept explanations remain separate. | Keep task instructions focused and link longer reference material. |
| T61. Commands without context or result | [D2](#d2) | **review**: Install includes a version check; loop examples need their stopping and handoff behavior. | State where to run a command and what should happen. |
| T62. Recovery omitted from instructions | [D2](#d2), [I1](#i1) | **not observed in rewritten setup**: Doctor, repair, upgrade conflicts and removal are described. | Keep recovery close to the relevant task and consistent with the owner in code. |
| T63. Explaining self-evident controls | [D3](#d3), [P3](#p3) | **review**: Some desktop shortcut descriptions are useful; repeated headings and label explanations are not. | Keep detail that changes what a reader can successfully do. |
| T64. Inconsistent names for one thing | [I1](#i1), [A1](#a1) | **review**: Saved work, history, session and conversation need distinct meanings. | Use one term per concept and explain the difference between project records and chats. |
| T65. Same sentence opener everywhere | [D1](#d1) | **review**: What Magistr remembers repeats you can and ask openings. | Vary sentence construction by meaning while preserving direct instructions. |
| T66. Exhaustiveness instead of usefulness | [D3](#d3) | **review**: Avoid pushing contributor architecture and every MCP operation into the public getting-started path. | Cover the reader's task, then link detailed references. |

## Review

| ID and pattern | Sources | Magistr assessment before the second iteration | Correction or legitimate use |
| --- | --- | --- | --- |
| T67. New template sold as anti-template | [A1](#a1), [P2](#p2), [P3](#p3) | **present**: The first iteration substituted a restrained technical/editorial look for the familiar glossy SaaS look. | Judge actual information, tasks and brand fit. Do not ban fonts, colors or punctuation by identity. |

## How to use the corpus

For a suspected pattern, identify the actual instance and the reader's task. Explain what the treatment contributes. Keep a technique when it helps comprehension, operation or a deliberate product identity; remove or revise it when it supplies ceremony, repetition or unsupported persuasion.

Do not convert this corpus into an AI-authorship detector, a numerical taste score, or an automatic ban on particular fonts, colors, punctuation or component libraries. A replacement template can fail for the same reason as the original.

For Magistr, the next iteration should make the product concrete immediately, put real recorded sessions close to that explanation, remove repeated slogans and decorative labels, flatten the walkthrough, and keep the existing functional checks. Original screenshots, transcript quotes and their limits must remain accurate. New screenshots must document the changed composition; the first iteration's captures must remain intact.

## Source register

No third-party skills were installed or followed as instructions. Their content was inspected as research material only.

### A1

[Anthropic frontend design guidance](https://github.com/anthropics/claude-code/blob/main/plugins/frontend-design/skills/frontend-design/SKILL.md) — Anthropic; vendor guidance. Publication date not established; mutable page. Retrieved 2026-10-20.

Current guidance names newer editorial and technical defaults. Mutable main branch; retrieval date identifies this reading.

### A2

[Improving frontend design through Skills](https://claude.com/resources/articles/improving-frontend-design-through-skills) — Anthropic; vendor guidance. Publication/update: 2025-11-12. Retrieved 2026-10-20.

Earlier vendor examples identify generic font/gradient defaults; some recommended alternatives have since become defaults themselves.

### K1

[Scoring Show HN submissions for AI design patterns](https://www.adriankrebs.ch/blog/design-slop/) — Krebs; observational scan. Publication/update: 2026-04-20. Retrieved 2026-10-20.

Reports 1,590 submissions: 347 with four or more markers, 508 with two or three, 735 with zero or one. CSS markers do not establish authorship.

### K2

[slop-detect](https://github.com/ravidsrk/slop-detect) — Krebs; derived checklist. Publication date not established; mutable page. Retrieved 2026-10-20.

Explicitly derives from Krebs. Its 67% summary differs from the current original report; do not repeat it or count it as independent corroboration.

### P1

[Design Slop Symptom Catalog](https://github.com/wpgaurav/design-slop/blob/main/references/slop-patterns.md) — Gaurav; practitioner catalog. Publication date not established; mutable page. Retrieved 2026-10-20.

Broad critique covering composition, content and behavior. Not a prevalence study; categorical thresholds are not adopted.

### P2

[antislop](https://github.com/miqdadbadjuber/anti-slop/blob/main/antislop.md) — Badjuber; practitioner catalog. Publication date not established; mutable page. Retrieved 2026-10-20.

Useful pattern inventory; its installation instructions and absolute punctuation bans are not adopted.

### P3

[Anti-UI-Slop](https://github.com/tuanphung2005/anti-ui-slop) — Phung; practitioner catalog. Publication date not established; mutable page. Retrieved 2026-10-20.

Includes fake editorial sophistication and excess explanation. Its universal bans on colors, fonts and geometry are treated as opinion.

### I1

[What makes a website look AI-generated?](https://blog.interfacekit.io/what-makes-a-website-look-ai-generated) — InterfaceKit; practitioner article. Publication/update: 2026-09-07. Retrieved 2026-10-20.

Vendor-authored critique updated September 7. Useful product-specific tests; not independent measurement of AI authorship.

### W1

[Wikipedia:Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) — Wikipedia editors; editor observation catalog. Publication date not established; mutable page. Retrieved 2026-10-20.

Examples come from encyclopedia editing. Stylistic recurrence is not reliable authorship detection; transfer to product copy requires judgment.

### D1

[Google: Voice and tone](https://developers.google.com/style/tone) — Google; documentation guidance. Publication/update: 2026-05-27. Retrieved 2026-10-20.

Primary writing guidance, not evidence of AI-specific prevalence.

### D2

[Google: Procedures](https://developers.google.com/style/procedures) — Google; documentation guidance. Publication date not established; mutable page. Retrieved 2026-10-20.

Primary procedural guidance for meaningful instructions, results and context.

### D3

[Diataxis: How-to guides](https://diataxis.fr/how-to-guides/) — Procida; documentation guidance. Publication date not established; mutable page. Retrieved 2026-10-20.

Organizes guidance around a person's task. Not an AI-detection resource.

### R1

[Interrogating Design Homogenization in Web Vibe Coding](https://arxiv.org/html/2603.13036v1) — Shin et al.; research framework. Publication/update: 2026-03-13. Retrieved 2026-10-20.

Literature review, tool walkthroughs and risk analysis. It does not measure the population frequency of individual visual tropes.

### R2

[Usable but Conventional](https://arxiv.org/html/2605.15124v1) — Romero et al.; empirical study. Publication/update: 2026-05-14. Retrieved 2026-10-20.

92 students judged ten static prototypes. Human examples came from one researcher. Authorship did not determine perceived quality; this is not an interactive usability test.

### U1

[Concise, Scannable, and Objective](https://www.nngroup.com/articles/concise-scannable-and-objective-how-to-write-for-the-web/) — Nielsen Norman Group; empirical usability research. Publication/update: 1997-01-01. Retrieved 2026-10-20.

Early web-writing study supports useful editing practices; it predates generative AI and supplies no AI prevalence claim.

### U2

[WebAIM Million 2026](https://webaim.org/projects/million/) — WebAIM; observational accessibility scan. Publication/update: 2026-02. Retrieved 2026-10-20.

One million homepages; 83.9% had detected low-contrast text. General web sample, not AI-created sites; automated absence of errors is not conformance.

### U3

[WCAG 2.2: Contrast minimum](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) — W3C; standards explanation. Publication date not established; mutable page. Retrieved 2026-10-20.

Contrast threshold guidance. Accessibility requirements are separate from aesthetic judgments.
## Changes made after this audit

The tables preserve the first rewrite as the audit baseline. The subsequent [website revision and per-frame review](desktop-iterations/2026-10-20-website-corpus-revision/REVIEW.md) apply these findings:

- A concrete, single-color heading names Codex and Claude Code. Heading size and tracking are reduced, and browser text flow replaces forced line breaks.
- Recorded product sessions follow the introduction. The decorative hero process diagram, duplicate agent strip, numbered section eyebrows and repeated footer pitch are removed.
- The real task walkthrough keeps its useful tab interaction, but loses numbered tab labels, decorative arrows, heading preambles and nested border frames. The before/after test comparison retains its meaningful directional arrow.
- The examples describe research, roadmap revision, bounded implementation and review, following the owner's use. The new guide explains these tasks before listing shortcuts.
- Body-size labels replace tiny uppercase monospace metadata; monospace remains for code. Shared docs headings and code samples are more readable, with fewer section rules and less page padding.
- The social preview uses the same concrete description. A follow-up [404 and search review](desktop-iterations/2026-10-20-website-corpus-recovery/REVIEW.md) catches the secondary-route preamble and internal-link arrows missed in the first correction.

The palette, useful active-state accents, genuine procedure numbers, real screenshots and keyboard behavior are retained. Replacing those automatically would reproduce the blanket-rule problem in T67. These are reviewed changes, not a measured “slop score” reduction or a claim that every remaining design choice has owner approval.
