# Cockpit module reference

The module index lists the shared cockpit, terminal renderer, and the host's
workflow and workspace clients, application session contracts, shared runtime
transport and host session relays, with their responsibilities and tests. Paths in
table cells are relative to `crates/`.

See [terminal architecture](../architecture/terminal.md) for the data flow and
[desktop architecture](../architecture/desktop.md) for the native interface.


## Canonical workflow contracts

These modules define the records used by producers and surfaces. Contract
and dependency guards are `magistr-engine/tests/workflow_contracts.rs` and
`magistr-cockpit/tests/boundary.rs`. Native wire, cursor and owner-file tests
remain in the engine crate, where they also exercise the real publisher.

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-workflow-contracts/src/broadcast_frame.rs` | Supplied wire-line decoding | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/content.rs` | Structured record content and validation | `magistr-workflow-contracts/src/content.tests.rs` |
| `magistr-workflow-contracts/src/delivery.rs` | Arrival ordering and freshness consequences | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/event_cursor.rs` | Publisher epoch and sequence | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/family.rs` | Family names and wire stamps | `magistr-workflow-contracts/src/family.tests.rs` |
| `magistr-workflow-contracts/src/feed_cursor.rs` | Duplicate, gap and epoch classification | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/feed_status.rs` | Connection observation | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/gate_output.rs` | Captured gate bytes | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/lib.rs` | Canonical values and current re-exports | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/magistr_frame.rs` | Typed host phase and output events | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/native_event.rs` | One typed workflow or host event | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/native_snapshot.rs` | One per-family coherent snapshot | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/owner_generation.rs` | Observed nonzero owner generation; no allocation | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/roadmap/frame.rs` | Typed family mutation events | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/roadmap/mod.rs` | Public family facade | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/roadmap/name.rs` | Short work-item names | `magistr-workflow-contracts/src/roadmap/name.tests.rs` |
| `magistr-workflow-contracts/src/roadmap/records.rs` | Family records and deterministic merge rules | `magistr-workflow-contracts/src/roadmap/records.tests.rs` |
| `magistr-workflow-contracts/src/roadmap_progress.rs` | Aggregate completion values | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/ship/action.rs` | Recorded action and kind | `magistr-workflow-contracts/src/ship/action.tests.rs` |
| `magistr-workflow-contracts/src/ship/artifact.rs` | Produced artifact reference and kind | `magistr-workflow-contracts/src/ship/artifact.tests.rs` |
| `magistr-workflow-contracts/src/ship/check.rs` | Recorded check and evidence origin | `magistr-workflow-contracts/src/ship/check.tests.rs` |
| `magistr-workflow-contracts/src/ship/frame.rs` | Typed family mutation events | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/ship/mod.rs` | Public family facade | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/ship/objective.rs` | Objective, verification mode and acceptance revision | `magistr-workflow-contracts/src/ship/objective.tests.rs` |
| `magistr-workflow-contracts/src/ship/report.rs` | Report parsing outcome and test summary values | `magistr-workflow-contracts/src/ship/report.tests.rs` |
| `magistr-workflow-contracts/src/ship/task.rs` | Task status and recorded evidence | `magistr-workflow-contracts/src/ship/task.tests.rs` |
| `magistr-workflow-contracts/src/ship/verification_mode.rs` | Focused or full verification | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/signal/frame.rs` | Typed family mutation events | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/signal/mod.rs` | Public family facade | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/signal/records.rs` | Family records and deterministic merge rules | `magistr-workflow-contracts/src/signal/records.tests.rs` |
| `magistr-workflow-contracts/src/snapshot_cursor.rs` | Supplied store observation stamp | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think/branch.rs` | Reasoning branch and lifecycle | `magistr-workflow-contracts/src/think/branch.tests.rs` |
| `magistr-workflow-contracts/src/think/dep_edge.rs` | Reasoning dependency reference | `magistr-workflow-contracts/src/think/dep_edge.tests.rs` |
| `magistr-workflow-contracts/src/think/frame.rs` | Typed family mutation events | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think/history.rs` | Persisted trace and metadata | `magistr-workflow-contracts/src/think/history.tests.rs` |
| `magistr-workflow-contracts/src/think/mod.rs` | Public family facade | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think/recorded_step.rs` | Recorded reasoning response | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think/session.rs` | Supplied session history entry | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think/step.rs` | Reasoning step, next action and stable identity | `magistr-workflow-contracts/src/think/step.tests.rs` |
| `magistr-workflow-contracts/src/think/step_error.rs` | Reasoning validation error | `magistr-engine/tests/workflow_contracts.rs` |
| `magistr-workflow-contracts/src/think_branch.rs` | Recorded branch metadata | `magistr-engine/tests/workflow_contracts.rs` |

## Motion registry

| Motion | Means | Night | Day |
| --- | --- | --- | --- |
| `sweep` | prose is streaming | bright band crosses the line | subtle band, no white |
| `arrive` | something new landed | 220 ms rise + glow | rise only |
| `breathe` | the working thing | halo swells | compass ring pulses |
| `twinkle` | shipped history, alive | slow phase per star | none |
| `ready-pulse` | claimable now | expanding ring | expanding ring, drawn |
| `ignite` | a chunk ships | workstream-hue flare → green | mark inked in → green |
| `edge-run` | the unlock | light travels the edge | pen stroke draws the edge |
| `wake` | dashed → ready | steps once | steps once |
| `toast · counter` | the offer · the tally | rise/fade · tick | rise/fade · tick |
| `rise` | cards dealt | staggered, worst first | staggered, worst first |
| `born` | the charting | stars appear in reasoning order | marks drawn in order |
| `radar` | the survey runs | light wedge circles | graphite arc circles |
| `caret · blink` | input waits here | 1.1 s steps | 1.1 s steps |
| `chrome fade` | rest ↔ intent | 150 ms both ways | 150 ms both ways |
| `skeleton` | content loading | shimmer rows · ghost stars pulse | shimmer rows · ghost stars pulse, no glow |
| `parallax` | panning the sky | stars lag the graticule | marks lag the grid |

## Invariants and checks

| Invariant | Tests |
| --- | --- |
| `update`/`view` are pure and reach no effect | `magistr-cockpit/tests/purity.rs` |
| the core never names a producer handle | `magistr-cockpit/tests/purity.rs` |
| semantic ports and job results own no runtime effects | `magistr-cockpit/tests/purity.rs` |
| the theme reads the environment in exactly one place | `magistr-cockpit/tests/purity.rs` |
| terminal bytes and neutral input share draft behavior; replies affect capabilities without input or effects; resizing preserves measurement precedence | `magistr-tui/tests/boundary.rs`, `magistr-tui/src/runtime/driver.tests.rs` |
| the host opens engines before the terminal is taken over | `magistr-cli/tests/host_clients.rs` |
| shared records have one canonical owner and the cockpit dependency closure excludes concrete stores and runtime clients | `magistr-cockpit/tests/boundary.rs`, `magistr-engine/tests/workflow_contracts.rs` |
| concrete workflow and workspace clients stay in the host | `magistr-cockpit/tests/boundary.rs` |
| terminal dependencies exclude the legacy GUI, with `deny(dead_code, unused)` enabled | `magistr-tui/tests/boundary.rs` |
| no cockpit surface names a terminal protocol or the terminal crate, and the cockpit crate's root keeps `deny(dead_code, unused)` | `magistr-cockpit/tests/boundary.rs` |
| every terminal still and filmstrip matches its golden | `magistr-tui/tests/goldens.rs` |
| what a terminal shows equals what the painter drew | `magistr-tui/tests/end_to_end.rs` |
| no input can panic the renderer | `magistr-tui/tests/fuzz.rs` |
| the loop coalesces bursts, and a frame folds what already arrived | `magistr-tui/tests/runtime_harness.rs` |
| the turn stage streams, scrolls and repaints within its latency bound | `magistr-tui/tests/turn_harness.rs` |
| cockpit journal and undo use the agent's owner; conflicts preserve external edits and the next write keeps the generation | `magistr-cli/tests/cockpit_journal_owner.rs` |
| a real ACP session drives the cockpit end to end | `magistr-cli/tests/session_harness.rs` |
| jobs and engines never stall the loop | `magistr-tui/tests/jobs_engines_harness.rs` |
| the compositor's economy: one transmit per change, none while idle | `magistr-tui/tests/graphics_harness.rs` |
| nothing else in the process can write to the screen mid-session | `magistr-tui/tests/terminal_integrity.rs` |
| a full 120×40 frame stays under 1 ms at p99 (release, `--ignored`) | `magistr-tui/tests/perf.rs` |
| the pixel layer's per-frame byte budget (release, `--ignored`) | `magistr-tui/tests/pixel_perf.rs` |
| pixel scrolling samples between model ticks, then stops; reduced motion jumps directly (release, `--ignored`) | `magistr-tui/tests/runtime_harness.rs` |
| text measurement and glyph blending preserve pixels while avoiding repeated work | `magistr-cockpit/src/pixel/typography.tests.rs`, `magistr-tui/src/render/pixel_view.tests.rs` |
| this page names every module and every harness | `magistr-tui/tests/architecture_doc.rs` |

## Module index

Each row names a source module, its responsibility, and a companion or integration
test. The section headings separate the crates and their module families.

## Module index — `crates/magistr-cockpit`

### crate root

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/lib.rs` | Shared interface model and rendering primitives | `magistr-cockpit/tests/boundary.rs` |
| `magistr-cockpit/src/base64.rs` | RFC 4648 base64 encoding with the standard alphabet and padding | `magistr-cockpit/src/base64.tests.rs` |
| `magistr-cockpit/src/fonts.rs` | The embedded monospace faces as bytes, shared by every renderer magistr has | `magistr-tui/src/screenshot/typeface.tests.rs` |
| `magistr-cockpit/src/widget_id.rs` | The identity a widget keeps across layout, painting and hit-testing | `magistr-cockpit/src/widget_id.tests.rs` |

