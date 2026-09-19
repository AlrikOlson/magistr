# Sessions and providers

`AgentConfig` selects a provider for each session. The shared `connect_transport`
path serves the CLI backend, terminal cockpit, desktop launcher and loop connector.
The desktop's production composition and its separate fixture viewer are described
in the [desktop guide](desktop.md).
See the [contract baseline](contracts.md) for
entry paths, behavioral differences, and intended consolidation policies.

| Provider ID | Session type |
| --- | --- |
| `claude` | Claude Code coding runtime |
| `copilot` | Copilot coding runtime |
| `codex` | Codex App Server coding runtime |
| `openai`, `gemini`, `ollama` | Chat-only API adapters |

Unknown provider names fail before authentication probing or child startup.
Omitting the provider uses the user's default and automatic detection. Explicit
`auto` requests detection even if the global default names an API provider.
Automatic detection chooses Claude Code or Copilot.

## Configuration and defaults

Resolve one specification before connecting:

```rust
let spec = magistr::agent::AgentConfig::new("Review the change", project_dir)
    .provider("codex")
    .effort("high")
    .resolve()?;
let transport = spec.connect_transport().await?;
```

`ResolvedSessionSpec` freezes the selected runtime, project directory, model,
effort, permission policy, unattended response mode, and project MCP configuration.
Its fields are private; configuration accessors borrow the snapshot, and the
session owner consumes it to take the same permission policy. CLI, cockpit,
loop workers, and judge startup use this path. `connect_transport(&AgentConfig)`
currently resolves a specification internally. It is not a protected API;
update its callers directly if the intended session interface changes.

An explicit provider takes precedence over user defaults; automatic detection
pins one runtime. Missing models inherit only that provider's user default for
new sessions. Resumes retain their runtime model unless explicitly overridden.
Project model governance applies after defaults. The specification's provenance
records the source of each choice, without credentials or MCP environment values.
Changing configuration files afterwards affects the next specification. A project
with model restrictions requires a verifiable model choice; an unknown runtime
or resumed model is refused. Malformed permission/MCP files and unsupported MCP
entries also fail before startup. The shared host currently accepts only stdio
entries from `.mcp.json`; direct provider callers can use the additional
transports listed below.

The ACP handshake applies explicit model and effort choices before exposing a
ready transport. In-binary providers validate namespaced
`_meta.magistr.session` options during session creation/loading. Other agents
use their negotiated model/reasoning selectors; absent, invalid, or unacknowledged
choices fail. Selecting a model later remains an explicit session operation.
The loop judge uses an explicit override or the common provider default; it no
longer silently chooses a different model after startup.

A request to continue an unspecified last conversation is rejected with guidance
to provide a runtime-owned resume ID. An unsupported load is an error; it never
opens a new empty conversation as a fallback. The legacy options now have an
explicit contract:

| AgentConfig option | Execution contract |
| --- | --- |
| `system_prompt` | Transported as `_meta.magistr.session.system_prompt`. Claude appends it with `--append-system-prompt`; Codex sends `developerInstructions`; OpenAI and Ollama send a system message; Gemini sends `systemInstruction`. |
| `max_turns` | Enforced by the shared host supervisor across headless and cockpit sessions. Admission is refused before provider inference once the budget is scheduled; zero is rejected during resolution. |
| `permission_mode` | Explicitly unsupported before provider startup. The shared permission profile and host policy remain authoritative; Claude and Codex keep their provider approval defaults behind that authority. |

The ACP agent advertises the supported system prompt session option alongside
model and effort.

## Capability guarantees

The in-binary agent publishes its implemented guarantees in
`initialize._meta.magistr.capabilities`, separate from provider model names.
`session_options` identifies model, effort, and system prompt startup options. Capabilities describe
this adapter, rather than every feature the vendor may offer.

| Adapter | Durable load/replay | Open-session continuation | Steering | Model changes | Cancellation |
| --- | --- | --- | --- | --- | --- |
| Claude Code | Persisted transcript and runtime resume | Yes | Unavailable | Next turn | Interrupt/grace; no terminal acknowledgment guarantee |
| Codex | Runtime thread restore and replay | Yes | Direct library API | Next turn | Waits for terminal runtime event |
| OpenAI, Gemini, Ollama APIs | Unavailable | Process-local history | Unavailable | Next turn | HTTP stream dropped; no remote acknowledgment |
| Copilot | Negotiated from external agent | Agent-owned | Agent-owned | Negotiated selectors | Agent-owned |

