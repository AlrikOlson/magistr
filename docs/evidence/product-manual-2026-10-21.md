# Standalone product manual — editorial and factual review

Reviewed 2026-10-21. Scope: all 21 public manual pages, current repository entry points and contributor navigation.

## Result and method

**PASS.** Each page was read as a direct entry, without relying on the landing-page recording or another page's example. Review covered the subject and terminology, prerequisites, actionable instructions and expected results, locally introduced examples, implementation support, and relevant next links. Definitions and lookup pages were judged as references rather than forced into tutorials.

The website MDX collection owns user instructions. The documentation home, sidebar, breadcrumbs, related links and route checks share collection metadata. Search filters to the documentation collection. The seven existing guide URLs remain available.

## Page review

All rows pass the applicable editorial criteria above.

| Page | Independent-entry finding | Factual basis |
| --- | --- | --- |
| Documentation | Explains Magistr's role beside an agent, introduces saved records, and offers setup and topic entry points. Availability is stated locally. | Workflow/context owners; installation and desktop prerequisites. |
| Installation | Separates installing the executable from connecting a project. States repository access, preview platform and Rust requirements; explains version/help success and build failures. | Cargo manifests, CLI help, release inventory and source installation path. |
| Connect an agent | Starts in the intended project root. Separates Codex, Claude and portable MCP setup, names generated files and gives a read-only connection confirmation. | Init and launcher implementation; generated project skills; disposable setup check. |
| Your first task | Covers existing and empty projects, defines acceptance criteria, distinguishes direct requests from roadmap work, and names actual completion evidence. | Native objective/task lifecycle and next skill. |
| Concepts | Defines records before relating them. Separates project evidence, source, conversations and private model reasoning; explains source-tool limits. | Native roadmap, think, ship, source-tool and session contracts. |
| Plan a project | Defines a roadmap and chunk locally; the planning example asks for research and saved deliverables before implementation. Dependencies and priorities are distinct. | Roadmap commands and roadmap-refresh skill. |
| Reasoning and research | Explains deliberate saved reasoning, evidence, revisions and uncertainty. Its research prompt is self-contained and does not imply access to private model thinking. | Native reasoning operations and craft/roadmap-refresh skills. |
| Working in chunks | States setup prerequisites; distinguishes chat skills, a terminal launch and bounded fresh-session runs. Recovery is explained before new work. | CLI next/next-loop options and next/next-loop skills. |
| Verification | Explains executed checks, written reviews, relevant inputs, stale evidence and retries after diagnosis. Full verification is an explicit choice. | Quality-gate and verification types; check execution and freshness contracts. |
| Sessions and resuming work | Names what carries over, how to read unfinished work and how provider/directory-bound conversations differ from shared project records. | Execution-context identity and conversation resume owners. |
| Feedback | Defines a signal and gives a complete issue example. Investigation, deferral and promotion have distinct outcomes; recording feedback does not implement it. | Signals skill and native signal lifecycle. |
| Desktop preview | States source-build requirements, preview limits and meaningful controls. Current verification is distinguished from platform build support. | Desktop architecture, launcher, UI controls and release inventory. |
| Command reference | Establishes terminal/project context and groups commands by effect. Placeholders and destructive operations are identified. | CLI definitions and 36 documented command forms checked with help. |
| Skills | Defines chat skills and their host syntax, with the nine project skills and their effects. A bounded chat handoff is distinct from a terminal loop. | Current repository SKILL.md files and generated setup. |
| Configuration | Identifies writers and readers for settings. Explains launch precedence and check settings; the JSON example is explicitly a fragment requiring real scripts. | Project-input, native-config and verification types; disposable configuration validation. |
| Storage and backups | Lists actual stores and precedence. Backup instructions include closed owners and SQLite sidecars; moving a repository is not presented as a full data migration. | Workflow persistence, execution-context identity and conversation persistence. |
| Permissions | Separates host approval, Magistr launch profiles and source-tool declarations. Does not promise a universal sandbox or automatic permission transfer. | Agent launch configuration, workspace command authorization and host responsibilities. |
| Troubleshooting | Starts with diagnostics, then distinguishes connection, ownership, uncertain dispatch, check and storage problems. Does not recommend deleting data or guessing recovery outcomes. | Doctor, workspace doctor, resolve-dispatch and persistence error paths. |
| Upgrades | Separates updating the executable, refreshing setup and automatic conversion. Names recovery copies, safe retry behavior and conflicts requiring review. | Existing upgrade owners and old-to-current integration fixtures; disposable repair conversion. |
| Remove Magistr | Separates connection removal, executable uninstall and data erasure. States what remains and cautions against deleting a shared store for one project. | Remove/codex-remove/reset command definitions; disposable removal preserving project identity. |
| Contributing | Clearly addresses developers changing Magistr, states repository-access requirements, and routes to current architecture, integration, release and historical material. | Current repository paths and contributor link checks. |

