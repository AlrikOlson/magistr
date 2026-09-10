# Sessions and providers

`AgentConfig` selects a provider for each session. The shared `connect_transport`
path serves the CLI backend, terminal cockpit, and loop connector. The native
desktop executable on the audited `gui2` branch runs fixtures; its live session
connection is not integrated. See the [contract baseline](contracts.md) for
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
remains a compatibility entry point that resolves a specification internally.

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
opens a new empty conversation as a fallback. Legacy `AgentConfig` fields
`system_prompt`, `max_turns`, and `permission_mode` remain outside this resolved
execution contract; their inherited inert behavior is tracked separately. Shared
permission profiles and project policy are the enforced permission controls.

## Capability guarantees

The in-binary agent publishes its implemented guarantees in
`initialize._meta.magistr.capabilities`, separate from provider model names.
`session_options` identifies model/effort startup options. Capabilities describe
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

## Permission decisions

The [shared permission service](permissions.md) evaluates received ACP requests
before cockpit presentation or noninteractive response selection. AgentSession
installs its policy worker at startup, including plain sessions. Profiles, rules,
gates and governance use the selected project directory. Noninteractive Ask
refuses by default; `.claude/permissions.toml` can explicitly select
`noninteractive = "legacy-automatic-approval"` for compatibility.

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
[direct Codex launcher](../reference/codex.md) has separate read-only compatibility
tests and does not select this session provider.

API sessions send their accumulated conversation history to the configured
service. External runtimes use their own authentication and storage. Local
records can contain prompts, paths, and outputs; see
[running agents](../development.md#running-agents).