### Application session contracts

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-application/src/sessions/ask_everything.rs` | Permission policy that asks the user for every request | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/ask_plan.rs` | Plan details attached to a permission request: steps, images, and scope | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-application/src/sessions/edit_diff.rs` | The diff a tool call carried | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/event.rs` | Agent session events | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/image_block.rs` | An image block the protocol carried | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/mod.rs` | Portable session commands, observations, options and policies | `magistr-cli/tests/application_boundary.rs` |
| `magistr-application/src/sessions/permission_choice.rs` | One option the agent offers in a permission request | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/permission_kind.rs` | What choosing a permission option means | `magistr-application/src/sessions/permission_kind.tests.rs` |
| `magistr-application/src/sessions/policy_decision.rs` | What a policy says about one permission request | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/policy_request.rs` | Full tool identity and encoded input for host permission decisions | `magistr-cli/src/permissions/request_data.tests.rs` |
| `magistr-application/src/sessions/prompt_files.rs` | File-reference anchoring and deduplication with adapter-supplied quoted-path decoding | `magistr-application/src/sessions/prompt_files.tests.rs`, `magistr-acp/src/transport/prompt_content.tests.rs` |
| `magistr-application/src/sessions/service.rs` | Scoped session use cases and command/outcome validation | `magistr-application/src/sessions/service.tests.rs` |
| `magistr-application/src/sessions/session_command.rs` | Commands addressed to domain session and turn identities | `magistr-application/src/sessions/service.tests.rs` |
| `magistr-application/src/sessions/session_handle.rs` | Live session operations and application-owned event observations | `magistr-cli/tests/cockpit_session_contract.rs` |
| `magistr-application/src/sessions/session_options.rs` | What a session does once the handshake completes | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/session_outcome.rs` | Session readiness, accepted turns, cancellation and closure outcomes | `magistr-application/src/sessions/service.tests.rs` |
| `magistr-application/src/sessions/session_policy.rs` | The decision a session asks for before a permission request reaches the cockpit | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/session_port.rs` | Host boundary for scoped session commands | `magistr-application/src/sessions/service.tests.rs` |
| `magistr-application/src/sessions/session_response.rs` | Scoped session outcome paired with its command receipt | `magistr-application/src/sessions/service.tests.rs` |
| `magistr-application/src/sessions/tool_payload.rs` | Plain text or encoded JSON without a JSON-library dependency | `magistr-cli/src/agent/translate.tests.rs`, `magistr-application/src/sessions/tool_record.tests.rs` |
| `magistr-application/src/sessions/tool_record.rs` | Tool-call metadata and output, bounded by `ToolRecord::CAP` and stored with the source line | `magistr-application/src/sessions/tool_record.tests.rs`, `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-application/src/sessions/tool_status.rs` | Where a tool call stands | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/tool_verb.rs` | What kind of thing a tool call does, as a one-word verb column | `magistr-cli/tests/session_harness.rs` |
| `magistr-application/src/sessions/turn_end.rs` | Why a turn ended | `magistr-cli/tests/session_harness.rs` |

### Host session translation

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cli/src/agent/translate.rs` | ACP events converted to application observations and bounded tool records | `magistr-cli/src/agent/translate.tests.rs` |
| `magistr-cli/src/agent/permission_request.rs` | ACP tool identity, full input and offered choice meanings converted to application contracts | `magistr-cli/src/permissions/request_data.tests.rs` |
| `magistr-cli/src/permissions/request_data.rs` | Host decoding of portable policy requests, preserving kind and full input | `magistr-cli/src/permissions/request_data.tests.rs` |

### `agent` — tool presentation and host relay

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/agent/mod.rs` | Tool-title presentation for shared session observations | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit-runtime/src/session.rs` | Relays application session events to the inbox and explicit commands to the shared owner; stamps presentation timing and serves local effect ports | `magistr-cli/tests/session_harness.rs`, `magistr-cockpit-runtime/src/session.tests.rs` |
| `magistr-cockpit-runtime/src/session_jobs.rs` | The blocking port calls a session relay owns: a capacity, a fence closed on settlement or drop so late answers never post, a join grace | `magistr-cockpit-runtime/src/session.tests.rs` |
| `magistr-cockpit/src/agent/tool_title.rs` | Familiar tool titles shared by conversation cards, plain output, edits, and permission requests | `magistr-cockpit/src/agent/tool_title.tests.rs` |

### `capability` — what the surface can show

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/capability/mod.rs` | Display capabilities available to the shared model | `magistr-cockpit/src/capability/mode.tests.rs` |
| `magistr-cockpit/src/capability/mode.rs` | Pixels or plain text: decided once at startup, never mid-session | `magistr-cockpit/src/capability/mode.tests.rs` |
| `magistr-cockpit/src/capability/text_reason.rs` | Why the cockpit is running without pixels | `magistr-tui/tests/graphics_harness.rs` |

### Shared runtime — the inbox, commands and background jobs

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit-runtime/src/lib.rs` | Shared message transport, conversation coordination and job dispatch without concrete host dependencies | `magistr-cockpit-runtime/tests/boundary.rs` |
| `magistr-cockpit-runtime/src/interactive_session.rs` | Purpose-aware connection, retained conversation/planning contexts, queued requests, retry and owned relay shutdown | `magistr-cockpit-runtime/src/interactive_session.tests.rs`, `magistr-cli/src/orchestration/loop_driver/driver.tests.rs` |
| `magistr-cockpit-runtime/src/local_session.rs` | Provider-free handle for local project controls; refuses unavailable provider actions | `magistr-cockpit-runtime/src/interactive_session.tests.rs` |
| `magistr-cli/src/application/interactive_host.rs` | Admitted project/provider startup, projection feeds, surface command forwarding and bounded shutdown | `magistr-cli/src/application/interactive_host.tests.rs`, `magistr-cli/tests/host_clients.rs` |
| `magistr-cli/src/orchestration/interactive_session.rs` | Host composition: resolved provider connector, planning policy and admitted concrete project ports for the shared coordinator | `magistr-cli/src/orchestration/interactive_session.tests.rs`, `magistr-cli/src/orchestration/loop_driver/driver.tests.rs` |
| `magistr-cockpit-runtime/src/channel/mod.rs` | Producer inbox and runtime command channels | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-cockpit-runtime/src/channel/arrivals.rs` | When a message reached the runtime's inbox | `magistr-cockpit-runtime/src/channel/arrivals.tests.rs` |
| `magistr-cockpit-runtime/src/channel/commands.rs` | The core's commands on their way to the session | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-cockpit-runtime/src/channel/inbox.rs` | The channel every producer posts on | `magistr-tui/tests/runtime_harness.rs` |

### `checkpoint` — the ledger rail's port

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/checkpoint/mod.rs` | Checkpoint operations for the session ledger | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit/src/checkpoint/apply.rs` | One checkpoint command, carried out on a port | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit/src/checkpoint/describe.rs` | What a checkpoint command asked, in the words a refusal names | `magistr-cockpit-runtime/src/session.tests.rs` |
| `magistr-cli/src/orchestration/loop_driver/checkpoint_detach.rs` | Runs checkpoint operations on a blocking thread and returns results through the inbox | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit/src/checkpoint/port.rs` | Where a checkpoint is actually taken, resumed and forked | `magistr-cli/tests/session_harness.rs` |

