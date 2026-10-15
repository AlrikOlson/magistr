# Interface module index

The shared crates hold semantic messages, controls and effect ports. The
application owns sessions, workflow and source resources. The desktop owns
native input, accessible HTML, layout and bounded presentation.
There is one desktop executable and no terminal renderer.

The module inventory below is checked by `scripts/interface-index-gate.py`.
Each retained implementation and harness is named, including the separate
desktop workspace. Companion Rust tests live beside their owners.

| Contract | Owner and evidence |
| --- | --- |
| Resident admission, addressed controls and settlement | `crates/magistr-cli/src/application/session_registry.rs`, its companion suite and `crates/magistr-cli/tests/resident_session_process.py` |
| Names, configuration, stable source revisions, accepted input, status, requests and drafts | `crates/magistr-cli/src/application/session_observation.rs` and its companion suite |
| Canonical source identity, stream coalescing and complete tool updates | `crates/magistr-cli/src/application/retained_observation.rs` and the session observation companion suite |
| Safe rich documents, copy, native selection and reading position | `crates/magistr-desktop/src/content.rs`, `crates/magistr-desktop/src/document.rs`, `crates/magistr-desktop/src/links.rs` and `crates/magistr-desktop/ui/transcript.mjs` |
| Provider and project resources | `crates/magistr-cli/src/application/interactive_host.rs` and `crates/magistr-cli/src/application/session_admissions.rs` |
| One workflow and source authority | `crates/magistr-cli/src/application/workflow_client` and `crates/magistr-cli/src/application/workspace_client` |
| Native installation and Windows window launch | `crates/magistr-desktop/tests/installed_windows.ps1`, `crates/magistr-desktop/tests/installed_macos.py`, `scripts/desktop.mjs` and `.github/workflows/desktop-apps.yml` |
| Protocol-only child roles, no retired dependencies | `crates/magistr-desktop/tests/process.rs` and `crates/magistr-cli/tests/host_clients.rs` |
| Rich source, native held selection, concurrent updates and reading position | `crates/magistr-desktop/tests/rich_conversation_fixture.py`, `crates/magistr-desktop/tests/rich_conversation_journey.py` and `crates/magistr-desktop/tests/rich_conversation.swift` |
| Immutable attachment input, addressed staging and native selection | `crates/magistr-application/src/sessions/prompt_input.rs`, `crates/magistr-cli/src/application/session_attachments.rs`, `crates/magistr-desktop/src/attachments.rs`, `crates/magistr-desktop/ui/attachments.mjs` and `crates/magistr-desktop/tests/attachment_journey.py` |
| Saved drafts, stable attachment selections and appearance | `crates/magistr-cli/src/application/session_drafts.rs`, `crates/magistr-cli/src/agent/persistence/draft.rs`, `crates/magistr-cli/src/application/desktop_preferences.rs` and `crates/magistr-desktop/tests/draft_persistence_journey.py` |
| Native window, keyboard, pointer, clipboard, requests and Quit | `crates/magistr-desktop/tests/native_window.py` and its Swift driver |
| Source/index fitness, contrast, icon and current captures | `scripts/interface-index-gate.py`, `scripts/desktop-visual-gate.py` |

| Work admission, retained run state and boundary controls | `crates/magistr-cli/src/orchestration/hosted_work.rs`, `crates/magistr-cli/src/orchestration/loop_driver/control.rs`, `crates/magistr-desktop/src/work.rs` and `crates/magistr-desktop/tests/work_journey.py` |

## Modules and harnesses

