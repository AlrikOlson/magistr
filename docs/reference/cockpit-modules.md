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
| Names, configuration, accepted input, status, requests and drafts | `crates/magistr-cli/src/application/session_observation.rs` and its companion suite |
| Provider and project resources | `crates/magistr-cli/src/application/interactive_host.rs` and `crates/magistr-cli/src/application/project_resources.rs` |
| One workflow and source authority | `crates/magistr-cli/src/application/workflow_client` and `crates/magistr-cli/src/application/workspace_client` |
| Protocol-only child roles, no retired dependencies | `crates/magistr-desktop/tests/process.rs` and `crates/magistr-cli/tests/host_clients.rs` |
| Native window, keyboard, pointer, clipboard, requests and Quit | `crates/magistr-desktop/tests/native_window.py` and its Swift driver |
| Source/index fitness, contrast, icon and current captures | `scripts/interface-index-gate.py`, `scripts/desktop-visual-gate.py` |

## Modules and harnesses

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
- `crates/magistr-desktop/src/commands.rs`
- `crates/magistr-desktop/src/conversations.rs`
- `crates/magistr-desktop/src/credit.rs`
- `crates/magistr-desktop/src/launch.rs`
- `crates/magistr-desktop/src/main.rs`
- `crates/magistr-desktop/src/projects.rs`
- `crates/magistr-desktop/tests/projects.mjs`
- `crates/magistr-desktop/ui/projects.mjs`
- `crates/magistr-desktop/src/streams.rs`
- `crates/magistr-desktop/src/wire.rs`
- `crates/magistr-desktop/tests/composer.mjs`
- `crates/magistr-desktop/tests/conversation.mjs`
- `crates/magistr-desktop/tests/native_window.py`
- `crates/magistr-desktop/tests/native_window.swift`
- `crates/magistr-desktop/tests/process.rs`
- `crates/magistr-desktop/tests/process_roles.py`
- `crates/magistr-desktop/tests/response.mjs`
- `crates/magistr-desktop/tests/selection.mjs`
- `crates/magistr-desktop/tests/session_controls.mjs`
- `crates/magistr-desktop/tests/session_controls_fixture.py`
- `crates/magistr-desktop/ui/app.mjs`
- `crates/magistr-desktop/ui/composer.mjs`
- `crates/magistr-desktop/ui/conversation.mjs`
- `crates/magistr-desktop/ui/selection.mjs`
- `crates/magistr-desktop/ui/session_controls.mjs`
- `crates/magistr-desktop/ui/supervision.mjs`
- `crates/magistr-desktop/tests/supervision.mjs`