### `cockpit` — the Elm core

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/cockpit/mod.rs` | Elm-style model, messages, update function, runtime commands, and view tree | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/agent_activity.rs` | Observed request intent and agent activity, independent of historical objective completion | `magistr-cockpit/src/cockpit/agent_activity.tests.rs` · `magistr-cockpit/src/cockpit/roadmap_request.tests.rs` |
| `magistr-cockpit/src/cockpit/agent_missing.rs` | Missing agent executable details, including its last-seen timestamp | `magistr-cockpit/src/cockpit/bad_day.tests.rs` |
| `magistr-cockpit/src/cockpit/animation.rs` | Animation state and tick interval | `magistr-cockpit/src/cockpit/animation.tests.rs` |
| `magistr-cockpit/src/cockpit/animations.rs` | Active animation set and shared timer interval | `magistr-cockpit/src/cockpit/animations.tests.rs` |
| `magistr-cockpit/src/cockpit/bad_day.rs` | Failure cards derived from model state and ordered by severity | `magistr-cockpit/src/cockpit/bad_day.tests.rs` |
| `magistr-cockpit/src/cockpit/binding.rs` | Keyboard shortcuts and their display groups | `magistr-cockpit/src/cockpit/binding.tests.rs` |
| `magistr-cockpit/src/cockpit/birth.rs` | Project-creation state: survey, interview, proposal, accepted output, and animation timing | `magistr-cockpit/src/cockpit/birth.tests.rs` |
| `magistr-cockpit/src/cockpit/birth_cmd.rs` | Project setup commands for survey, proposal, and file generation | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/birth_moment.rs` | Project setup stages: location, survey, interview, proposal, and completion | `magistr-cockpit/src/cockpit/birth_moment.tests.rs` |
| `magistr-cockpit/src/cockpit/born_data.rs` | Accepted project setup output: roadmap items, gates, and protected paths | `magistr-cockpit/src/cockpit/born_data.tests.rs` |
| `magistr-cockpit/src/cockpit/brief.rs` | Session activity summary derived from the model and ordered by severity | `magistr-cockpit/src/cockpit/brief.tests.rs` |
| `magistr-cockpit/src/cockpit/brief_body.rs` | Activity card body containing an action or a meter | `magistr-cockpit/src/cockpit/brief_body.tests.rs` |
| `magistr-cockpit/src/cockpit/brief_card.rs` | Activity summary card with severity, heading, and body | `magistr-cockpit/src/cockpit/brief_card.tests.rs` |
| `magistr-cockpit/src/cockpit/budget.rs` | Session spend, budget limit, and breaker threshold in cents | `magistr-cockpit/src/cockpit/budget.tests.rs` |
| `magistr-cockpit/src/cockpit/camera.rs` | Roadmap graph viewport and camera transitions | `magistr-cockpit/src/cockpit/camera.tests.rs` |
| `magistr-cockpit/src/cockpit/chunk_state.rs` | Typed roadmap state, including Stuck derived from in-progress work with a blocker | `magistr-cockpit/src/cockpit/chunk_state.tests.rs` |
| `magistr-cockpit/src/cockpit/changes.rs` | Journal state and interaction: file identity anchors, review, undo refusals and workbench outcomes | `magistr-cockpit/src/cockpit/changes.tests.rs` |
| `magistr-cockpit/src/cockpit/chapter.rs` | Completed roadmap item details for reports and exports | `magistr-cockpit/src/cockpit/chapter.tests.rs` |
| `magistr-cockpit/src/cockpit/charter_offer.rs` | A proposed scope expansion, its allowed/total count, and the key sent when the user delegates | `magistr-cockpit/src/cockpit/charter_offer.tests.rs` · `magistr-cockpit/src/cockpit/rail.tests.rs` |
| `magistr-cockpit/src/cockpit/charter_rule.rs` | Session permission rule displayed in the rail | `magistr-cockpit/src/cockpit/charter_rule.tests.rs` |
| `magistr-cockpit/src/cockpit/checkpoint.rs` | Checkpoint entry displayed in the session ledger | `magistr-cockpit/src/cockpit/checkpoint.tests.rs` |
| `magistr-cockpit/src/cockpit/checkpoint_cmd.rs` | Checkpoint operation requests | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/checkpoint_event.rs` | Checkpoint operation results | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/checkpoint_state.rs` | Checkpoint status: requested, created, or refused | `magistr-cockpit/src/cockpit/checkpoint.tests.rs` |
| `magistr-cockpit/src/cockpit/cmd.rs` | What `update` asks the runtime to do | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/completion.rs` | The completion popup over the prompt | `magistr-cockpit/src/cockpit/completion.tests.rs` |
| `magistr-cockpit/src/cockpit/completion_kind.rs` | What a completion popup completes | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/copy.rs` | Shared quantity formatting and copy vocabulary: `2.07 USD`, `1.84 / 12.00`, `04:12`, and `12 min` / `4 h` / `2 d` | `magistr-cockpit/src/cockpit/copy.tests.rs`
| `magistr-cockpit/src/cockpit/dag_layout.rs` | Deterministic layered DAG layout with virtual routing vertices and crossing reduction | `magistr-cockpit/src/cockpit/dag_layout.tests.rs` |
| `magistr-cockpit/src/cockpit/dag_node.rs` | A positioned roadmap node with typed ChunkState | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/dag_path.rs` | A routed dependency edge through the layered roadmap layout | `magistr-cockpit/src/cockpit/dag_layout.tests.rs` |
| `magistr-cockpit/src/cockpit/decision.rs` | Permission and confirmation interaction: the decision state (conditional register, scroll, seen, stood-down), the semantic intents, the one reducer every key, click and `Msg::Decision` reaches, and the moment/review/ask selectors | `magistr-cockpit/src/cockpit/decision.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/diff_layout.rs` | Width thresholds for diff display modes | `magistr-cockpit/src/cockpit/diff_layout.tests.rs` |
| `magistr-cockpit/src/cockpit/density.rs` | Row height and padding for comfortable and compact density | `magistr-cockpit/src/cockpit/settings.tests.rs` |
| `magistr-cockpit/src/cockpit/dock.rs` | State for pending user actions in the dock | `magistr-cockpit/src/cockpit/dock.tests.rs` |
| `magistr-cockpit/src/cockpit/dock_item.rs` | One card on the Needs-you dock | `magistr-cockpit/src/cockpit/dock_item.tests.rs` |
| `magistr-cockpit/src/cockpit/eased.rs` | A value that glides to its target instead of jumping there | `magistr-cockpit/src/cockpit/eased.tests.rs` |
| `magistr-cockpit/src/cockpit/edge_kind.rs` | Which of the two ways a dependency edge is drawn | `magistr-cockpit/src/cockpit/edge_kind.tests.rs` |
| `magistr-cockpit/src/cockpit/engine_cmd.rs` | What the cockpit asks of the think-and-ship engines | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/fault.rs` | Failure card content, severity, actions, related history, and blocking state | `magistr-cockpit/src/cockpit/fault.tests.rs` |
| `magistr-cockpit/src/cockpit/fault_verb.rs` | Failure card action label, widget ID, and emphasis | `magistr-cockpit/src/cockpit/fault.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_composer.rs` | Bounded pixel message editor with attachments, submission state, and complete border | `magistr-cockpit/src/cockpit/flight_composer.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_conversation.rs` | Proportional conversation, grouped speakers, and actionable work cards | `magistr-cockpit/src/cockpit/flight_conversation.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_divider.rs` | Shared divider geometry, minimums and visible grab areas | `magistr-cockpit/src/cockpit/flight_panels.tests.rs` · `magistr-tui/tests/panel_resize.rs` |
| `magistr-cockpit/src/cockpit/flight_dock.rs` | Pixel permission requests and pinned command output above the message area | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_inspector.rs` | Selected reasoning steps and related work with independent pixel detail scrolling | `magistr-cockpit/src/cockpit/flight_inspector.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_layout.rs` | Shared physical allocations for the workspace, wrapping, and viewport calculations | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_overlays.rs` | Pixel permission cards, command palette, and hover help | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_panels.rs` | Session proportions and captured mouse/keyboard panel resizing | `magistr-cockpit/src/cockpit/flight_panels.tests.rs` · `magistr-tui/tests/panel_resize.rs` |
| `magistr-cockpit/src/cockpit/flight_recorder.rs` | Bounded dynamic reasoning graph, recorded question summaries, and selection controls | `magistr-cockpit/src/cockpit/flight_recorder.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_records.rs` | Recorded-work and roadmap lists with shared physical row geometry | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_roadmap.rs` | Readable roadmap neighborhood with shared physical points, labels, and hit targets | `magistr-cockpit/src/cockpit/flight_roadmap.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_roadmap_graph.rs` | Complete dependency graph with measured labels, physical pointer targets, and camera navigation | `magistr-cockpit/src/cockpit/flight_roadmap_graph.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_requests.rs` | Measured request history projected from real conversation source ranges, with independent scrolling | `magistr-cockpit/src/cockpit/roadmap_request.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_roadmap_page.rs` | Dedicated roadmap page with work list, selected details, and preserved conversation state | `magistr-cockpit/src/cockpit/view.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_secondary.rs` | Settings, search, and keyboard-help overlays above the live workspace | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_shell.rs` | Production conversation and reasoning workspace | `magistr-cockpit/src/cockpit/flight_shell.tests.rs` |
| `magistr-cockpit/src/cockpit/flight_style.rs` | Day/night colors and pixel panel, text, and spacing helpers | `magistr-cockpit/src/cockpit/flight_style.tests.rs` |
| `magistr-cockpit/src/cockpit/investigation.rs` | Recorded question membership, outcomes, exact merge destinations, dependencies, and legacy merge inference | `magistr-cockpit/src/cockpit/investigation.tests.rs` |
| `magistr-cockpit/src/cockpit/frame_clock.rs` | The only clock `view` ever sees | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/freshness.rs` | Whether the projection is current, stale after a gap or a new owner, or unavailable | `magistr-cockpit/src/cockpit/projection.tests.rs` |
| `magistr-cockpit/src/cockpit/galaxy.rs` | Native project overview state: projects, selection, and zoom | `magistr-cockpit/src/cockpit/galaxy.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/galaxy_data.rs` | Project overview data and total spend | `magistr-cockpit/src/cockpit/galaxy.tests.rs`
| `magistr-cockpit/src/cockpit/galaxy_project.rs` | Project metrics for the overview | —
| `magistr-cockpit/src/cockpit/gate_led.rs` | One quality gate on the status strip | `magistr-cockpit/src/cockpit/gate_led.tests.rs` |
| `magistr-cockpit/src/cockpit/ground.rs` | Project setup location: an existing folder, a clone, or a new folder | `magistr-cockpit/src/cockpit/ground.tests.rs` |
| `magistr-cockpit/src/cockpit/ground_field.rs` | Active project setup field: path, clone URL, or new folder | `magistr-cockpit/src/cockpit/birth.tests.rs` |
| `magistr-cockpit/src/cockpit/hover.rs` | Hovered widget state | `magistr-cockpit/src/cockpit/hover.tests.rs` |
| `magistr-cockpit/src/cockpit/hover_role.rs` | Widget hover appearance | `magistr-cockpit/src/cockpit/hover_role.tests.rs` |
| `magistr-cockpit/src/cockpit/interview.rs` | Project setup questions, suggested answers, quality gates, and permissions | `magistr-cockpit/src/cockpit/interview.tests.rs` |
| `magistr-cockpit/src/cockpit/launch_action.rs` | What the launcher asks the entry point to start | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/launcher_data.rs` | What the launcher's cards show, delivered by one job | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/launcher_session.rs` | One recent session as the launcher's card lists it | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/kill_cmd.rs` | Cancellation requests for an ACP terminal by ID or a running gate by name | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/loop_cmd.rs` | What the user asks of the iteration loop | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/loop_event.rs` | What the iteration-loop driver reports | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/loop_stage.rs` | Where the loop driver stands within an iteration | `magistr-cockpit/src/cockpit/loop_stage.tests.rs` · `magistr-cockpit/src/cockpit/bad_day.tests.rs` |
| `magistr-cockpit/src/cockpit/loop_status.rs` | Where the iteration loop stands | `magistr-cockpit/src/cockpit/loop_status.tests.rs` |
| `magistr-cockpit/src/cockpit/loop_verdict.rs` | Iteration review outcome | `magistr-cockpit/src/cockpit/loop_verdict.tests.rs` |
| `magistr-cockpit/src/cockpit/marked.rs` | A line of text with vocabulary glyphs in it, built once for both kinds of terminal | `magistr-cockpit/src/cockpit/marked.tests.rs` |
| `magistr-cockpit/src/cockpit/inspection.rs` | Renderer-neutral selected-record text and actions | `magistr-cockpit/src/cockpit/flight_inspector.tests.rs` · `magistr-gui2/tests/workspace.rs` |
| `magistr-cockpit/src/cockpit/panel_allocation.rs` | Bounded native panel measurements for shared resizing | `magistr-cockpit/src/cockpit/flight_panels.tests.rs` |
| `magistr-cockpit/src/cockpit/inspector.rs` | Shared selected-record content, disclosure and field scrolling | `magistr-cockpit/src/cockpit/view.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/pane.rs` | Terminal keyboard focus across rail, stage and inspector | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/selected.rs` | Stable shared chunk, step or change selection | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/model.rs` | The cockpit's state | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/motion.rs` | Everything on the cockpit that moves, as state the tick advances | `magistr-cockpit/src/cockpit/motion.tests.rs` |
| `magistr-cockpit/src/cockpit/motion_key.rs` | Model values used to detect changes that trigger animations | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/motion_name.rs` | Animation names and their day/night behavior | `magistr-cockpit/src/cockpit/motion_name.tests.rs` |
| `magistr-cockpit/src/cockpit/msg.rs` | Everything that can change the model | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/nav.rs` | Escape-key navigation through overlays, viewers, transient screens, and parent views | `magistr-cockpit/src/cockpit/nav.tests.rs` |
| `magistr-cockpit/src/cockpit/palette.rs` | Command palette state and filtered results | `magistr-cockpit/src/cockpit/palette.tests.rs` |
| `magistr-cockpit/src/cockpit/palette_row.rs` | Command palette result and its dispatched action | `magistr-cockpit/src/cockpit/palette.tests.rs` |
| `magistr-cockpit/src/cockpit/path_stop.rs` | Navigation breadcrumb derived from the current view | —
| `magistr-cockpit/src/cockpit/permission.rs` | A permission request waiting for the user's answer | `magistr-cockpit/src/cockpit/permission.tests.rs` |
| `magistr-cockpit/src/cockpit/phase.rs` | Where the iteration stands, as the engine feed tells it | `magistr-cockpit/src/cockpit/phase.tests.rs` |
| `magistr-cockpit/src/cockpit/place.rs` | The native window's navigation location in the shared model | `magistr-cockpit/src/cockpit/place.tests.rs` |
| `magistr-cockpit/src/cockpit/plan_task.rs` | One task of the ship plan, as the rail lists it | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/pointer_shape.rs` | The mouse pointer's shape, as kitty's OSC 22 names it | `magistr-cockpit/src/cockpit/pointer_shape.tests.rs` |
| `magistr-cockpit/src/cockpit/progress.rs` | What the terminal's own progress indicator is told | `magistr-cockpit/src/cockpit/progress.tests.rs` |
| `magistr-cockpit/src/cockpit/projection.rs` | How the model stands to the store: freshness and the last delivery folded | `magistr-cockpit/src/cockpit/projection.tests.rs` |
| `magistr-cockpit/src/cockpit/request_phase.rs` | Typed waiting, queued, active and terminal request states | `magistr-cockpit/src/cockpit/roadmap_request.tests.rs` |
| `magistr-cockpit/src/cockpit/prompt.rs` | The prompt: a multi-line editor with history | `magistr-cockpit/src/cockpit/prompt.tests.rs` |
| `magistr-cockpit/src/cockpit/proposal.rs` | Proposed roadmap items and workstreams, including ordering and names | `magistr-cockpit/src/cockpit/proposal.tests.rs` |
| `magistr-cockpit/src/cockpit/proposal_kind.rs` | Which field of a chunk an open proposal wants to change | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/proposed_chunk.rs` | Proposed roadmap item passed to `roadmap_add_chunk` on acceptance | `magistr-cockpit/src/cockpit/proposed_chunk.tests.rs` |
| `magistr-cockpit/src/cockpit/rail.rs` | Navigation and execution-plan rail state | `magistr-cockpit/src/cockpit/rail.tests.rs` |
| `magistr-cockpit/src/cockpit/rail_mark.rs` | Terminal output timing markers derived from arrival ticks: second boundaries, pauses, and elapsed timestamps | `magistr-cockpit/src/cockpit/rail_mark.tests.rs` |
| `magistr-cockpit/src/cockpit/roadmap.rs` | Roadmap state and deterministic dependency-layer layout with connected-node alignment | `magistr-cockpit/src/cockpit/roadmap.tests.rs` |
| `magistr-cockpit/src/cockpit/roadmap_request.rs` | Submitted roadmap messages with admission, outcome and real reply source ranges | `magistr-cockpit/src/cockpit/roadmap_request.tests.rs` |
| `magistr-cockpit/src/cockpit/roadmap_chunk.rs` | Roadmap item projection with typed ChunkState and stable chunk identity | `magistr-cockpit/src/cockpit/roadmap_chunk.tests.rs` |
| `magistr-cockpit/src/cockpit/roadmap_sort.rs` | Sortable roadmap columns and their header targets | `magistr-cockpit/src/cockpit/roadmap.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/rule_mode.rs` | How a charter rule answers — the ASK / AUTO / OFF badge vocabulary | `magistr-cockpit/src/cockpit/rule_mode.tests.rs` |
| `magistr-cockpit/src/cockpit/scaffold_kind.rs` | Project setup artifact types and their displayed completion status | `magistr-cockpit/src/cockpit/scaffold_kind.tests.rs` |
| `magistr-cockpit/src/cockpit/search.rs` | Search state, text matching, excerpts, and result limits | `magistr-cockpit/src/cockpit/search.tests.rs` |
| `magistr-cockpit/src/cockpit/search_hit.rs` | Search result metadata and destination | `magistr-cockpit/src/cockpit/search.tests.rs` |
| `magistr-cockpit/src/cockpit/search_kind.rs` | Search result categories and display order | `magistr-cockpit/src/cockpit/search.tests.rs` |
| `magistr-cockpit/src/cockpit/search_results.rs` | What a query found — the rows shown and the total before any cut | `magistr-cockpit/src/cockpit/search.tests.rs` |
| `magistr-cockpit/src/cockpit/search_scope.rs` | Search category filter | `magistr-cockpit/src/cockpit/search_scope.tests.rs` |
| `magistr-cockpit/src/cockpit/session.rs` | Session, transcript and draft interaction: the session state (the turn, where it stands, the draft, the completion popup, the reading position), the agent and frame folds that consume the supervisor's and the store's outcomes by reference, the draft keys, the one send and the one stop the keys and the composer button share, transcript selection, and the line-based anchor a pinned window is held by across a re-wrap | `magistr-cockpit/src/cockpit/session.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-tui/src/flat/transcript.tests.rs` |
| `magistr-cockpit/src/cockpit/session_agent.rs` | Agent identity and cost summary | `magistr-cockpit/src/cockpit/session_agent.tests.rs` |
| `magistr-cockpit/src/cockpit/session_outcome.rs` | How a recorded session ended | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/settings.rs` | Settings navigation, filtering, and keyboard selection | `magistr-cockpit/src/cockpit/settings.tests.rs` |
| `magistr-cockpit/src/cockpit/settings_data.rs` | Settings values supplied by the runtime | `magistr-cockpit/src/cockpit/settings.tests.rs` |
| `magistr-cockpit/src/cockpit/settings_group.rs` | Settings groups, scopes, source files, and read-only status | `magistr-cockpit/src/cockpit/settings_group.tests.rs` |
| `magistr-cockpit/src/cockpit/settings_scope.rs` | Application, project, and team settings scopes | `magistr-cockpit/src/cockpit/settings_group.tests.rs` |
| `magistr-cockpit/src/cockpit/shelf.rs` | Completed item artifacts: commit, checks, cost, reasoning, execution records, and images | `magistr-cockpit/src/cockpit/shelf.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/shelf_cmd.rs` | Artifact collection, export, and report-sharing requests | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/shelf_still.rs` | Artifact image metadata and decoded pixels | `magistr-cockpit/src/shelf/load_still.tests.rs` |
| `magistr-cockpit/src/cockpit/session_item.rs` | Timestamp-ordered session items and stable inspector selection | `magistr-cockpit/src/cockpit/session_item.tests.rs` |
| `magistr-cockpit/src/cockpit/ship_moment.rs` | Recorded actions and checks, preserving verified versus self-reported evidence | `magistr-cockpit/src/cockpit/ship_moment.tests.rs` |
| `magistr-cockpit/src/cockpit/ship_run.rs` | The ship moment in flight | `magistr-cockpit/src/cockpit/ship_run.tests.rs` |
| `magistr-cockpit/src/cockpit/ship_view.rs` | The ship objective and its plan, as the rail shows them | `magistr-cockpit/src/cockpit/ship_view.tests.rs` |
| `magistr-cockpit/src/cockpit/signal_row.rs` | One stakeholder signal as the Roadmap tab's inbox holds it | `magistr-cockpit/src/cockpit/signal_row.tests.rs` |
| `magistr-cockpit/src/cockpit/size.rs` | The terminal's size in cells | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/space.rs` | The cockpit's spacing scale | `magistr-cockpit/src/cockpit/space.tests.rs` |
| `magistr-cockpit/src/cockpit/spine.rs` | Checkpoint ledger state | `magistr-cockpit/src/cockpit/spine.tests.rs` |
| `magistr-cockpit/src/cockpit/stage_tab.rs` | Which surface the stage shows | `magistr-cockpit/src/cockpit/stage_tab.tests.rs` |
| `magistr-cockpit/src/cockpit/star_state.rs` | Roadmap node display states | `magistr-cockpit/src/cockpit/star_state.tests.rs` |
| `magistr-cockpit/src/cockpit/state_word.rs` | Shared status labels: Ready, Executing, Review, Stuck, Blocked, Done, and Idle | `magistr-cockpit/src/cockpit/state_word.tests.rs`
| `magistr-cockpit/src/cockpit/still_kind.rs` | What kind of picture a shelf still is — a still, or a filmstrip from a `motion` golden directory | `magistr-cockpit/src/cockpit/still_kind.tests.rs` |
| `magistr-cockpit/src/cockpit/summary.rs` | What a session amounted to, for the scrollback | `magistr-cockpit/src/cockpit/summary.tests.rs` |
| `magistr-cockpit/src/cockpit/surface.rs` | Which kind of screen the cockpit is drawn on — `view` arranges the same widget ids for a terminal or a window | `magistr-cockpit/src/cockpit/surface.tests.rs` · `magistr-cockpit/src/cockpit/view.tests.rs` |
| `magistr-cockpit/src/cockpit/survey_kind.rs` | Project detection categories: stack, tests, Git, and agent | `magistr-cockpit/src/cockpit/survey_kind.tests.rs` |
| `magistr-cockpit/src/cockpit/survey_report.rs` | Project detection results and suggested setup answers | `magistr-cockpit/src/cockpit/survey_report.tests.rs` |
| `magistr-cockpit/src/cockpit/task_state.rs` | Where one planned task stands, as `ship_status` and the ship frames name it | `magistr-cockpit/src/cockpit/task_state.tests.rs` |
| `magistr-cockpit/src/cockpit/term.rs` | Terminal screen model with a vt100 parser, 10,000 scrollback rows, row timestamps, and viewport state | `magistr-cockpit/src/cockpit/term.tests.rs` |
| `magistr-cockpit/src/cockpit/term_cell.rs` | Terminal cell text, colors, and attributes | `magistr-cockpit/src/cockpit/term.tests.rs` |
| `magistr-cockpit/src/cockpit/term_hold.rs` | Press-and-hold cancellation state and progress, driven by model ticks | `magistr-cockpit/src/cockpit/term_hold.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/term_dock.rs` | Terminal dock selection, size, drag, maximize, and animation state | `magistr-cockpit/src/cockpit/term_dock.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/term_signals.rs` | Terminal progress reports decoded from ConEmu OSC 9;4 sequences | `magistr-cockpit/src/cockpit/term_signals.tests.rs` |
| `magistr-cockpit/src/cockpit/terminals.rs` | Terminal screen models indexed by ACP terminal ID or gate name | `magistr-cockpit/src/cockpit/terminals.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/theme_choice.rs` | Dark, light, and system theme selection | `magistr-cockpit/src/cockpit/settings.tests.rs` |
| `magistr-cockpit/src/cockpit/tooltip.rs` | Tooltip content, timing, and placement | `magistr-cockpit/src/cockpit/tooltip.tests.rs` |
| `magistr-cockpit/src/cockpit/trace.rs` | Reasoning steps, persisted and live branch lifecycle, exact merge links, selection, recorded work, and checkpoints | `magistr-cockpit/src/cockpit/trace.tests.rs` |
| `magistr-cockpit/src/cockpit/trace_card.rs` | The rows a selected step's card takes — brief or full, wrapped to the width the scroll counts | `magistr-cockpit/src/cockpit/trace_card.tests.rs` |
| `magistr-cockpit/src/cockpit/trace_focus.rs` | A selected step's ancestors and descendants — what the graph lights | `magistr-cockpit/src/cockpit/trace_focus.tests.rs` |
| `magistr-cockpit/src/cockpit/trace_step.rs` | One reasoning step as the think feed carries it | `magistr-cockpit/src/cockpit/trace_step.tests.rs` |
| `magistr-cockpit/src/cockpit/turn_state.rs` | Where the turn stands | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/update.rs` | Folding one message into the model | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/cockpit/view.rs` | The model as a layout tree | `magistr-cockpit/src/cockpit/view.tests.rs` |
| `magistr-cockpit/src/cockpit/widget.rs` | The widgets of the cockpit and their ids | `magistr-cockpit/src/cockpit/widget.tests.rs` |
| `magistr-cockpit/src/cockpit/workbench_cmd.rs` | What the cockpit asks of the workbench | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/cockpit/work.rs` | Roadmap selection, navigation, graph/list interaction and chunk commands | `magistr-cockpit/src/cockpit/work.tests.rs` |
| `magistr-cockpit/src/cockpit/workbench_event.rs` | Workbench outcomes; a refusal retains the command it answers | `magistr-cockpit/src/cockpit/update.tests.rs` · `magistr-cockpit/tests/purity.rs` |

### Workflow ports and host clients

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cli/src/application/workflow_client/mod.rs` | Host workflow clients, concrete ports and feed delivery | `magistr-cli/src/application/workflow_client/client.tests.rs` |
| `magistr-cli/src/application/workspace_client/mod.rs` | Host MCP workspace connection and journal adapter | `magistr-cli/src/application/workspace_client/mcp_workbench.tests.rs` |
| `magistr-cockpit/src/engine/mod.rs` | Semantic workflow ports and events | `magistr-tui/tests/jobs_engines_harness.rs` |
| `magistr-cockpit/src/engine/apply.rs` | Carrying an engine command out on a port | `magistr-cockpit/src/engine/apply.tests.rs` |
| `magistr-cli/src/application/workflow_client/attached_port.rs` | The engine port over an owner this process did not open | `magistr-cli/src/application/workflow_client/client.tests.rs` |
| `magistr-cli/src/application/workflow_client/client.rs` | The engines, opened before the terminal is taken over | `magistr-cli/src/application/workflow_client/client.tests.rs` |
| `magistr-cockpit/src/engine/describe.rs` | What an engine command asked, in the words a refusal names | `magistr-cockpit-runtime/src/session.tests.rs` |
| `magistr-cli/src/application/workflow_client/error.rs` | Why the engines could not open | `magistr-cli/src/application/workflow_client/client.tests.rs` |
| `magistr-cockpit/src/engine/event.rs` | What the feed — or the cockpit's own engine port — reports | `magistr-tui/tests/jobs_engines_harness.rs` |
| `magistr-cli/src/application/workflow_client/feed.rs` | The broadcast socket decoded at the edge into typed events with their delivery, relayed onto the inbox | `magistr-cli/src/application/workflow_client/feed.tests.rs` |
| `magistr-cli/src/application/workflow_client/feed_link.rs` | What the feed tells the re-seed: attached or not, and a snapshot asked for | `magistr-cli/src/application/workflow_client/feed_link.tests.rs` |
| `magistr-cli/src/application/workflow_client/native_port.rs` | The engine port over the in-process engines | `magistr-cli/src/application/workflow_client/native_port.tests.rs` |
| `magistr-cockpit/src/engine/port.rs` | What the cockpit asks of the think-and-ship engines | `magistr-tui/tests/jobs_engines_harness.rs` |
| `magistr-cli/src/application/workflow_client/reseed.rs` | Re-reading the store as one snapshot on a timer while no feed is attached, or when the feed asks — confirming an unchanged store when it asked | `magistr-cli/src/application/workflow_client/reseed.tests.rs` |

