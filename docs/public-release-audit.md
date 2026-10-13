# Public release readiness audit

Historical readiness assessment, not a release declaration. Magistr remains
unreleased; the owner has not declared an official public release. The
[pre-release policy](../AGENTS.md#before-public-release) supersedes any inferred
compatibility obligation from version numbers or release wording in this audit.
This assessment predates the desktop cutover. Use the current [desktop guide](architecture/desktop.md)
and [release inventory](release-inventory.md) for interface and packaging requirements;
the checks below describe the audit date rather than the current commit.

**Audit date:** 2026-09-14  
**Scope:** the v0.1.0 source release, host package, documentation website, public links, contributor intake, and the visual explanation of Magistr's product model.

This audit is intentionally written for a project that is still in development. The question is whether a new person can reach the project, understand what it does today, install it without hidden local state, and tell what is experimental.

## Decision

Magistr is **technically ready for a source-first development release**, but it is **not publicly ready to announce yet**.

The Rust workspace, native quality gates, website build, host packaging, public policy entry points, release workflow, and diagram system now pass locally. The remaining blocker is external: the configured GitHub repository, release, issue, and magistr.dev URLs are not anonymously reachable from this audit environment. The closure work records that dependency as an explicit release gate with an owner and rerun command.

The most urgent distinction is:

- **Build readiness:** green in this checkout.
- **Public availability:** unverified or unavailable from an anonymous environment.
- **Public comprehension:** covered in source by the roadmap/workflow, ownership-boundary, and evidence-chain views.

## Evidence at a glance

| Area | Status | Evidence | Release implication |
| --- | --- | --- | --- |
| Rust source build | Ready | cargo package --locked --list -p magistr-cli succeeds; Cargo.lock is tracked | A source install is reproducible from the repository |
| Native quality gates | Ready | The configured full gate run passed: check, nextest, clippy, and all required custom gates | The current commit has strong local verification |
| Host archive | Ready for this host | scripts/package-release.sh produced magistr-0.1.0-aarch64-apple-darwin.tar.gz and .sha256; checksum verification passed | The packaging path works, but only the current host was exercised |
| CLI smoke | Ready | Packaged binary reports magistr 0.1.0 and renders help | The archive contains a usable CLI |
| Website build | Ready in a clean checkout | npm test in apps/website passed, including static routes, docs, metadata, favicons, contrast, diagram assets, and output assertions | Build no longer depends on ignored local assets |
| Public repository links | Blocked | Anonymous requests to the configured repository, releases, and issues URLs returned HTTP 404 during this audit | Clone, issue, release, and source links are currently dead to a new visitor |
| Website hostname | Unverified | magistr.dev did not resolve from this audit environment | Do not announce the site until DNS and deployment are verified externally |
| Clean website assets | Ready | Hero uses tracked `favicon.svg`; CI asserts tracked diagram assets and fails on ignored public paths | A clean clone has a complete static asset set |
| CI and release automation | Ready in source | `.github/workflows/public-release.yml` runs Rust, website, asset, package, checksum, and advisory link jobs | External hosting still must run the workflow after repository publication |
| Crates.io publication | Deliberate source-only boundary or gap | cargo publish --dry-run --locked -p magistr-cli fails because local path dependencies have no publish version requirements | Do not imply a crates.io install until a publication plan exists |
| Contributor intake | Ready | `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and `SECURITY.md` are linked from README and website | New contributors and reporters have canonical entry points |
| License and notices | Ready | MIT LICENSE; host package includes MIT and bundled font notices | Distribution notices are present |
| Experimental boundaries | Documented | Release inventory names gui2, peer protocol, live-provider prerequisites, and disabled roadmap exports | Keep these limits visible in the first-run docs |
| Product message | Ready | README, website hero/meta, manifest, Cargo metadata, installation docs, and release inventory lead with roadmap/workflow management and source-first v0.1 | New users receive one explanation of the product |

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

### Resolved in closure: clean-checkout logo dependency

The baseline hero defect is resolved: the hero now references tracked `favicon.svg`, the broad PNG ignore has reviewed public exceptions, and CI checks the referenced assets from a clean checkout.

The implementation uses the tracked SVG path and `.github/workflows/public-release.yml` asserts each reviewed public asset with `git ls-files`, `git check-ignore --no-index`, and a source scan for `/logo.png`. The same rule applies to future visual assets.

### Resolved in closure: automated public-release lane

`.github/workflows/public-release.yml` now runs Rust formatting/check/test/clippy, the website test, clean-checkout asset assertions, and an Ubuntu/macOS package/checksum matrix. `scripts/public-link-check.sh` probes the configured URLs; its strict failure remains the external release gate until the repository and DNS are live.

### Resolved in closure: explicit distribution boundary

The source install is real and tested. The host archive path is real and tested for the current machine. Crates.io publication is intentionally not claimed: `cargo publish --dry-run --locked -p magistr-cli` fails because local path dependencies such as `magistr-acp` do not specify publish versions. README and website installation surfaces say v0.1 is source-first and point to the package script and release inventory for host archives. No registry install command is shown.

### Resolved in closure: public contribution and security intake

A public development release needs three small policy pages:

- CONTRIBUTING.md: setup, focused changes, required gates, and how to propose a roadmap/workflow change.
- SECURITY.md: how to report a vulnerability privately, what local data may contain, and the support window for an early release.
- CODE_OF_CONDUCT.md: the community standard and contact path.

Link them from the README and website Community section. The existing development guide is an excellent maintainer document, but it does not replace these public entry points.

### Resolved in closure: one product message

The README, website hero/meta, manifest, Cargo metadata, installation docs, and release inventory now lead with “Roadmap and workflow management for coding agents.”

The canonical lead sentence is:

> **Roadmap and workflow management for coding agents.**

Then explain the terminal cockpit, MCP/workspace tools, and provider adapters as the supporting surfaces. Keep the early-development warning beside that sentence.

### Resolved in closure: safety and privacy boundary in first-run docs

The README and installation page now place the permission and privacy boundary beside the source install. The detailed development guide remains the deeper reference for provider data, child-process inheritance, and local records.

The callout names local workflow state, provider credentials, normal operating-system permissions, and the need to review generated files and commands.

### Resolved in closure: time-aware release claims

The installation docs now describe v0.1 as source-first and point to the package script/release inventory for host archives. The anonymous URL gate still owns the final “published” claim until the repository, releases, issues, and website are reachable.

## A better visual language for Magistr

The public surface now uses four hand-authored SVG views with the same visual vocabulary and literal labels. Together they answer how to steer the roadmap, which loop owns the work, what one iteration does, and what remains when you return.

Use four views, each answering one question.

### View 1: conversation-driven roadmap control

**Question:** “How does an idea become the next safe change?”

The conversation view shows a user describing an outcome, an agent drafting a roadmap change, the user reviewing that proposal, and one ready chunk entering the queue. It makes the steering boundary clear: the user chooses the direction and applies roadmap changes.

### View 2: harness and TUI loop ownership

**Question:** “What changes when I use Codex or Claude instead of the TUI?”

The modes view separates the agent-harness path from the native cockpit. Codex and Claude run skills one chunk per session; `$next-loop` writes a handoff and requires a fresh agent session before the next chunk. `magistr next-loop` is the host-owned TUI loop: it starts a fresh provider session per iteration, persists a checkpoint, and can recover it. Both paths use the same native project state. Crate-level ownership details remain in docs/architecture/overview.md.

### View 3: one bounded iteration

**Question:** “What does one next chunk include?”

The iteration view follows a ready chunk through selection, preparation, workspace changes, verification, and recorded output. The boundary is concrete: one chunk, its configured checks, and its artifacts.

### View 4: resumable native state

**Question:** “What will still be there when I return?”

The state view connects the project checkout, roadmap/reasoning/ship records, and agent/workspace clients through the local service. The README describes the records in plain language so the image is not required to understand the model.

## Visual rules

- Use a flat 2D treatment with orthographic geometry, clear labels, and no 3D metaphors.
- Preserve the existing accents: cyan for roadmap, orange for planning/action, violet for agent/session, and green for verification/evidence.
- Give every node a noun and a verb-sized caption. Avoid icons that require guessing.
- Keep literal labels and text alt descriptions next to every artwork. Never make color the only encoding.
- Use the same labels in the README, website, and architecture docs so a reader can move between levels without translating vocabulary.
- Mark experimental surfaces with a small “experimental” label rather than giving them equal visual weight with the supported terminal path.
- Keep motion optional and honor the existing reduced-motion behavior on the website.

## Closure status

The local implementation order is complete: tracked assets, canonical copy, policy pages, installation boundaries, hand-authored SVG diagrams, website assertions, CI/package checks, and the anonymous URL probe are all in source. The only remaining release gate is external endpoint availability; the maintainer or hosting operator must make the repository, releases, issues, and magistr.dev reachable and rerun `PUBLIC_RELEASE_STRICT=1 ./scripts/public-link-check.sh`.

## Definition of “ready to announce”

A stranger can anonymously clone or download the project, read the license and release notes, reach the issues and security path, follow one install path from a clean checkout, run magistr --version, initialize a disposable project, identify the next roadmap item, and understand what is recorded and what remains their responsibility. The README, website, Cargo metadata, release inventory, and package contents all name the same v0.1 development boundary.