## Ownership and source grounding

The following implementation owners were inspected when establishing the claims; they are not extra product instructions:

- [CLI arguments and command effects](../../crates/magistr-cli/src/cli.rs).
- [Project configuration inputs](../../crates/magistr-core/src/config/inputs/project_inputs.rs), [quality gates](../../crates/magistr-core/src/types/quality_gate.rs) and [verification settings](../../crates/magistr-core/src/types/verification_config.rs).
- [Native agent preferences](../../crates/magistr-engine/src/native/native_config.rs) and [execution identity](../../crates/magistr-engine/src/native/execution_context.rs).
- [Workflow storage](../../crates/magistr-workflow/src/infra/persistence.rs), [execution storage](../../crates/magistr-workflow/src/ship/persistence.rs) and [conversation resumption](../../crates/magistr-cli/src/agent/persistence/resume.rs).
- [Upgrade fixtures](../../crates/magistr-cli/tests/project_upgrade_integration.rs), [desktop contract](../architecture/desktop.md), [release inventory](../release-inventory.md) and the [nine repository skills](../../.agents/skills/).
- Pagefind's [filtering API](https://pagefind.app/docs/js-api-filtering/) supports the documentation-only search filter.

A real limitation was preserved: project identity shares roadmap records, while execution plans and conversations also depend on working-copy/provider identity. Copying a project folder does not migrate every saved record. A separate owning roadmap fix, `preserve-conversation-recovery`, records the destructive `sessions.db` deletion advice in an existing CLI error. The manual explains the data-loss risk; this documentation change does not alter runtime persistence.

## Repository organization and audience

The former seven public guides were rewritten at their existing addresses. Agent setup, configuration, skills, storage, permissions, session recovery, feedback, desktop use and maintenance now have their own pages rather than being buried in installation or an extended example.

[README](../../README.md), [getting started](../getting-started.md) and the user-facing part of [agent settings](../reference/agent-settings.md) point to the manual. [The repository index](../index.md) separates current development guidance, technical reference and dated evidence. Contributor-specific material remains in the repository. Broken session links and obsolete compatibility guidance were corrected. Existing historical evidence and the landing-page recording retain their original content.

## Verification record

- CLI examples: 36 documented command forms checked against installed Magistr 0.1.0; isolated portable setup, dry run, check-configuration fragment, agent selection, old-setting repair and removal passed. Test folders and user-data locations were disposable. No model turn, real project reset or live user record was used.
- Repository links: all 38 current contributor guides passed file/manual/fragment checks; historical evidence is intentionally outside current-instruction link checks.
- Collection/build checks: 21 routes, metadata, headings, fragments, related pages, redirects and documentation-only search coverage passed.
- Headless WebKit: every page at desktop/compact sizes in light/dark appearance; search results, empty results and load failure; keyboard navigation; active-page visibility; resizing/focus; 320-pixel reflow; no-JavaScript reading; and measured 200% text enlargement passed.
- [Final visual review](desktop-iterations/2026-10-21-product-manual-06/REVIEW.md): all 118 final images opened and inspected, with per-image observations. The [404 recovery supplement](desktop-iterations/2026-10-21-product-manual-07/REVIEW.md) adds four inspected full frames for the changed recovery page. Earlier failed and interrupted attempts remain explicitly labeled and are not substituted for passing evidence.
- Native OS input, assistive technology, live provider model turns, CLI reinstall and destructive data reset were not exercised. Build instructions and those effects were checked against their owners instead.

Native task `docs-review` records the executed check commands, exit statuses and subsequent required GitHub results. This editorial record describes review evidence; it does not claim a deployment occurred.