### `golden` — the PNG review flow

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/golden/mod.rs` | Shared comparison of rendered frames against reference PNGs | `magistr-cockpit/src/golden/directory.tests.rs` |
| `magistr-cockpit/src/golden/directory.rs` | A directory of golden PNGs, one per named frame | `magistr-cockpit/src/golden/directory.tests.rs` |
| `magistr-cockpit/src/golden/error.rs` | How a golden comparison ends other than with a match | `magistr-cockpit/src/golden/directory.tests.rs` |

### `shelf` — the deliverables shelf's three effects

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/shelf/mod.rs` | Artifact image loading, bundle export, and HTML report generation | `magistr-cockpit/tests/purity.rs` |
| `magistr-cockpit/src/shelf/bundle.rs` | Artifact bundle containing execution records and reference images | `magistr-cockpit/src/shelf/bundle.tests.rs` |
| `magistr-cockpit/src/shelf/page.rs` | Self-contained HTML report generation | `magistr-cockpit/src/shelf/page.tests.rs` |
| `magistr-cockpit/src/shelf/load_still.rs` | A golden on disk decoded into a still, through the cockpit's own picture decoder | `magistr-cockpit/src/shelf/load_still.tests.rs` |

### `harness` — the cockpit's own fakes and scenes

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/harness/mod.rs` | Shared mock services and interface test scenes | `magistr-cockpit/src/cockpit/view.tests.rs` |
| `magistr-cockpit/src/harness/input_journey.rs` | Shared adapter journey for navigation, draft retention, send/stop and permission refusal | `magistr-tui/src/input/decoder.tests.rs`, `magistr-gui2/src/input.tests.rs` |
| `magistr-cockpit/src/harness/fake_workbench.rs` | A scripted workbench for the harness | `magistr-cockpit/src/harness/fake_workbench.tests.rs` |
| `magistr-cockpit/src/harness/fixtures.rs` | Typed store records and events for scenes and tests, filled from partial JSON at the wire's edge | `magistr-cockpit/src/cockpit/view.tests.rs` · `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/harness/scenes.rs` | Shared scenes for view tests, screenshots, and animation filmstrips | `magistr-tui/tests/goldens.rs` · `magistr-cockpit/src/cockpit/view.tests.rs` |

### `hit` — mouse hit-testing

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/hit/mod.rs` | Mouse hit-testing: screen rects to widget ids | `magistr-cockpit/src/hit/registry.tests.rs` |
| `magistr-cockpit/src/hit/registry.rs` | Which widget is under a cell | `magistr-cockpit/src/hit/registry.tests.rs` |

