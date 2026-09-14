# Public release readiness audit

**Audit date:** 2026-09-14  
**Scope:** the v0.1.0 source release, host package, documentation website, public links, contributor intake, and the visual explanation of Magistr's product model.

This audit is intentionally written for a project that is still in development. The question is whether a new person can reach the project, understand what it does today, install it without hidden local state, and tell what is experimental.

## Decision

Magistr is **technically ready for a source-first development release**, but it is **not publicly ready to announce yet**.

The Rust workspace, native quality gates, website build, and host packaging all pass in this checkout. The release is blocked by public reachability and a clean-checkout website asset defect. Several follow-ups would make the public experience trustworthy: automated release checks, contributor and security intake, a single product message, and diagrams that explain ownership and durable evidence in addition to the existing loop.

The most urgent distinction is:

- **Build readiness:** green in this checkout.
- **Public availability:** unverified or unavailable from an anonymous environment.
- **Public comprehension:** improving, but still missing the system boundary and evidence model.

## Evidence at a glance

| Area | Status | Evidence | Release implication |
| --- | --- | --- | --- |
| Rust source build | Ready | cargo package --locked --list -p magistr-cli succeeds; Cargo.lock is tracked | A source install is reproducible from the repository |
| Native quality gates | Ready | The configured full gate run passed: check, nextest, clippy, and all required custom gates | The current commit has strong local verification |
| Host archive | Ready for this host | scripts/package-release.sh produced magistr-0.1.0-aarch64-apple-darwin.tar.gz and .sha256; checksum verification passed | The packaging path works, but only the current host was exercised |
| CLI smoke | Ready | Packaged binary reports magistr 0.1.0 and renders help | The archive contains a usable CLI |
| Website build | Ready in this checkout | npm test in apps/website passed, including static routes, docs, metadata, favicons, contrast, and output assertions | Build is healthy when local ignored assets are present |
| Public repository links | Blocked | Anonymous requests to the configured repository, releases, and issues URLs returned HTTP 404 during this audit | Clone, issue, release, and source links are currently dead to a new visitor |
| Website hostname | Unverified | magistr.dev did not resolve from this audit environment | Do not announce the site until DNS and deployment are verified externally |
| Clean website assets | Blocked | apps/website/public/logo.png is referenced by the hero, ignored by .gitignore, and absent from a git archive HEAD checkout | A clean clone can build while serving a broken hero logo |
| CI and release automation | Gap | No tracked .github workflow files were found | Gates and packages depend on a maintainer's local machine |
| Crates.io publication | Deliberate source-only boundary or gap | cargo publish --dry-run --locked -p magistr-cli fails because local path dependencies have no publish version requirements | Do not imply a crates.io install until a publication plan exists |
| Contributor intake | Gap | No tracked CONTRIBUTING, CODE_OF_CONDUCT, or SECURITY policy file was found | Public feedback and vulnerability reporting have no canonical path |
| License and notices | Ready | MIT LICENSE; host package includes MIT and bundled font notices | Distribution notices are present |
| Experimental boundaries | Documented | Release inventory names gui2, peer protocol, live-provider prerequisites, and disabled roadmap exports | Keep these limits visible in the first-run docs |
| Product message | Needs alignment | README leads with roadmap/workflow management; website and package metadata lead with an AI terminal toolkit | New users receive two different explanations of the product |

The HTTP and DNS observations are environment-scoped checks, not a claim about the intended deployment. Run the same checks from an anonymous network after the repository and site are published.

## What is already strong

The release inventory is a useful source of truth. It names the shipped CLI, terminal cockpit, MCP server, native workspace tools, agent integrations, API adapters, package contents, prerequisites, and experimental surfaces. The README now gives a first-time reader an honest v0.1.0 development warning, a source install, project setup, the Codex and Claude paths, and the $next / $next-loop / $validate workflow.

The implementation has a coherent product model:

1. A person and a project supply intent.
2. Native roadmap state keeps goals, priorities, dependencies, acceptance, and ready work.
3. A selected item becomes one recorded workflow iteration.
4. The agent uses project context and the native workspace tools to make changes.
5. Configured checks produce evidence attached to the work.
6. The result is committed as one scoped change and the roadmap is ready for the next decision.

The package script also does the practical things a development release needs on the current host: builds with --locked, includes shell completions, a generated man page, the MIT license, bundled font notices, and a SHA-256 sidecar.

## Release blockers and follow-ups

### Blocker: make the public endpoints real