All built-in adapters currently consume text prompt blocks. Unsupported images,
audio, and embedded resources are refused before a turn is enqueued. Coding
runtime adapters support tools; chat API adapters have no tool harness and reject
nonempty MCP configuration. Claude accepts stdio/HTTP/SSE MCP configuration;
Codex accepts stdio/HTTP and rejects SSE. The scripted provider is a deterministic
test fixture, including its simulated replay, and is not durable user storage.

ACP loading promises persisted context across process restarts and history
replay before the load response ([session setup](https://agentclientprotocol.com/protocol/v1/session-setup)).
API adapters reject loading through both the provider and wire entry points.
Claude loading requires a readable, valid transcript; missing or malformed
history is an error. Codex validates restored thread identity. Unknown provider
capabilities default to unsupported.

## Chat API completion

Chat adapters require explicit provider terminal evidence and a complete HTTP
body before answering ACP: OpenAI requires a recognized `finish_reason` followed
by `[DONE]`; Gemini requires a recognized `finishReason` or prompt block;
Ollama requires `done: true` with a recognized reason (older responses may omit
the reason). Normal stops, token limits and content refusals produce distinct
ACP stop reasons. Unsupported reasons, stream error objects, malformed data,
truncated transport and missing terminal evidence fail the turn.

Usage is independent of completion: absent counts produce no usage update.
OpenAI refusal deltas remain visible as text and cannot turn a normal finish
into success. Cancellation drops the HTTP request or body and reports local
cancellation, without claiming remote acknowledgment. Failed and cancelled
turns retain visible partial replies, or remove unanswered prompts, before the
next request uses the session history. Decoder tests and loopback HTTP fixtures
exercise these contracts without live credentials.

## Shared session supervision

The CLI host's [SessionSupervisor](../../crates/magistr-cli/src/agent/session_supervisor/mod.rs)
owns both headless and cockpit provider connections.
`AgentSession`, `AcpBackend`, and the cockpit's `CockpitSession` handle delegate
to it; they do not run separate permission or lifecycle loops. The supervisor
consumes the resolved specification and its permission policy, admits the opening
prompt after the handshake's model/effort selection, queues follow-ups in order,
and runs one provider turn
at a time. Configuration changes are refused while a turn or an earlier
configuration operation is active.

The supervisor's snapshot separates provider residency, active/queued turns,
cancellation requests, and the last observed turn outcome. Provider errors or
event-stream loss fail the session; missing terminal evidence never becomes
success or confirmed cancellation. Plain drains also reject EOF without a
terminal response and settle an explicit cancellation without waiting for the
retained session's stream to close. A recorded stop reason is the adapter's
observed response; it does not strengthen the provider guarantees in the
capability table above.

| Operation | Contract |
| --- | --- |
| `cancel_turn` | Request cancellation, cancel outstanding permissions and queued input, and keep the provider session resident. A terminal prompt response determines the observed outcome; a request alone does not confirm cancellation. |
| `terminate_session` | End admission and await transport/task cleanup. Closing an unacknowledged turn does not invent a successful or cancelled provider response. |
| `detach_client` | Stop message observation while the owning supervisor continues the provider session. Dropping the owner still performs cleanup; this is not a durable/background daemon handoff. |

| `interrupt` | Wind the session down the way a process interrupt does: admit no further turn, discard queued input, cancel the active turn, and close the transport once the provider's terminal response arrives — at once when nothing is active. The process-wide shutdown request takes this path inside every resident owner. |

A later turn may start after the previous turn's terminal response. The legacy
`cancel` convenience operation ends the usable session after cancellation;
new callers use the explicit operations above. Headless `run`, `next`, and
`next-loop` await termination before process exit or the next attempt.

### Bounds

The owner's admission and observation are bounded explicitly
(`audit-260912-11-bounded-session`). At most
`SessionSupervisor::QUEUED_TURN_CAPACITY` follow-ups wait behind the active
turn; the next one is refused with the reason. Both observer streams — the
headless message stream and the cockpit's event stream — run through the
[observation channel](../../crates/magistr-cli/src/agent/observation/mod.rs)
under an `ObservationCapacity`, and the provider's execution never waits on
an observer:

| Class | Items | Rule under the bound |
| --- | --- | --- |
| Lossless | Decisions (permission requests, cancellations, lapses), terminal outcomes, tool calls and updates, user echoes, initialization, images, errors, any multi-block assistant message | Delivered in order, never coalesced. Past `lossless_items` (4096) the channel admits one explicit overflow marker — an `Error` event, an `Err` message — and closes; the owner marks the observer detached and the session stays resident. Nothing is dropped silently. |
| Text | Assistant text and thought deltas, one terminal's output bytes | Adjacent deltas of the same kind coalesce into one pending item. Past `text_bytes` (256 KiB) the oldest pending text is trimmed on a char boundary and one lag marker (`AgentEvent::Lagged`, the `observation_lagged` system message) stands where the trim happened, counting bytes and whole deltas; the cockpit shows it as a note in the transcript. |
| Telemetry | Usage, plan, configuration and command sets | The newest value replaces the latest pending one of its kind in place; an observer never drains a backlog of stale meters. |

The supervisor fixtures flood a real in-process provider with two megabytes
of deltas while the observer reads nothing until the turn has ended: the
pending text stays under the bound, the lag marker accounts for every
trimmed byte, and the terminal outcome still arrives. A lossless flood past
a small bound ends observation behind its marker and leaves a session that
admits the next turn.

### Conversation and planning coordination

The shared runtime's
[InteractiveSession](../../crates/magistr-cockpit-runtime/src/interactive_session.rs)
owns conversation/planning context switching, one queued roadmap request,
connection retry and the lifetime of its local and provider relays. Hosts supply
an asynchronous connector for the requested `SessionPurpose`, returning an
application `SessionHandle`, plus admitted workbench, workflow and checkpoint
ports. The runtime does not resolve provider configuration or acquire project
authority.

The CLI's [host factory](../../crates/magistr-cli/src/orchestration/interactive_session.rs)
supplies that connector using the existing resolved provider setup and planning
permission policy. Conversation and planning retain separate provider contexts.
After a provider failure, the coordinator retains the initialized session ID
for each purpose and asks the host to restore that exact context on the next
request. Unsupported or failed restoration rejects the request visibly and keeps
its ID for another attempt; it never opens an empty conversation behind existing
history. Restored messages before the provider's initialization marker are not
appended again. An initial explicit resume still displays saved history. The
coordinator retains this state only for its own lifetime; window close is not a
background-session or draft-persistence mechanism.

A queued planning request starts after the current turn ends; cancelling the
queued request leaves the active conversation alone. Loop handoffs still belong
to the loop driver, which waits for work and checks to settle before invoking
this coordinator and receives its loop controls back without reopening settled
iteration accounting.

Normal closure waits for the active, parked and local relays with bounded
deadlines. Dropping or aborting the coordinator cancels all three relays,
including local work that has been queued but not dispatched. Already running
blocking operations cannot be cancelled; the relay's result fence suppresses
their late answers. Direct coordinator tests cover context reuse, queued
cancellation, retry, provider death and owner cancellation with a running job.
The desktop launcher consumes this coordinator through the host below.
A real ACP channel test restores both purposes after provider failure and checks
that loaded history does not duplicate the next response. The native desktop
check uses a controlled Ollama peer to verify failure, unsupported restoration
and the retained request. Full live-provider and recovery journeys remain
unfinished.

### Live interface host

[InteractiveHost](../../crates/magistr-cli/src/application/interactive_host.rs)
composes the existing workflow client, workspace admission, resolved provider,
shared coordinator and projection feeds. `magistr run` and `magistr-desktop`
use this host. Its
`start` method accepts an initial model, inbox and command receiver and returns
the running host plus the seeded model. It opens or attaches to the workflow
owner and preserves the workspace's admitted or refused source route. It does
not open a terminal or window.

Create the channels before opening a surface. Provider startup errors then
arrive before terminal raw mode; messages received during startup stay queued.
The terminal consumes those channels through `Runtime::with_channels` and
resizes the model to the actual terminal before drawing.

Call `shutdown().await` when the surface closes. Dropping the host also requests
closure; its cleanup task retains the project until the coordinator settles,
then joins the feeds, stops owner endpoints and shuts down the workspace. The
coordinator gets five seconds before it is aborted and joined. Keep the Tokio
runtime alive through cleanup. Startup failure releases source references
before closing the workspace, and failed command forwarding returns the
undelivered action to the surface.

The `magistr-cli` package exposes the `magistr` host library with
`default-features = false`. Its normal dependency graph then excludes
`magistr-tui`; the default `terminal` feature enables terminal commands and
the CLI binary. Provider and workspace implementations remain in the host,
not the neutral runtime or desktop renderer. Test harnesses may still depend
on the terminal crate.

A new executable must also supply the host's existing process entry roles:
provider startup can invoke the executable's ACP-agent command, and workspace
workers invoke its private worker role. Disabling terminal features alone does
not provide that dispatch. The separate `magistr-desktop` executable provides
these roles and both native MCP launch targets, preserving the renderer's
dependency boundary. Its main thread runs the native window and awaits this
host's shutdown on every returned window outcome. See the desktop guide for
the distinction between launcher checks and the remaining live-provider matrix.

### Owned relay jobs

The shared runtime's cockpit relay ([session](../../crates/magistr-cockpit-runtime/src/session.rs))
carries workbench, engine and checkpoint commands out as its own jobs
([`SessionJobs`](../../crates/magistr-cockpit-runtime/src/session_jobs.rs)):
each call runs on a blocking thread against the shared port, so a slow
search or a git read never stalls the task that observes the provider and
forwards its cancellation. At most `SessionJobs::CAPACITY` (4) run at once;
one more is refused as a `Failed` / `PortFailed` event naming the ask. A
command whose port is absent — a session without a workbench, an engine
port or a checkpoint ledger — is refused by name, never answered with
silence. When the runtime ends the relay closes its fence before it asks
the owner to terminate, waits `SessionJobs::JOIN_GRACE` (2 s) for its jobs,
and reports how many it had to leave running; a fenced job's answer never
posts on behalf of a session that is gone. Dropping the job owner, including
when its relay task is aborted, also closes the fence. Closing and publishing
share a lock, so a result cannot pass an open check and publish after closure.
This suppresses late answers; it does not cancel blocking side effects. Hosts
retain responsibility for provider cleanup and must await their relay tasks.

### Interrupts and shutdown

A plain `SIGINT` no longer exits the process from the signal handler. The
first one requests the process-wide
[`Shutdown`](../../crates/magistr-cli/src/utils/shutdown.rs): every resident
session owner takes its `interrupt` path (no further turn, the active one
cancelled, the transport closed after the provider's terminal response);
every owned subprocess — a gate on its pty, a workflow check, a workspace
command — is ended the way a cancel ends it (`SIGTERM`, the grace,
`SIGKILL`, reaped and its group settled) and answers `cancelled`; the loop
driver admits no further attempt. The command then finishes its own
settlement — the cost row, the receipts, the checkpoint, the workspace's
release, in the order it already runs them — and the process exits 130.
A second `SIGINT`, or the settle deadline (`Shutdown::SETTLE_DEADLINE`, 8 s)
passing first, is the distinct emergency exit: the terminal restored, the
registered process groups killed, exit 130, and a stderr line naming which
way out was taken. The process fixtures in `tests/cli_integration.rs`
interrupt a real `magistr validate --ci` over a sleeping gate (settled and
joined, exit 130, no emergency line) and over a `TERM`-ignoring gate with a
second interrupt (emergency line, the child gone).

Not covered: an external `SIGINT` to the interactive cockpit drains its
session and stops its loop but does not close the cockpit runtime itself,
which ends on its own keys or on the deadline's emergency exit; the
keyboard's Ctrl-C inside the cockpit is a key, not a signal, and keeps its
cancel-then-quit contract.
Transport shutdown retains task handles through timeout/caller cancellation,
aborts tasks that exceed the deadline, and awaits their release. Explicit
shutdown errors are returned to callers.

ACP subprocesses acquire the shared `magistr-process::ProcessGroup` guard at
spawn, before pipe extraction or transport handoff. The transport retains it
through handshake failures and owner cancellation; the supervisor reads its
provider PID without separately owning registry membership. Closing a transport
confirms group disappearance after its child/task cleanup, with a two-second
settlement deadline. Cleanup failure takes precedence over an earlier connection
timeout. A dropped child retains explicit reaping ownership outside the async
runtime; a dropped group requests SIGKILL and retires its registry entry only
after disappearance. Incomplete cleanup remains observable in the hard-exit
registry while fallback observation continues.

The workflow MCP owner proxy also uses cancellable stdio. Once either copy
direction ends, cancelling the other leaves no blocking stdin or stdout worker
keeping the client process alive. Client EOF ends its attachment cleanly; an
observed owner disconnect reports failure without taking over ownership.

The runtime-independent application `SessionHandle` gives the cockpit explicit
operations for turns, permissions, terminal cancellation, detach, and termination.
The application also owns its `AgentEvent` observations, `SessionOptions` and
`SessionPolicy` vocabulary. Both interactive and headless hosts import those
contracts directly; the CLI translates ACP events and permission requests into
them. Tool records carry bounded text or encoded JSON and textual wire locations.
Policy requests carry the full input independently of those display bounds.
The cockpit relay stamps presentation timing and serves workbench, engine, and
checkpoint commands; it does not own ACP connections or permission reply channels. The
supervisor emits initialization and the opening user text before provider output.
An absent opening prompt leaves the session idle, including home resume.

Interactive Ask stays pending inside the same owner that handles unattended
permissions. Replies must name an offered option and still belong to a live
request before remembered policy can change. Cancellation, expiry, stream loss,
and shutdown remove pending cards and revoke their replies. A new turn is
admitted only after terminal evidence for the previous turn.

Cockpit `next` and `next-loop` use the same handle. Negotiated goal extensions
enter through explicit host `begin_goal` and `clear_goal` operations; their actual
response settles the supervised turn. Observed extension responses carry the
admitted turn ID through the same ACP event queue as provider updates. The
supervisor forwards earlier text before emitting the matching terminal outcome;
an unrelated response cannot settle the active goal. The loop waits for both a
successful extension reply and the cockpit's terminal observation before closing
the relay, so completion cannot discard preceding transcript text. Command-based
goals use ordinary turn admission.
The driver's read-only transport tap observes capabilities and echoes;
it cannot send work. The separate workflow judge retains its constrained direct
transport path. The supervisor remains in the CLI runtime host; application
contracts have no ACP or Tokio dependency.

## Permission decisions

The [shared permission service](permissions.md) evaluates received ACP requests
before cockpit presentation or noninteractive response selection. Headless
supervision owns this handling at startup, including plain sessions. Profiles, rules,
gates and governance use the selected project directory. Noninteractive Ask
refuses by default; `.claude/permissions.toml` can explicitly select
`noninteractive = "legacy-automatic-approval"` as an explicit current setting.
The pre-release policy permits removing this setting with its callers and tests.

## Codex

Select `codex` per session or set `[session] default_provider = "codex"` in user
configuration. This adapter requires the Codex CLI signed in through ChatGPT.
It invokes Magistr's `acp-agent --provider codex`, which drives
`codex app-server --listen stdio://`. Missing runtime, unsupported authentication,
unavailable model, and quota errors are reported; selection never falls through
to another provider or API billing.

Codex owns credentials and refresh. The adapter checks its account response and
does not read tokens or perform login/logout. One retained App Server process
hosts its threads. Its JSONL client translates session events and permissions to
ACP for Magistr's consumers.

Models and supported reasoning efforts come from runtime discovery.
`CodexProvider::effort(...)` selects an effort that is validated before inference.
A restored thread retains its effort; changing models selects the discovered
default. ACP v1 does not add an effort picker.

The Codex thread ID is the ACP session ID. `AgentConfig::resume(id)` reloads that
thread and its history. Library callers can inspect history with `read_thread`
and steer the expected active turn with `steer`. Steering is a library API,
not an ACP v1 request. Cancellation requests an interrupt and consumes the
terminal turn event before the thread is reused.

Command and file-change approvals are relayed to the ACP client. Unsupported
server requests, including user-input questions, MCP elicitations, and dynamic
tools, receive an explicit refusal. Session MCP configuration supports stdio and
streamable HTTP and is merged by Codex with its own configuration. Legacy SSE
entries are rejected.

## Testing and data

The supervisor suite drives controlled ACP providers and a raw protocol fixture
through real connections. It checks queued-turn ordering, delayed cancellation
acknowledgment, permission refusal and fresh-turn grants, configuration admission,
provider failure, detached observation, and cancellation of a cleanup caller.
Transport tests separately verify real Unix subprocess cleanup, handshake
ownership, and task timeout cleanup. Synchronous owner drop requests abortion;
the runtime finishes cancellation and process reaping. These tests do not claim
live-provider or cross-platform process verification.

Recorded Codex fixtures use CLI version `0.153.4`; see their
[provenance](../../crates/magistr-acp-agent/tests/fixtures/codex/README.md).
Replay tests cover lifecycle, permissions, steering, cancellation, resume, and
failure handling. Malformed required fields fail explicitly. Fresh-process tests
check API load refusal through both the provider and actual CLI transport, replay
Claude history from a parent-written disk transcript, and restore Codex fixture
history in an isolated client process. These tests validate the adapter contract
with controlled runtimes; they do not claim live-provider storage compatibility.

The opt-in live check uses the real transport and a disposable directory:

```sh
MAGISTR_CODEX_LIVE=1 cargo run -p magistr-cli --example codex_transport_live
```

It consumes a short provider turn and verifies streamed tool evidence, completion,
file contents, and cleanup. Ordinary tests do not run it. The
[direct Codex launcher](../reference/codex.md) has separate read-only runtime integration
tests and does not select this session provider.

API sessions send their accumulated conversation history to the configured
service. External runtimes use their own authentication and storage. Local
records can contain prompts, paths, and outputs; see
[running agents](../development.md#running-agents).