### `input` — shared key and pointer values

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/input/mod.rs` | Surface-neutral keys and pointer values | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/button.rs` | A mouse button | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/key_code.rs` | Which key | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/key_event.rs` | One key report | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/key_kind.rs` | Press, repeat or release | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/modifiers.rs` | Modifier keys held during a key or mouse event | `magistr-cockpit/src/input/modifiers.tests.rs` |
| `magistr-cockpit/src/input/mouse_event.rs` | One mouse report | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-cockpit/src/input/mouse_kind.rs` | What the mouse did | `magistr-tui/src/input/decoder.tests.rs` |

### `job` — work off the loop thread

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/job/mod.rs` | Job identities and results consumed by the model | `magistr-tui/tests/jobs_engines_harness.rs` |
| `magistr-cockpit/src/job/job_id.rs` | The name of a job | `magistr-tui/tests/jobs_engines_harness.rs` |
| `magistr-cockpit-runtime/src/jobs.rs` | Spawning work off the loop thread | `magistr-cockpit-runtime/src/jobs.tests.rs` |
| `magistr-cockpit/src/job/output.rs` | What a job produced | `magistr-tui/tests/jobs_engines_harness.rs` |

### `layout` — flexbox over the cell grid

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/layout/mod.rs` | Flexbox layout over the cell grid, or over a window's pixels, from the same tree | `magistr-cockpit/src/layout/tree.tests.rs` |
| `magistr-cockpit/src/layout/align.rs` | Where a container puts its children on the cross axis | `magistr-cockpit/src/layout/align.tests.rs` |
| `magistr-cockpit/src/layout/basis.rs` | How many layout units one cell is worth on each axis | `magistr-cockpit/src/layout/basis.tests.rs` |
| `magistr-cockpit/src/layout/cell_measure.rs` | Text width measurement in terminal columns | `magistr-cockpit/src/layout/cell_measure.tests.rs` |
| `magistr-cockpit/src/layout/error.rs` | The one way layout can fail | `magistr-cockpit/src/layout/error.tests.rs` |
| `magistr-cockpit/src/layout/extent.rs` | The size of a node's content, scrolled or not | `magistr-cockpit/src/layout/tree.tests.rs` |
| `magistr-cockpit/src/layout/frame.rs` | Node border style | `magistr-cockpit/src/layout/frame.tests.rs` |
| `magistr-cockpit/src/layout/inset.rs` | How far an overlay sits from its parent's edges | `magistr-cockpit/src/layout/inset.tests.rs` |
| `magistr-cockpit/src/layout/justify.rs` | How a container distributes its children along the main axis | `magistr-cockpit/src/layout/justify.tests.rs` |
| `magistr-cockpit/src/layout/length.rs` | How a node sizes itself along one axis | `magistr-cockpit/src/layout/length.tests.rs` |
| `magistr-cockpit/src/layout/mark.rs` | What a node asks the pixel layer to draw over its cells | `magistr-cockpit/src/layout/mark.tests.rs` |
| `magistr-cockpit/src/layout/marks.rs` | Every mark on a laid-out tree, with the rect layout gave its node | `magistr-cockpit/src/layout/marks.tests.rs` |
| `magistr-cockpit/src/layout/measure.rs` | Text measurement interface | `magistr-cockpit/src/layout/cell_measure.tests.rs` |
| `magistr-cockpit/src/layout/node.rs` | A declarative description of one node in the layout tree | `magistr-cockpit/src/layout/node.tests.rs` |
| `magistr-cockpit/src/layout/placed.rs` | One node's place on screen after layout | `magistr-cockpit/src/layout/tree.tests.rs` |
| `magistr-cockpit/src/layout/px_rect.rs` | An axis-aligned rectangle in the engine's own units | `magistr-cockpit/src/layout/px_rect.tests.rs` |
| `magistr-cockpit/src/layout/rect.rs` | An axis-aligned rectangle of cells | `magistr-cockpit/src/layout/rect.tests.rs` |
| `magistr-cockpit/src/layout/resolved.rs` | The result of a layout pass | `magistr-cockpit/src/layout/resolved.tests.rs` |
| `magistr-cockpit/src/layout/text.rs` | Measuring and wrapping text in cell columns | `magistr-cockpit/src/layout/text.tests.rs` |
| `magistr-cockpit/src/layout/text_role.rs` | Text roles for renderer typography | `magistr-cockpit/src/layout/text_role.tests.rs` |
| `magistr-cockpit/src/layout/tree.rs` | Resolving a `Node` tree into rects with taffy's flexbox | `magistr-cockpit/src/layout/tree.tests.rs` |

### `pixel` — the pixel chrome

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/pixel/mod.rs` | The pixel chrome: widgets that paint RGBA through tiny-skia into a `PixelSurface` sized in cells | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-cockpit/src/pixel/branch_graph.rs` | The think trace's branches as a small commit-style graph | `magistr-cockpit/src/pixel/branch_graph.tests.rs` |
| `magistr-cockpit/src/pixel/brand.rs` | The product mark as a vector | `magistr-cockpit/src/pixel/brand.tests.rs` |
| `magistr-cockpit/src/pixel/cell_size.rs` | One terminal cell in pixels: what a surface's size in cells scales by | `magistr-cockpit/src/pixel/cell_size.tests.rs` |
| `magistr-cockpit/src/pixel/chip.rs` | A key cap as a drawn chip | `magistr-cockpit/src/pixel/chip.tests.rs` |
| `magistr-cockpit/src/pixel/gauge.rs` | A fraction as a smooth bar | `magistr-cockpit/src/pixel/gauge.tests.rs` |
| `magistr-cockpit/src/pixel/icon.rs` | The vocabulary as vector glyphs, painted with transparency | `magistr-cockpit/src/pixel/icon.tests.rs` |
| `magistr-cockpit/src/pixel/image.rs` | An agent's image, scaled into the cells the stream gave it | `magistr-cockpit/src/pixel/image.tests.rs` |
| `magistr-cockpit/src/pixel/painter.rs` | What paints a surface | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-cockpit/src/pixel/panel.rs` | A framed node's ring as a drawn panel | `magistr-cockpit/src/pixel/panel.tests.rs` |
| `magistr-cockpit/src/pixel/pixel_format.rs` | How the bytes of a packed pixel surface are laid out | `magistr-cockpit/src/pixel/pixel_format.tests.rs` |
| `magistr-cockpit/src/pixel/roadmap_dag.rs` | Roadmap pixels with cycle-safe visual transitive reduction; stored dependencies remain intact | `magistr-cockpit/src/pixel/roadmap_dag.tests.rs` |
| `magistr-cockpit/src/pixel/rounded.rs` | The rounded rectangle every drawn chip, segment and label is made of | `magistr-cockpit/src/pixel/rounded.tests.rs` |
| `magistr-cockpit/src/pixel/rule.rs` | A soft horizontal rule | `magistr-cockpit/src/pixel/rule.tests.rs` |
| `magistr-cockpit/src/pixel/segment.rs` | One segment of a drawn segmented control | `magistr-cockpit/src/pixel/segment.tests.rs` |
| `magistr-cockpit/src/pixel/sparkline.rs` | A series of values as a line over a filled area | `magistr-cockpit/src/pixel/sparkline.tests.rs` |
| `magistr-cockpit/src/pixel/typography.rs` | Shared embedded proportional and monospace font metrics for wrapping, layout, and painting | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/pixel/surface.rs` | A rectangle of cells that a widget paints in pixels | `magistr-cockpit/src/pixel/surface.tests.rs` |

### `render` — the cell vocabulary and painter

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/render/mod.rs` | The cell vocabulary and the cell painter: a styled grapheme, a grid of them, a run of scaled text, and `paint` putting a laid-out view into the grid | `magistr-cockpit/src/render/paint.tests.rs` |
| `magistr-cockpit/src/render/attrs.rs` | Text attributes as a small bit set | `magistr-cockpit/src/render/attrs.tests.rs` |
| `magistr-cockpit/src/render/buffer.rs` | A grid of cells with wide-grapheme bookkeeping and no out-of-range panics | `magistr-cockpit/src/render/buffer.tests.rs` |
| `magistr-cockpit/src/render/cell.rs` | One grid cell: a grapheme cluster and its style | `magistr-cockpit/src/render/cell.tests.rs` |
| `magistr-cockpit/src/render/color.rs` | A terminal color: the terminal's default, one of the 256 indexed colors, or 24-bit RGB | `magistr-tui/tests/end_to_end.rs` |
| `magistr-cockpit/src/render/headline.rs` | A run of scaled text placed on the grid: one headline | `magistr-cockpit/src/render/headline.tests.rs` |
| `magistr-cockpit/src/render/paint.rs` | Putting a laid-out view into the cell buffer | `magistr-cockpit/src/render/paint.tests.rs` |
| `magistr-cockpit/src/render/resolve.rs` | A terminal colour as the pixels it paints, through the palette | `magistr-cockpit/src/render/resolve.tests.rs` |
| `magistr-cockpit/src/render/style.rs` | How a cell is painted: colors, attributes and an optional hyperlink | `magistr-tui/tests/end_to_end.rs` |
| `magistr-cockpit/src/render/underline.rs` | The shape an underline is drawn in — SGR 4's colon-parameterized styles | `magistr-cockpit/src/render/underline.tests.rs` |
| `magistr-cockpit/src/render/width.rs` | Display width of text in terminal columns | `magistr-cockpit/src/render/width.tests.rs` |