- `crates/magistr-desktop/src/workflow.rs` — addressed on-demand workflow reads from the session owner.
- `crates/magistr-desktop/src/roadmap.rs` — desktop wire shape of shared roadmap projections.
- `crates/magistr-desktop/src/roadmap_item.rs` — selected complete roadmap records and safe rich documents.
- `crates/magistr-desktop/ui/work_item.mjs` — addressed item reader, full authored details and return navigation.
- `crates/magistr-desktop/ui/work_reasoning_search.mjs` — complete authored reasoning search across sessions.
- `crates/magistr-desktop/ui/work_reasoning_links.mjs` — direct references, revisions and project-wide identity resolution.
- `crates/magistr-desktop/ui/work_reasoning_branch.mjs` — session-scoped investigations and recorded lifecycle.
- `crates/magistr-desktop/ui/work_prose.mjs` — complete literal authored prose and exact copying.
- `crates/magistr-desktop/ui/work_feedback_search.mjs` — complete feedback search and saved kind/status filters.
- `crates/magistr-desktop/ui/work_feedback_research.mjs` — append-only research notes, reported confidence and source references.
- `crates/magistr-desktop/ui/work_feedback.mjs` — complete feedback browsing and stable record details.
- `crates/magistr-desktop/ui/work_reasoning.mjs` — complete reasoning sessions, step selection and pages.
- `crates/magistr-desktop/ui/work_reasoning_step.mjs` — complete authored reasoning fields and exact copying.
- `crates/magistr-desktop/ui/work_search.mjs` — complete-record matching and bounded result pages.
- `crates/magistr-desktop/ui/work_roadmap_search.mjs` — complete roadmap search and stable item navigation.
- `crates/magistr-desktop/ui/work_relations.mjs` — complete dependency/dependent navigation and owner readiness.
- `crates/magistr-desktop/ui/work_roadmap.mjs` — track selection and owner-provided readiness.
- `crates/magistr-desktop/ui/work_tabs.mjs` — accessible work-view tab navigation.
- `crates/magistr-desktop/ui/work_browser.mjs` — refresh, address guards and current-plan reading.
- `crates/magistr-desktop/ui/work_records.mjs` — authored work and observation-state presentation.
- `crates/magistr-desktop/ui/work_runs.mjs` — stable current/saved execution selection.
- `crates/magistr-desktop/ui/work_dom.mjs` — workflow DOM and reading-state retention.
- `crates/magistr-desktop/ui/work_checks.mjs` — executed/reported checks and complete output.
- `crates/magistr-desktop/ui/work_activity.mjs` — authored action history and artifact references.

