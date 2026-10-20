# Magistr website: direction and evidence

Research date: 20 October 2026. Baseline: b2377a4e. PR #3 remained open and all five checks passed when work began. Existing worktree was clean. The previous native website objective was complete; this work uses a separate focused objective.

## Direction

**Plan the work. Prove the change.** Position Magistr as **the workflow layer for coding agents**, aimed at developers already using Codex or Claude Code. Planning, source changes and recorded checks form the primary story; continuity is the consequence of keeping that work, not the entire pitch. The first screen must identify the product, supported agents, benefit, and source-preview installation path.

Graphite and porcelain surfaces, warm orange emphasis, Instrument Sans and IBM Plex Mono, a strong editorial grid, and a single ordered work record replace the cream/purple gallery. The workflow demonstration uses the Contact list normalization defect throughout. Large, readable explanations accompany inspectable, unmodified evidence. No animation is required to understand the sequence. This is design judgment, not measured user research.

## Current category research

| Official source | Category and observed presentation | Decision for Magistr |
| --- | --- | --- |
| [Factory](https://factory.com/) | Enterprise agent platform. Oversized monochrome proposition, product film, enterprise logos, broad capability catalogue and separate start/sales routes. Its product scope covers autonomous planning through delivery. | Borrow confident hierarchy and the connection from plan to validation. Do not borrow enterprise authority, customer logos or self-improvement claims. |
| [Conductor](https://www.conductor.build/) | Agent workspace with cloud sandboxes. Concrete first-screen category and download action; a large working application capture; collaboration and environment demonstrations. Mobile preserves the message and CTA but the full application image becomes tiny. | State how Magistr relates to existing agents immediately. Keep useful text beside captures and offer originals rather than depending on scaled-down screenshots. |
| [Amp](https://ampcode.com/) | Coding agent and development environment. Distinctive typographic identity, a clear start action, and a chaptered day-to-day product video with a transcript. The demonstration includes review and proof, not just code generation. | One coherent worked example is more persuasive than a gallery. Use real source/test evidence at readable size; avoid borrowing its remote execution promises or ornamental effects. |
| [LangChain Deep Agents](https://docs.langchain.com/oss/python/deepagents/overview) | SDK/harness for building agents, backed by LangGraph's runtime. Documentation leads with capabilities, runnable code and explicit architectural categories. Responsive navigation preserves reading order. The embedded preview was still loading in the captured frame. | Do not imply Magistr is an SDK or owns the underlying agent loop. Explain the MCP connection simply; keep the static walkthrough useful even without JavaScript. |
| [Kiro](https://kiro.dev/) | Agent IDE/CLI/workspace; closer in its spec-to-task story. Large task/spec demonstrations connect requirements to implementation, with several installation routes. Mobile shows the cost of dense screenshots and multiple equally strong CTAs. | Show a testable goal, source change and checks as one story. Use one primary installation action and make agent selection a later step. |

All five sites were actually rendered in headless WebKit at 1440×1000 and 390×844. `captures.json` contains URLs, times, dimensions, renderer and image hashes. In-app browser discovery returned no connected browser; headless WebKit was the focused fallback. Initial/detailed frames for every site and the prior Magistr site were opened individually. This was a visual and content inspection, not a competitor usability study or a validation of their performance/customer claims. Videos were inspected as rendered demonstrations with the available text/transcripts; no full video playback claim.

## Grounded product claims

- `crates/magistr-workflow/src/roadmap/application/use_cases.rs`: roadmap writers and readers share durable state; completion belongs to the work/check authority and refuses direct status changes.
- `crates/magistr-workflow/src/ship/application/source_observation.rs` and `crates/magistr-core/src/evidence/snapshot_witness.rs`: checks identify source and configuration, with later drift detection. Passing a check is evidence of that check, not a guarantee that code is correct.
- `crates/magistr-workspace/schemas/tools/edit.input.schema.json`: edits use fresh hashes, reject newly introduced parse errors, and journal changes. These guarantees apply to Magistr's source tools, not arbitrary shell writes.
- `docs/architecture/overview.md`, `docs/reference/codex.md`, and the generated agent setup establish the CLI/MCP/agent relationship and saved project work. Desktop is a separate source preview, not the entire planning interface.
- `apps/website/public/screenshots/captures.json` remains the image authority. Its four originals were opened and checked against the earlier capture review.

## Demonstration truthfulness

The recorded Contact list task reproduced a whitespace test failure, changed only `names.py`, ran the unchanged `python3 -m unittest -q`, and saved a passing result (one test, exit 0). Codex's execution transcript records receipt `86489004-f53c-4412-990f-417280b0167c`. The photographed fresh Codex session reads that record. Claude Code independently performed the same task and read its own record; the images do not prove transfer of one task between agents.

The desktop screenshots show the same defect explained and a fix proposed in a reopened conversation. They do **not** show that the desktop executed the fix or test. The interactive explanation is labeled as such, and the actual application captures are separately identified with dates and original-image links. No new runtime success, adoption claim or benchmark is invented.

## Availability and verification contract

On this run, npm's `magistr` endpoint returned 404, Cargo's API with a descriptive user-agent and its sparse index returned 404, and the repository returned 404 anonymously. Keep the source preview and required repository access explicit. Version stays sourced from `npm/magistr/package.json` (0.1.0); initial target stays Apple Silicon macOS. No registry publication is authorized.

Use focused website build/content checks, WebKit behavior and reflow checks, link-checker tests, whitespace checks and a separate required GUI visual review. Save each iteration in a new dated directory; inspect full pages and readable detail, both themes, compact/desktop/320px, walkthrough tabs, capture selection, menus, focus and lazy-search recovery. No foreground/native input testing. Commit/push to PR #3 and deploy the reviewed build to the existing Cloudflare Pages project `magistr`, then compare deployed bytes.