### `theme` — palette, icons, pens

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/theme/mod.rs` | Theme detection from terminal appearance and `MAGISTR_THEME`, with shared palettes and style pens | `magistr-cockpit/src/theme/pen.tests.rs` |
| `magistr-cockpit/src/theme/decision.rs` | Which palette the cockpit draws with, and where that decision came from | `magistr-cockpit/src/theme/decision.tests.rs` |
| `magistr-cockpit/src/theme/icon_mode.rs` | Which glyph vocabulary the cockpit draws with | `magistr-cockpit/src/theme/icon_mode.tests.rs` |
| `magistr-cockpit/src/theme/icons.rs` | The cockpit's glyph vocabulary, in two sets | `magistr-cockpit/src/theme/icons.tests.rs` |
| `magistr-cockpit/src/theme/mode.rs` | Dark or light: the one thing the palette choice depends on | `magistr-cockpit/src/theme/mode.tests.rs` |
| `magistr-cockpit/src/theme/motion_mode.rs` | Whether the cockpit moves | `magistr-cockpit/src/theme/motion_mode.tests.rs` |
| `magistr-cockpit/src/theme/pen.rs` | The cockpit's styles, drawn from one palette | `magistr-cockpit/src/theme/pen.tests.rs` |
| `magistr-cockpit/src/theme/severity.rs` | Severity colors for card backgrounds and headings | `magistr-cockpit/src/theme/severity.tests.rs` |

### `turn` — the agent's turn as lines and rows

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/turn/mod.rs` | The agent's turn as the stage shows it: source lines and the rows they wrap to at the stage width | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/accent.rs` | Which role's accent bar a turn line carries | `magistr-cockpit/src/turn/accent.tests.rs` |
| `magistr-cockpit/src/turn/anchor.rs` | Where in the turn's content a window is held | `magistr-cockpit/src/turn/anchor.tests.rs` |
| `magistr-cockpit/src/turn/diff.rs` | The edit card's diff body, from the ACP diff's old and new text | `magistr-cockpit/src/turn/diff.tests.rs` |
| `magistr-cockpit/src/turn/diff_kind.rs` | What a line of a unified diff is, for the pen | `magistr-cockpit/src/turn/diff_kind.tests.rs` |
| `magistr-cockpit/src/turn/highlight.rs` | Syntax highlighting for one fenced block, one closed line at a time | `magistr-cockpit/src/turn/highlight.tests.rs` |
| `magistr-cockpit/src/turn/hunk.rs` | A unified diff body, read as hunks a viewer can draw | `magistr-cockpit/src/turn/hunk.tests.rs` |
| `magistr-cockpit/src/turn/hunk_row.rs` | One line of a hunk: its side, its line numbers, its text in runs | `magistr-cockpit/src/turn/hunk.tests.rs` |
| `magistr-cockpit/src/turn/hunk_span.rs` | One run of a diff row's text, by whether it is what changed | `magistr-cockpit/src/turn/hunk.tests.rs` |
| `magistr-cockpit/src/turn/image.rs` | A decoded image block of the stream | `magistr-cockpit/src/turn/image.tests.rs` |
| `magistr-cockpit/src/turn/inline.rs` | Inline markdown on one line: markers stripped, spans over what stays | `magistr-cockpit/src/turn/inline.tests.rs` |
| `magistr-cockpit/src/turn/line.rs` | One source line of a turn | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/line_kind.rs` | What a source line is | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/receipt.rs` | Completed tool-call record | `magistr-cockpit/src/cockpit/update.tests.rs` |
| `magistr-cockpit/src/turn/row.rs` | One row of the stage | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/row_kind.rs` | How a row is styled, and which voice it is set in | `magistr-cockpit/src/turn/row_kind.tests.rs` |
| `magistr-cockpit/src/turn/span.rs` | One styled run within a row's text | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/span_style.rs` | How an inline run is styled | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/stream.rs` | The turn's lines and rows, kept current as the agent streams | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/turn/viewport.rs` | Which rows of the turn the stage shows | `magistr-cockpit/src/turn/viewport.tests.rs` |
| `magistr-cockpit/src/turn/voice.rs` | Who a prose line is from | `magistr-tui/tests/turn_harness.rs` |
| `magistr-cockpit/src/turn/conversation_card.rs` | Readable tool, edit, and check summaries from source records | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/turn/conversation_layout.rs` | Measured conversation rows preserving source identities and scroll anchors | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/turn/conversation_row.rs` | Source row identity and physical bounds for conversation painting and picking | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/turn/pixel_wrap.rs` | Proportional paragraph wrapping with aligned code and output fallback | `magistr-cockpit/src/turn/stream.tests.rs` |
| `magistr-cockpit/src/turn/wrap.rs` | From one source line to the rows it takes at a width | `magistr-cockpit/src/turn/wrap.tests.rs` |

### `workbench` — the journal model's port

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-cockpit/src/workbench/mod.rs` | The workbench behind the cockpit's journal model, as a port | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit/src/workbench/apply.rs` | One workbench command, carried out on a port | `magistr-cockpit/src/workbench/apply.tests.rs` |
| `magistr-cockpit/src/workbench/describe.rs` | What a workbench command asked, in the words a refusal names | `magistr-cockpit-runtime/src/session.tests.rs` |
| `magistr-cockpit/src/workbench/journal_entry.rs` | One journaled edit, as the workbench recorded it | `magistr-cli/tests/session_harness.rs` |
| `magistr-cli/src/application/workspace_client/mcp_client.rs` | One MCP server on the other end of a pipe | `magistr-cli/src/application/workspace_client/mcp_client.tests.rs` |
| `magistr-cli/src/application/workspace_client/mcp_workbench.rs` | Native journal listing and undo through one owner connection; legacy lock-free journal listing | `magistr-cli/src/application/workspace_client/mcp_workbench.tests.rs`, `magistr-cli/tests/cockpit_journal_owner.rs` |
| `magistr-cockpit/src/workbench/port.rs` | What the cockpit asks of a workbench | `magistr-cli/tests/session_harness.rs` |
| `magistr-cockpit/src/workbench/run_row.rs` | One structured row of a workbench run's result | `magistr-cli/tests/session_harness.rs` |