- `crates/magistr-cockpit-runtime/src/channel/arrivals.rs`
- `crates/magistr-cockpit-runtime/src/channel/commands.rs`
- `crates/magistr-cockpit-runtime/src/channel/inbox.rs`
- `crates/magistr-cockpit-runtime/src/channel/mod.rs`
- `crates/magistr-cockpit-runtime/src/channel/observation_sink.rs`
- `crates/magistr-cockpit-runtime/src/interactive_session.rs`
- `crates/magistr-cockpit-runtime/src/jobs.rs`
- `crates/magistr-cockpit-runtime/src/lib.rs`
- `crates/magistr-cockpit-runtime/src/local_session.rs`
- `crates/magistr-cockpit-runtime/src/session.rs`
- `crates/magistr-cockpit-runtime/src/session_jobs.rs`
- `crates/magistr-cockpit/src/checkpoint/apply.rs`
- `crates/magistr-cockpit/src/checkpoint/describe.rs`
- `crates/magistr-cockpit/src/checkpoint/mod.rs`
- `crates/magistr-cockpit/src/checkpoint/port.rs`
- `crates/magistr-cockpit/src/cockpit/budget.rs`
- `crates/magistr-cockpit/src/cockpit/charter_offer.rs`
- `crates/magistr-cockpit/src/cockpit/charter_rule.rs`
- `crates/magistr-cockpit/src/cockpit/checkpoint_cmd.rs`
- `crates/magistr-cockpit/src/cockpit/checkpoint_event.rs`
- `crates/magistr-cockpit/src/cockpit/cmd.rs`
- `crates/magistr-cockpit/src/cockpit/engine_cmd.rs`
- `crates/magistr-cockpit/src/cockpit/galaxy_data.rs`
- `crates/magistr-cockpit/src/cockpit/galaxy_project.rs`
- `crates/magistr-cockpit/src/cockpit/kill_cmd.rs`
- `crates/magistr-cockpit/src/cockpit/launch_action.rs`
- `crates/magistr-cockpit/src/cockpit/loop_cmd.rs`
- `crates/magistr-cockpit/src/cockpit/loop_event.rs`
- `crates/magistr-cockpit/src/cockpit/loop_scope.rs`
- `crates/magistr-cockpit/src/cockpit/loop_stage.rs`
- `crates/magistr-cockpit/src/cockpit/mod.rs`
- `crates/magistr-cockpit/src/cockpit/msg.rs`
- `crates/magistr-cockpit/src/cockpit/project_seed.rs`
- `crates/magistr-cockpit/src/cockpit/proposal_kind.rs`
- `crates/magistr-cockpit/src/cockpit/rule_mode.rs`
- `crates/magistr-cockpit/src/cockpit/supervision_seed.rs`
- `crates/magistr-cockpit/src/cockpit/workbench_cmd.rs`
- `crates/magistr-cockpit/src/cockpit/workbench_event.rs`
- `crates/magistr-cockpit/src/engine/apply.rs`
- `crates/magistr-cockpit/src/engine/describe.rs`
- `crates/magistr-cockpit/src/engine/event.rs`
- `crates/magistr-cockpit/src/engine/mod.rs`
- `crates/magistr-cockpit/src/engine/port.rs`
- `crates/magistr-cockpit/src/harness/fake_workbench.rs`
- `crates/magistr-cockpit/src/harness/fixtures.rs`
- `crates/magistr-cockpit/src/harness/mod.rs`
- `crates/magistr-cockpit/src/job/job_id.rs`
- `crates/magistr-cockpit/src/job/mod.rs`
- `crates/magistr-cockpit/src/job/output.rs`
- `crates/magistr-cockpit/src/lib.rs`
- `crates/magistr-cockpit/src/workbench/apply.rs`
- `crates/magistr-cockpit/src/workbench/describe.rs`
- `crates/magistr-cockpit/src/workbench/journal_entry.rs`
- `crates/magistr-cockpit/src/workbench/mod.rs`
- `crates/magistr-cockpit/src/workbench/port.rs`
- `crates/magistr-cockpit/src/workbench/run_row.rs`
- `crates/magistr-desktop/src/args.rs`
- `crates/magistr-desktop/src/attachments.rs`
- `crates/magistr-desktop/tests/attachment.swift`
- `crates/magistr-desktop/tests/attachment_fixture.py`
- `crates/magistr-desktop/tests/attachment_journey.py`
- `crates/magistr-desktop/tests/attachments.mjs`
- `crates/magistr-desktop/ui/attachments.mjs`
- `crates/magistr-desktop/src/commands.rs`
- `crates/magistr-desktop/src/content.rs`
- `crates/magistr-desktop/src/document.rs`
- `crates/magistr-desktop/src/links.rs`
- `crates/magistr-desktop/tests/reading.mjs` — finite view-coordinate retention and closed-owner cleanup.
- `crates/magistr-desktop/ui/reading.mjs` — bounded semantic reading and native editor position bookmarks; no durable content cache.
- `crates/magistr-desktop/tests/browser/reading.spec.mjs` — background switching, resize, tool disclosure, selection and editor continuity.
- `crates/magistr-desktop/ui/source_details.mjs` — shared bounded JSON documents for tool results and permission requests, with exact original text.
- `crates/magistr-desktop/ui/reply_outline.mjs` — contents derived from real reply headings.
- `crates/magistr-desktop/ui/transcript.mjs`
- `crates/magistr-desktop/tests/transcript.mjs`
- `crates/magistr-desktop/src/conversations.rs`
- `crates/magistr-desktop/src/credit.rs`
- `crates/magistr-desktop/src/drafts.rs`
- `crates/magistr-desktop/src/exit.rs`
- `crates/magistr-desktop/src/preferences.rs`
- `crates/magistr-desktop/ui/drafts.mjs`
- `crates/magistr-desktop/ui/preferences.mjs`
- `crates/magistr-desktop/ui/motion.mjs` — finite dialog and writing-surface entrances, cancellation when reduced motion applies, and disposal.
- `crates/magistr-desktop/ui/saved_drafts.mjs`
- `crates/magistr-desktop/tests/drafts.mjs`
- `crates/magistr-desktop/tests/preferences.mjs`
- `crates/magistr-desktop/tests/saved_drafts.mjs`
- `crates/magistr-desktop/tests/draft_persistence.swift`
- `crates/magistr-desktop/tests/draft_persistence_journey.py`
- `crates/magistr-desktop/src/launch.rs`
- `crates/magistr-desktop/src/main.rs`
- `crates/magistr-desktop/src/environment.rs` — bounded macOS login-shell command path before runtime startup; protocol children inherit it.
- `crates/magistr-desktop/src/projects.rs`
- `crates/magistr-desktop/tests/projects.mjs`
- `crates/magistr-desktop/ui/projects.mjs`
- `crates/magistr-desktop/ui/project_picker.mjs` — folder entry, independent file review, exact approval selection and setup recovery.
- `crates/magistr-desktop/tests/browser/projects.spec.mjs` — background setup review, cancellation, partial results and short-window actions.
- `crates/magistr-desktop/src/streams.rs`
- `crates/magistr-desktop/src/wire.rs`
- `crates/magistr-desktop/tests/composer.mjs`
- `crates/magistr-desktop/tests/conversation.mjs`
- `crates/magistr-desktop/tests/browser/fixture.mjs` — isolated observation fixture for the production UI; no OS input or owner integration.
- `crates/magistr-desktop/tests/browser/workspace.spec.mjs` — headless WebKit layout, themes, narrow Details and reviewed captures.
- `crates/magistr-desktop/tests/native_opt_in.test.mjs` — refuses foreground journeys before side effects unless explicitly enabled.
- `crates/magistr-desktop/tests/native_window.py`
- `crates/magistr-desktop/tests/native_window.swift`
- `crates/magistr-desktop/tests/process.rs`
- `crates/magistr-desktop/tests/process_roles.py`
- `crates/magistr-desktop/tests/response.mjs`
- `crates/magistr-desktop/tests/rich_conversation_fixture.py`
- `crates/magistr-desktop/tests/rich_conversation_journey.py`
- `crates/magistr-desktop/tests/rich_conversation.swift`
- `crates/magistr-desktop/tests/selection.mjs`
- `crates/magistr-desktop/tests/session_controls.mjs`
- `crates/magistr-desktop/tests/session_controls_fixture.py`
- `crates/magistr-desktop/ui/activity_material.mjs` — bounded ray-marched activity torus, cached on-demand draws, theme/resize observation, context loss and GPU cleanup.
- `crates/magistr-desktop/ui/activity.mjs` — current catalog activity counts and their visual overview; no extra session state.
- `crates/magistr-desktop/ui/source_color.mjs` — bounded lexical color for explicitly named source languages; exact text stays intact.
- `crates/magistr-desktop/ui/patch_document.mjs` — bounded patch line, position and change-count presentation.
- `crates/magistr-desktop/ui/code_blocks.mjs` — stable framed source documents, nonselectable line gutters and exact code copying.
- `crates/magistr-desktop/ui/attachment_viewer.mjs` — fit/actual-size image inspection from owned attachment previews with focus restoration and removal handling.
- `crates/magistr-desktop/tests/browser/atlas.spec.mjs` — spatial catalog bounds, focus and addressed navigation.
- `crates/magistr-desktop/ui/project_atlas.mjs` — bounded spatial catalog view with stable native controls.
- `crates/magistr-desktop/ui/app.mjs`
- `crates/magistr-desktop/ui/history.mjs` — searchable saved-record browser, independent reading/list surfaces and guarded resume feedback.
- `crates/magistr-desktop/tests/browser/history.spec.mjs` — background filtering, draft recovery, loading cancellation and refused resume.
- `crates/magistr-desktop/ui/conversation_picker.mjs` — destination, detected agents, optional model preview and guarded conversation admission.
- `crates/magistr-desktop/tests/browser/opening.spec.mjs` — background discovery, model read fencing, admission retry and focus.
- `crates/magistr-desktop/ui/conversation_actions.mjs` — addressed rename dialog with pending acknowledgement, errors and stale-target refusal.
- `crates/magistr-desktop/tests/browser/context.spec.mjs` — rename recovery, work summary and waiting-request navigation.
- `crates/magistr-desktop/ui/keyboard.mjs` — platform shortcut hints, modal/IME guards and logical region navigation.
- `crates/magistr-desktop/ui/switcher.mjs` — searchable current-catalog conversations and guarded selection/focus recovery.
- `crates/magistr-desktop/tests/browser/keyboard.spec.mjs` — background shortcut, region, modal, IME guard and refused-selection journeys.
- `crates/magistr-desktop/ui/navigation.mjs` — shared canonical project grouping, live opening directory, rail and addressed conversation actions.
- `crates/magistr-desktop/tests/navigation.mjs` — project identity and linked-copy grouping.
- `crates/magistr-desktop/tests/browser/navigation.spec.mjs` — background navigation, contextual actions and Settings recovery.
- `crates/magistr-desktop/ui/layout.mjs` — compact project browsing and responsive conversation/details visibility and focus; no session lifetime ownership.
- `crates/magistr-desktop/ui/writing_surface.mjs` — expanded composition of the existing native textarea.
- `crates/magistr-desktop/ui/composer.mjs`
- `crates/magistr-desktop/ui/context.mjs`
- `crates/magistr-desktop/ui/conversation.mjs`
- `crates/magistr-desktop/ui/selection.mjs`
- `crates/magistr-desktop/ui/session_controls.mjs`
- `crates/magistr-desktop/ui/supervision.mjs`
- `crates/magistr-desktop/tests/supervision.mjs`
- `crates/magistr-cockpit/src/cockpit/run_command.rs`
- `crates/magistr-cockpit/src/cockpit/work_phase.rs`
- `crates/magistr-cockpit/src/cockpit/work_run.rs`
- `crates/magistr-desktop/src/work.rs`
- `crates/magistr-desktop/ui/work_ruler.mjs` — bounded attempt-position landmarks from the current run; no inferred completion.
- `crates/magistr-desktop/ui/work.mjs`
- `crates/magistr-desktop/tests/work.mjs`
- `crates/magistr-desktop/tests/work_fixture.py`
- `crates/magistr-desktop/tests/work_journey.py`