Every public-facing surface currently points at https://github.com/AlrikOlson/magistr. The README, website, installation docs, release inventory, issue link, changelog fallback, JSON-LD download URL, and Cargo metadata all depend on that location. Anonymous checks during this audit returned 404 for the repository, releases, and issues pages.

Before announcing the development release:

1. Make the canonical repository public at the configured URL, or change every reference to the real canonical location.
2. Deploy the website and publish DNS for the configured hostname.
3. Check the repository, releases, issues, main branch, license, release inventory, and installation docs without authentication.
4. Check the website, every docs route, robots.txt, sitemap, favicon, OG image, GitHub links, release links, and issue links from a clean browser session.
5. Record this as a release gate so a future rename cannot silently break the public surface.

### Blocker: remove the clean-checkout logo dependency

The website hero references /logo.png. The file exists in the current working tree but is ignored by the root *.png rule and is absent from git archive HEAD. The website build does not fail when a static image is missing, so the current test suite can pass while a clean checkout serves a broken hero.

Choose one source of truth:

- track a deliberately reviewed logo asset and narrow the PNG ignore rule, or
- replace the hero reference with the tracked favicon.svg or a tracked SVG logo.

Then add a clean-archive build or static-reference check that fails when any public asset referenced by Astro is missing.

The same rule applies to future visual assets. The checked-in README PNGs are intentionally force-added today; a narrow exception makes that policy durable for the next contributor.

### High: add an automated public-release lane

There are no tracked .github workflows. Add a small matrix that:

- runs the configured Rust gates;
- runs npm test under apps/website;
- builds and smoke-tests each supported host archive;
- verifies each SHA-256 sidecar and archive contents;
- builds the website from a clean checkout;
- checks anonymous links after deployment;
- publishes release notes that match the exact tag and asset names.

The current shell script is a good local primitive. The missing piece is repeatable execution on every named target.

### High: state the distribution boundary explicitly

The source install is real and tested. The host archive path is real and tested for the current machine. A crates.io publication is not currently real: cargo publish --dry-run --locked -p magistr-cli stops because local dependencies such as magistr-acp do not specify publish versions.

Pick one of these explicit positions:

- **Source and host archives only for v0.1.0:** say so in the README and website, and remove any future-looking package implication.
- **Publish to crates.io:** add version requirements for publishable local dependencies, test the package in a clean consumer project, and document the resulting install command.

Do not add a registry command until the second path is complete.

### High: add public contribution and security intake

A public development release needs three small policy pages:

- CONTRIBUTING.md: setup, focused changes, required gates, and how to propose a roadmap/workflow change.
- SECURITY.md: how to report a vulnerability privately, what local data may contain, and the support window for an early release.
- CODE_OF_CONDUCT.md: the community standard and contact path.

Link them from the README and website Community section. The existing development guide is an excellent maintainer document, but it does not replace these public entry points.

### Medium: bring the product message into one sentence

The root README says “Roadmap and workflow management for coding agents.” The website hero, manifest, JSON-LD, and Cargo description say “AI-powered dev toolkit for the terminal.” Both are true, but the second hides the product's strongest differentiator.

Use one lead sentence everywhere:

> **Magistr turns a project's roadmap into small, recorded, verifiable work for coding agents.**

Then explain the terminal cockpit, MCP/workspace tools, and provider adapters as the supporting surfaces. Keep the early-development warning beside that sentence.

### Medium: move the safety and privacy boundary into the first-run path

The development guide accurately says that agent processes keep their normal operating-system permissions, provider adapters may send prompts and history to the configured provider, child processes inherit environment settings, and local records may contain prompts, paths, and outputs. The root README mentions the lack of a sandbox, but the website and installation path do not give this boundary equal prominence.

Add a short “Before you run an agent” callout beside installation:

- use a disposable or reviewed checkout;
- provider credentials and model billing remain the caller's responsibility;
- permission prompts are workflow guidance, not an OS sandbox;
- local records can contain project paths, prompts, and outputs.

Link to the detailed development and authentication references.

### Medium: make release claims time-aware

The installation docs currently say that prebuilt archives “are published,” while the changelog fallback says no published GitHub release is available yet. Until the public release page exists, call the archives “produced by the package script” or “published with the development release.” After publication, have the release inventory and changelog read from the same tag and asset names.

## A better visual language for Magistr

The current hero diagram correctly shows a four-stage loop and the three icons establish a consistent flat 2D vocabulary. Its limitation is structural: it explains sequence, but not ownership, storage, or proof. Adding more decorative icons would repeat the same answer.