## Module index — `crates/magistr-tui`

### crate root

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/lib.rs` | magistr-tui — the cockpit's terminal renderer | `magistr-tui/tests/boundary.rs` |

### `flat` — the flat (screen-reader) cockpit

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/flat/mod.rs` | The flat cockpit: the session as linear, append-only plain text | `magistr-cli/tests/session_harness.rs` |
| `magistr-tui/src/flat/echo.rs` | Incremental echo of the live prompt, confined to one terminal row | `magistr-tui/src/flat/echo.tests.rs` |
| `magistr-tui/src/flat/flat_line.rs` | One line of the flat cockpit | `magistr-tui/src/flat/flat_line.tests.rs` |
| `magistr-tui/src/flat/lines.rs` | The session as flat lines | `magistr-tui/src/flat/lines.tests.rs` |
| `magistr-tui/src/flat/plain.rs` | Cockpit text with its glyphs taken out | `magistr-tui/src/flat/plain.tests.rs` |
| `magistr-tui/src/flat/role.rs` | Who a flat line is from | `magistr-tui/src/flat/role.tests.rs` |
| `magistr-tui/src/flat/transcript.rs` | Tracks transcript content already written to the terminal | `magistr-tui/src/flat/transcript.tests.rs` |

### `graphics` — the pixel layer's wire side

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/graphics/mod.rs` | The pixel layer's wire side: the two graphics protocols as bytes — the kitty protocol and sixel — tmux DCS passthrough, and the startup probe that decides which of them the cockpit runs on, or whether it runs as plain text | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-tui/src/graphics/decision.rs` | The probe's verdict: which protocol pixels travel by, or why they do not — decided once at startup, never mid-session | `magistr-tui/src/graphics/decision.tests.rs` |
| `magistr-tui/src/graphics/env.rs` | What the environment says about the terminal before any byte is sent | `magistr-tui/src/graphics/env.tests.rs` |
| `magistr-tui/src/graphics/error.rs` | The ways a graphics command can be refused before any byte is written | `magistr-tui/src/graphics/error.tests.rs` |
| `magistr-tui/src/graphics/image_id.rs` | The identity of an image the terminal has stored | `magistr-tui/src/graphics/image_id.tests.rs` |
| `magistr-tui/src/graphics/kitty.rs` | The kitty graphics protocol as bytes | `magistr-tui/src/graphics/kitty.tests.rs` |
| `magistr-tui/src/graphics/palette.rs` | An image reduced to the at-most-256 colour registers sixel allows | `magistr-tui/src/graphics/palette.tests.rs` |
| `magistr-tui/src/graphics/passthrough.rs` | Getting a graphics command past tmux | `magistr-tui/src/graphics/passthrough.tests.rs` |
| `magistr-tui/src/graphics/placement_id.rs` | The identity of one placement of a stored image | `magistr-tui/src/graphics/placement_id.tests.rs` |
| `magistr-tui/src/graphics/probe.rs` | The startup capability probe: one write, one read, a decision | `magistr-tui/src/graphics/probe.tests.rs` |
| `magistr-tui/src/graphics/probe_reply.rs` | What the terminal answered to the startup probe | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-tui/src/graphics/protocol.rs` | Which pixel protocol the cockpit will speak | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-tui/src/graphics/sixel.rs` | An RGBA image as a sixel DCS string | `magistr-tui/src/graphics/sixel.tests.rs` |

### `harness` — the scripted terminal, agent and engines

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/harness/mod.rs` | A terminal, an agent and a think-and-ship feed made of scripts: bytes and resizes that arrive at chosen moments with the output captured for inspection, agent deltas and feed events posted on the runtime's inbox at chosen moments — and, for the session, a real ACP agent in-process on the other end of the transport | the harnesses that drive it |
| `magistr-tui/src/harness/acp_observed.rs` | What the fake ACP agent saw the client do | the harnesses that drive it |
| `magistr-tui/src/harness/capture.rs` | Everything a fake terminal was told | the harnesses that drive it |
| `magistr-tui/src/harness/deliveries.rs` | When a fake terminal handed bytes to the reader | the harnesses that drive it |
| `magistr-tui/src/harness/fake_acp_agent.rs` | An in-process ACP agent for the session: the transport's other end without a subprocess | the harnesses that drive it |
| `magistr-tui/src/harness/fake_agent.rs` | An agent made of a script | `magistr-tui/src/harness/fake_agent.tests.rs` |
| `magistr-tui/src/harness/fake_checkpoints.rs` | A scripted checkpoint port for the harness | `magistr-cli/tests/session_harness.rs` |
| `magistr-tui/src/harness/fake_engine_port.rs` | A scripted engine port for the harness | `magistr-tui/src/harness/fake_engine_port.tests.rs` |
| `magistr-tui/src/harness/fake_engines.rs` | A think-and-ship feed made of a script | `magistr-tui/src/harness/fake_engines.tests.rs` |
| `magistr-tui/src/harness/fake_input.rs` | The input side of a fake terminal: the script, played on the clock | `magistr-tui/src/harness/fake_input.tests.rs` |
| `magistr-tui/src/harness/fake_terminal.rs` | A terminal that captures output and plays a script as input | `magistr-tui/src/harness/fake_terminal.tests.rs` |
| `magistr-tui/src/harness/script.rs` | What a fake terminal will send, and when | `magistr-tui/src/harness/script.tests.rs` |
| `magistr-tui/src/harness/step.rs` | One thing a scripted terminal does | the harnesses that drive it |
| `magistr-tui/src/harness/timeline.rs` | Messages that arrive at chosen moments | `magistr-tui/src/harness/timeline.tests.rs` |

### `input` — terminal bytes to typed events

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/input/mod.rs` | Terminal decoding facade | `magistr-tui/tests/boundary.rs` |
| `magistr-tui/src/input/csi.rs` | Splitting a CSI parameter string into numbers | `magistr-tui/src/input/csi.tests.rs` |
| `magistr-tui/src/input/decoder.rs` | The incremental decoder: bytes in, events out, any fragmentation | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-tui/src/input/event.rs` | One decoded input event | `magistr-tui/src/input/decoder.tests.rs` |
| `magistr-tui/src/input/reply.rs` | The terminal's answers to the cockpit's startup queries | `magistr-tui/src/input/decoder.tests.rs` |

### `pixel` — the compositors

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/pixel/mod.rs` | The pixel layer's terminal transport: the compositors that ship the cockpit's pixel surfaces to the terminal inside the cell frame's synchronized-output bracket | `magistr-tui/tests/graphics_harness.rs` |
| `magistr-tui/src/pixel/backend.rs` | Which compositor the probe's protocol chose | `magistr-tui/src/pixel/backend.tests.rs` |
| `magistr-tui/src/pixel/compositor.rs` | Ships pixel surfaces to the terminal as kitty placements | `magistr-tui/src/pixel/compositor.tests.rs` |
| `magistr-tui/src/pixel/compression.rs` | How a surface's pixel bytes are packed for the wire | `magistr-tui/src/pixel/compression.tests.rs` |
| `magistr-tui/src/pixel/id_space.rs` | The range of kitty image ids one session draws from | `magistr-tui/src/pixel/id_space.tests.rs` |
| `magistr-tui/src/pixel/sixel_compositor.rs` | Ships pixel surfaces to the terminal as sixel images | `magistr-tui/src/pixel/sixel_compositor.tests.rs` |

