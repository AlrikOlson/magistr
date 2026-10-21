# Whole-project website: research and decisions

Date: October 20, 2026 (America/Los_Angeles).

The owner accepted a project-building story, an explorable overview alongside the full build recording, a creator voice on the landing page, and practical guides written from one builder to another. Delivery is a commit and direct push to main, followed by publishing the existing magistr.dev website.

## What the product and usage support

The accessible Codex history and Claude Code project logs support a recurring workflow: inspect an existing project, question its architecture, research decisions, revise the roadmap, run bounded chunks, inspect the actual result, and return in later sessions. This is a synthesis of accessible history, not a claim to have recovered every conversation. No raw private session history is published.

Native owners in `crates/magistr-engine/src/native/native_engines.rs` delegate roadmap, reasoning, work and feedback to their owning engines. The website reads dated captured examples; it does not become a second writable project store. The public roadmap excerpt includes completed foundations and the still-planned complete desktop planning/evidence flow. The page must keep that distinction visible.

The original support-inbox recording contains real tasks, edits, command results, reviews and failures. It does not contain a roadmap mutation or branching reasoning. Those features therefore use clearly named, separately captured real Magistr records. The existing inbox recording, source archive, video and original captures remain unchanged.

Native reasoning steps 1739–1742 record the website decision: compare leading with the full recording against leading with explorable project records, merge both alternatives into a pinned conclusion, and keep the overview first with the recording accessible below. The public excerpt excludes local working-directory and timing internals; it preserves the selected source records and relationships.

## Primary-source research

- [Linear project dependencies](https://linear.app/docs/project-dependencies) and [project milestones](https://linear.app/docs/project-milestones): meaningful dependency relationships and readable completion groupings. Applied selected real edges with attached endpoints, explicit statuses, and inspectable requirements. No invented schedule or timeline.
- [LangSmith trajectories and tracing](https://www.langchain.com/blog/langsmith-trajectories-tracing), published September 24, 2026: a readable overview can coexist with deeper execution records. Applied a four-part product overview and links into the full original build recording. This is an inference for Magistr's website, not a claim that Magistr has LangSmith's interface or features.
- [Mailchimp voice and tone](https://styleguide.mailchimp.com/voice-and-tone/): familiar, direct language and warmth that serves the reader. Applied specific actions, ordinary examples and a brief creator statement, without exaggerated outcomes or forced humor.
- The [earlier recorded-walkthrough research](../2026-10-20-walkthrough-research/RESEARCH.md) remains relevant to the unchanged chapter player and actual video.

## Presentation decisions

The feature explorer shows the roadmap, saved reasoning, completed work and actual checks. Native SVG relationships and semantic HTML controls fit these small graphs; a canvas or Three.js scene would not improve the visitor's task. All four explanations remain present without JavaScript. Selection uses real buttons and tabs with keyboard navigation. Details expose the exact downloadable records and the original recording rather than fabricating a live application.

The page keeps its single-color heading, restrained type and surfaces. Accent color identifies links, actions and selection. There are no gradient headings, glowing decoration, invented metrics, fake testimonials, repetitive benefit tiles or simulated progress.

The seven guides are organized around installation, the working loop, planning, reasoning, chunks, verification, and continuing across sessions. README and the repository getting-started guide use the same story. Technical contributor architecture/reference material remains owned by its existing implementation documentation.

## Review and delivery evidence

The first complete iteration retained 111 captures and failed visual review on the oversized introduction, a copyable-command line break and a compact caption. A bounded correction retained 25 captures; the compact opening still needed shortening. Both failed reviews remain intact. The final hero correction has its own source-bound captures and review. Browser passes and actual image review are separate records. Verification stays focused on the website, original demo integrity, links, screenshots, and deployed output; unrelated Rust/native suites are not required for this change.
