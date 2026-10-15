# Desktop roadmap delivery — October 15, 2026

This delivery implements the requested 30 chunks, prioritizing roadmap features.
Each row is one native roadmap chunk and one atomic commit. The first four
repair the native records and agent-context owners needed by the desktop;
the remaining work builds the Tauri interface on those owners.

## Delivered work

| # | Delivered behavior | Native roadmap chunk | Commit |
| --- | --- | --- | --- |
| 1 | Retain complete executions with stable run and task-instance identities. | `rust-gui-execution-history` | `9d3e701d` |
| 2 | Load addressed native work context into agent startup and resume. | `workflow-agent-context-ownership` | `38aa57dc` |
| 3 | Remove unproduced plan files and use the native work owner. | `workflow-remove-orphan-plans` | `09e92948` |
| 4 | Read complete default and named reasoning histories explicitly. | `workflow-reasoning-scope-ownership` | `f899ba45` |
| 5 | Read the complete current objective, requirements and tasks. | `rust-gui-current-plan` | `f1ae1d03` |
| 6 | Select saved work by stable execution identity. | `rust-gui-saved-work-browser` | `ae1e7da9` |
| 7 | Read every executed or reported check attempt and full output. | `rust-gui-check-results` | `7a09b3e3` |
| 8 | Read task activity, full outcomes and produced-item references. | `rust-gui-task-activity` | `8ec41924` |
| 9 | Browse tracks with owner-derived counts and readiness. | `rust-gui-roadmap-overview` | `e16d19bc` |
| 10 | Read complete roadmap descriptions, requirements and structured context. | `rust-gui-roadmap-details` | `635da229` |
| 11 | Share immutable workflow observations during streamed replies. | `rust-gui-observation-snapshot-sharing` | `27386657` |
| 12 | Navigate prerequisites and dependents with a local return trail. | `rust-gui-roadmap-dependencies` | `e84276f8` |
| 13 | Browse completed and retired roadmap items. | `rust-gui-roadmap-history` | `ee4a61e8` |
| 14 | Search complete records across roadmap tracks. | `rust-gui-roadmap-search` | `03704381` |
| 15 | Browse complete reasoning sessions and stable step details. | `rust-gui-reasoning-browser` | `ca6744fd` |
| 16 | Inspect investigations, membership and recorded outcomes. | `rust-gui-reasoning-branches` | `6e5d5fba` |
| 17 | Follow reasoning dependencies and revisions across sessions. | `rust-gui-reasoning-links` | `240a3b93` |
| 18 | Search complete authored reasoning across histories. | `rust-gui-reasoning-search` | `8b903bbd` |
| 19 | Read full feedback and authored context. | `rust-gui-feedback-browser` | `ef0d73ea` |
| 20 | Inspect research notes and source references. | `rust-gui-feedback-research` | `193610c1` |
| 21 | Search feedback and research with kind/status filters. | `rust-gui-feedback-search` | `192d2005` |
| 22 | Follow exact related records and return to feedback. | `rust-gui-feedback-references` | `b674c1ab` |
| 23 | Read suggested changes beside the actual saved values. | `rust-gui-roadmap-proposals` | `528d8729` |
| 24 | Search complete current and retained executions. | `rust-gui-execution-search` | `fd670f38` |
| 25 | Filter and page tasks while preserving original plan positions. | `rust-gui-task-navigation` | `85d2223a` |
| 26 | Search every check attempt with recorded-outcome filters. | `rust-gui-check-index` | `189f339a` |
| 27 | Search recorded activity and produced items across tasks. | `rust-gui-activity-index` | `2b63c355` |
| 28 | Inspect independent owner read status, counts and provenance. | `rust-gui-record-status` | `c235456d` |
| 29 | Retain reading places across tabs and keep failed refreshes explicit. | `rust-gui-work-reading-places` | `8f2cb165` |
| 30 | Read selected records in compact windows and verify complete large corpora. | `rust-gui-compact-work-reading` | This document's introducing commit |

The final commit is titled `feat(desktop): make complete work readable in compact windows`.
Its identity is the introducing commit of this file.

## Verification

Every chunk completed its focused recorded checks before commit. Changes to Rust
owners and contracts used the affected crate tests, formatting, Clippy and
architecture checks. Desktop changes used the affected headless WebKit scenarios
and the screenshot gate; they did not trigger unrelated workspace-wide suites.

The final interface verification covers all 49 workflow-browser cases:

```sh
npm --prefix crates/magistr-desktop run test:ui -- 'work_.*\.spec\.mjs'
```

The large-corpus case uses 10,000 roadmap items, 10,000 reasoning steps,
10,000 feedback items and 10,000 tasks/check attempts. Searches reach the final
records while list pages render at most 50 matches. Complete long reasoning
text is copied exactly. This checks browser presentation and interaction using
native-contract fixtures; it is not a native runtime throughput benchmark.

The focused layout scenario renders the production interface in headless WebKit.
The [receipt](receipt.json) binds 131 captures to the UI and scenario sources,
and the [review](review.json) records inspected images. At the 760×550 minimum,
task browsing retains at least 160 pixels of reading height and selected details
at least 270 pixels. Back restores collection controls; removed reasoning histories
still offer a way back. Both appearances and reduced-motion behavior are included
in the layout checks. The screenshot gate also checks contrast, icons and the
interface index.

Selected minimum-size captures:

- [Task plan](work-plan-minimum.png)
- [Roadmap detail](roadmap-detail-minimum-light.png)
- [Reasoning detail](reasoning-detail-minimum-light.png)
- [Feedback detail](feedback-detail-minimum-light.png)

## Limits and remaining work

Current native OS-input journeys, VoiceOver, native clipboard behavior and live
provider integration were not exercised in this delivery. Historical native
evidence retains its actual recorded source version. Headless checks do not
establish current native or assistive-technology behavior.

The configured workflow server stayed running throughout this work. Its loaded
binary predates the native execution-retention change; a newly built process is
needed to use the new owner behavior.

The parent desktop roadmap still includes planning-conversation controls,
roadmap editing and working-copy change review. Two ownership defects were filed
separately: `workflow-reasoning-write-selection` for explicit reasoning mutation
routing, and `workflow-qualified-execution-references` for unambiguous references
to execution tasks and checks. These are not claimed as delivered here.

See the [desktop architecture](../../architecture/desktop.md) for the current
writer, reader, identity and navigation contracts.