### `render` — diff and encode

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/render/mod.rs` | The terminal's side of the cell renderer: a double-buffered pair of the cockpit's cell grids, a damage diff between frames, and an encoder that turns the diff into the fewest bytes the terminal needs | `magistr-tui/tests/end_to_end.rs` |
| `magistr-tui/src/render/change.rs` | One cell that differs between two frames | `magistr-tui/tests/end_to_end.rs` |
| `magistr-tui/src/render/diff.rs` | Damage tracking between the frame on screen and the frame to show | `magistr-tui/src/render/diff.tests.rs` |
| `magistr-tui/src/render/encode.rs` | Turn a frame's changes into the fewest bytes the terminal needs | `magistr-tui/src/render/encode.tests.rs` |
| `magistr-tui/src/render/pixel_type.rs` | Embedded proportional and monospace pixel metrics and glyph cache | `magistr-tui/src/render/pixel_type.tests.rs` |
| `magistr-tui/src/render/pixel_view.rs` | Pixel layout, clipped typography and grapheme-safe composer editing | `magistr-tui/src/render/pixel_view.tests.rs` |
| `magistr-tui/src/render/pixel_frame.rs` | Compose graphics and typography into cached opaque strips | `magistr-tui/src/render/pixel_frame.tests.rs` |
| `magistr-tui/src/render/screen.rs` | The double-buffered screen: draw into the back buffer, present the diff | `magistr-tui/src/render/screen.tests.rs` |

### `runtime` — the loop

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/runtime/mod.rs` | The runtime: the loop that drives the Elm core over a `Terminal` | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-tui/src/runtime/activity.rs` | What just happened, as far as the frame cadence cares | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-tui/src/runtime/driver.rs` | The event loop proper: fold events, paint on the tick | `magistr-tui/tests/runtime_harness.rs`, `magistr-tui/src/runtime/driver.tests.rs` |
| `magistr-tui/src/runtime/terminal_replies.rs` | Resolve terminal queries into background, release support and measured cell size | `magistr-tui/src/runtime/terminal_replies.tests.rs`, `magistr-tui/src/runtime/driver.tests.rs` |
| `magistr-tui/src/runtime/error.rs` | What can stop the runtime | `magistr-tui/src/runtime/error.tests.rs` |
| `magistr-tui/src/runtime/options.rs` | How the runtime is configured | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-tui/src/runtime/runner.rs` | Taking the terminal over, running the loop, handing it back | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-tui/src/runtime/marks.rs` | The frame's marks as pixel surfaces, cached by what each was painted from | `magistr-tui/src/runtime/marks.tests.rs` |
| `magistr-tui/src/runtime/osc52.rs` | The terminal's clipboard: `Cmd::Clipboard` written as OSC 52 over base64 beside the frame, the way an announcement is | `magistr-tui/src/runtime/osc52.tests.rs` |
| `magistr-tui/src/runtime/scheduler.rs` | Deciding when the next frame may be painted | `magistr-tui/src/runtime/scheduler.tests.rs` |
| `magistr-tui/src/runtime/surfaces.rs` | The frame's pixel surfaces: what the cockpit draws with pixels rather than with cells, and the caches that keep an unchanged one from costing a byte | `magistr-tui/tests/runtime_harness.rs` |
| `magistr-tui/src/runtime/tier.rs` | How often the cockpit may paint | `magistr-tui/tests/runtime_harness.rs` |

### `screenshot` — the golden gate's rasterizer

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/screenshot/mod.rs` | The screenshot gate: a cockpit frame as the pixels a terminal would show, so visual regressions are judged on what a person sees — colours, weight, chrome — not on the attr-blind text dumps the insta snapshots are | `magistr-tui/tests/goldens.rs` |
| `magistr-tui/src/screenshot/boxdraw.rs` | Box-drawing and block glyphs drawn from the cell, not from a font | `magistr-tui/src/screenshot/boxdraw.tests.rs` |
| `magistr-tui/src/screenshot/frame.rs` | A cockpit model as the pixels its frame would show | `magistr-tui/src/screenshot/frame.tests.rs` |
| `magistr-tui/src/screenshot/preview.rs` | Custom shell studies through the runtime's pixel layout, surfaces and wire encoder | `magistr-tui/src/screenshot/preview.tests.rs` |
| `magistr-tui/src/screenshot/glyph.rs` | One rasterized glyph, placed inside its cell | `magistr-tui/tests/goldens.rs` |
| `magistr-tui/src/screenshot/kitty.rs` | What a kitty terminal would be holding after a frame: the graphics APC commands read back out of the bytes the cockpit just wrote | `magistr-tui/src/screenshot/kitty.tests.rs` |
| `magistr-tui/src/screenshot/placement.rs` | One image on screen, as the terminal would hold it | `magistr-tui/tests/goldens.rs` |
| `magistr-tui/src/screenshot/profile.rs` | Which terminal a still is rasterized as | `magistr-tui/tests/goldens.rs` |
| `magistr-tui/src/screenshot/raster.rs` | A cell buffer as the pixels a terminal would show | `magistr-tui/src/screenshot/raster.tests.rs` |
| `magistr-tui/src/screenshot/strip.rs` | A motion as a filmstrip: the frames it passes through, side by side | `magistr-tui/src/screenshot/strip.tests.rs` |
| `magistr-tui/src/screenshot/typeface.rs` | The embedded monospace faces the screenshot gate draws with | `magistr-tui/src/screenshot/typeface.tests.rs` |

### `term` — the terminal side

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/term/mod.rs` | The terminal side of the cockpit: the session as bytes, the `Terminal` the runtime writes to, the `TerminalInput` its input thread reads from, and the real implementation over the process's own file descriptors | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/input.rs` | The input side of a terminal: a read that gives up after a while | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/input_event.rs` | What the input thread sends the runtime | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/input_thread.rs` | The one thread that reads the terminal | `magistr-tui/src/term/input_thread.tests.rs` |
| `magistr-tui/src/term/owned_stderr.rs` | Owning the process's stderr while the cockpit owns the screen | `magistr-tui/src/term/owned_stderr.tests.rs` |
| `magistr-tui/src/term/panic_hook.rs` | Leaving the screen before a panic message prints | `magistr-tui/src/term/panic_hook.tests.rs` |
| `magistr-tui/src/term/raw_mode.rs` | Raw mode for as long as the guard lives | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/real.rs` | The process's own terminal, through rustix | `magistr-tui/src/term/real.tests.rs` |
| `magistr-tui/src/term/real_input.rs` | Timed reads from a terminal descriptor | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/restorer.rs` | Putting a terminal back from anywhere | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/session.rs` | Entering and leaving the full-screen session | `magistr-tui/src/term/session.tests.rs` |
| `magistr-tui/src/term/terminal.rs` | What the runtime needs from a terminal | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/termios_restore.rs` | Putting a tty's termios back on demand | `magistr-tui/tests/terminal_integrity.rs` |
| `magistr-tui/src/term/unsupported.rs` | The process terminal where there is no tty the cockpit can drive (Windows): `RealTerminal::stdio` refuses with `Unsupported` before anything is painted | `magistr-tui/src/term/unsupported.tests.rs` |
| `magistr-tui/src/term/unsupported_input.rs` | The input side of that terminal, never handed out | `magistr-tui/src/term/unsupported.tests.rs` |
| `magistr-tui/src/term/winsize.rs` | The window size of a tty, as cells | `magistr-tui/tests/terminal_integrity.rs` |

### `wire` — the escape-sequence walker

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-tui/src/wire/mod.rs` | Escape-sequence parsing shared by graphics probes and input decoding | `magistr-tui/src/wire/sequence.tests.rs` |
| `magistr-tui/src/wire/sequence.rs` | Finding one complete escape sequence at the head of a byte buffer | `magistr-tui/src/wire/sequence.tests.rs` |

## Native desktop launcher

The launcher composes concrete host capabilities without adding them to the
renderer. Both desktop workspaces remain outside the root workspace.

| Module | Responsibility | Tests |
| --- | --- | --- |
| `magistr-desktop/src/main.rs` | Main-thread window entry and protocol child dispatch | `magistr-desktop/tests/process.rs`, `magistr-desktop/tests/native_launch.py` |
| `magistr-desktop/src/args.rs` | Project/session selection and strict child arguments | `magistr-desktop/src/args.tests.rs` |
| `magistr-desktop/src/launch.rs` | Native window composition and awaited host shutdown | `magistr-desktop/src/launch.tests.rs`, `magistr-desktop/tests/native_launch.swift` |

## Additional test references

- `magistr-tui/tests/flat_pty.rs`
