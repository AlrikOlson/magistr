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

Set the provider before applying defaults:

```rust
let config = magistr::agent::AgentConfig::new("Review the change", project_dir)
    .provider("codex")
    .apply_user_defaults();
```

`apply_user_defaults` fills a missing provider, pins successful automatic
detection, and copies that provider's model default for a new session. Explicit
models and runtime-owned resume IDs remain unchanged. Resumes retain their
runtime model unless the caller overrides it. A resolved provider does not
change when global preferences later change.

`AgentSession::start` applies these defaults. Callers using `connect_transport`
directly choose model configuration through the transport's `set_config_option`.

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
failure handling. Malformed required fields fail explicitly.

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