Use three views, each answering one question.

### View 1: the decision loop

**Question:** “How does an idea become the next safe change?”

~~~mermaid
flowchart LR
    I["Intent<br/>you + project"] --> R["Roadmap<br/>goals · priorities · dependencies"]
    R --> C["Ready chunk<br/>acceptance + context"]
    C --> W["Recorded workflow<br/>research → plan → execute → verify"]
    W --> E["Evidence + commit<br/>checks attached to the change"]
    E --> R
~~~

Place this in the README hero or immediately after the opening paragraph. Keep the existing Blender-rendered overview as the visual companion, but make its caption say what the loop preserves: roadmap context, one focused change, and attached evidence.

### View 2: the ownership boundary

**Question:** “Where does each part of the system run?”

~~~mermaid
flowchart TB
    Person["You<br/>choose goals · review changes"] --> Agent["Agent runtime<br/>Codex · Claude Code · Copilot"]
    Agent --> MCP["Magistr MCP<br/>roadmap + workflow tools"]
    Agent --> WB["Workbench bridge<br/>search · read · edit · run"]
    MCP --> Store[("Local project state<br/>roadmap · plans · receipts")]
    WB --> Project["Your checkout<br/>source files + configured commands"]
    Agent --> Provider["Configured provider<br/>runtime or API credentials"]
    Provider -. "prompts / responses<br/>per provider config" .-> Agent
~~~

This is the most important missing diagram for a public reader. Draw it as a labeled swimlane or layered map, not as a generic AI architecture picture. Annotate the boundary in plain language: Magistr is local; it does not host a model or a hosted project service; roadmap_exports=false keeps native roadmap state in the project store; permission prompts guide actions but do not create an OS sandbox.

Place it on the website and link it from the README. Keep crate-level ownership details in docs/architecture/overview.md.

### View 3: the evidence chain

**Question:** “Why can I trust what the agent says it did?”

~~~mermaid
flowchart LR
    A["Roadmap decision"] --> B["Ship objective"]
    B --> C["Focused task plan"]
    C --> D["Changed files"]
    D --> G["Configured gates"]
    G --> Q["Check receipts"]
    Q --> K["Atomic commit"]
    K --> N["Next ready item"]
~~~

This diagram should sit beside the verification section. It gives “recorded workflow” a concrete meaning and explains why a new session can resume without reconstructing the whole chat. A flat 2D asset can show the same chain as a paper trail with labeled cards; the Mermaid source must remain the accessible explanation.

### Optional fourth view: setup path

Use this only on the website or installation guide, where the reader is ready for commands:

~~~mermaid
flowchart LR
    A["Install Magistr + agent runtime"] --> B["magistr init ."]
    B --> C["Agent config + MCP + project.json"]
    C --> D["Create or review roadmap"]
    D --> E["$next / $next-loop"]
    E --> F["$validate or recorded verification"]
~~~

This is a task sequence, not the product architecture. Keep it separate from the three explanatory views so “how to start” does not replace “how it works.”

## Visual rules

- Use a flat 2D treatment with orthographic geometry, clear labels, and no 3D metaphors.
- Preserve the existing accents: cyan for roadmap, orange for planning/action, violet for agent/session, and green for verification/evidence.
- Give every node a noun and a verb-sized caption. Avoid icons that require guessing.
- Keep Mermaid or text equivalents next to every artwork. Never make color the only encoding.
- Use the same labels in the README, website, and architecture docs so a reader can move between levels without translating vocabulary.
- Mark experimental surfaces with a small “experimental” label rather than giving them equal visual weight with the supported terminal path.
- Keep motion optional and honor the existing reduced-motion behavior on the website.

## Recommended implementation order

1. Fix the public repository/site endpoints and the clean-checkout logo.
2. Add CI and the public policy pages.
3. Unify the lead sentence and move the safety/privacy callout into installation.
4. Add the ownership and evidence diagrams, keeping Mermaid sources beside the flat 2D renders.
5. Update website and README tests to check every referenced asset and every canonical link from a clean archive.
6. Re-run the release inventory against the actual public tag and package assets.

## Definition of “ready to announce”

A stranger can anonymously clone or download the project, read the license and release notes, reach the issues and security path, follow one install path from a clean checkout, run magistr --version, initialize a disposable project, identify the next roadmap item, and understand what is recorded and what remains their responsibility. The README, website, Cargo metadata, release inventory, and package contents all name the same v0.1 development boundary.

