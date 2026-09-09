//! The Codex provider: drives `codex app-server` on PATH over the official
//! App Server protocol Codex speaks on stdin/stdout.
//!
//! # The wire, as recorded
//!
//! Everything here was recorded from `codex-cli 0.153.4` (the fixtures under
//! `tests/fixtures/codex/`, the generated schema subset pinned beside them
//! with its MANIFEST), never typed from memory: the protocol is versioned
//! with the CLI (`codex app-server generate-json-schema` matches the running
//! binary exactly), so a wire change must fail a named test rather than a
//! cockpit turn.
//!
//! One app-server process per provider, spawned as
//!
//! ```text
//! codex app-server --listen stdio://
//! ```
//!
//! and killed when the provider is dropped. It hosts one Codex *thread* per ACP
//! session. Every line each way is one JSON-RPC 2.0 object without the
//! `jsonrpc` header (JSONL):
//!
//! | direction | frame | meaning |
//! |---|---|---|
//! | → | `initialize` request, then `initialized` notification | the handshake; nothing else is accepted before it |
//! | → | `account/read` | requires the runtime-managed `chatgpt` login; API keys and absent accounts refuse discovery |
//! | → | `model/list` | the catalog: id, display name, default and supported reasoning efforts, the default model |
//! | → | `thread/start` / `thread/resume` | one thread per session, in the session's cwd, with the session's MCP servers as `config.mcp_servers`; Codex mints the thread id, which becomes the ACP session id |
//! | → | `turn/start` | one prompt turn, on the session's model and effort; answers the turn id at once |
//! | ← | `item/started` / `item/completed` | a message, reasoning, command, file change or MCP call — announced as an ACP tool call and closed with its status |
//! | ← | `item/agentMessage/delta`, `item/reasoning/*Delta` | the streamed text and thinking |
//! | ← | `item/commandExecution/requestApproval`, `item/fileChange/requestApproval` | a permission ask, answered with `accept` / `acceptForSession` / `decline` / `cancel` after the ACP client decides |
//! | ← | `turn/plan/updated`, `thread/tokenUsage/updated` | the plan and the context window |
//! | ← | `turn/completed` | exactly one per turn: `completed`, `interrupted` or `failed` with the Codex error |
//! | → | `turn/steer` (`expectedTurnId`) | more input for the turn in flight; refused for any other turn id |
//! | → | `turn/interrupt` | cancel: the turn ends `interrupted` |
//! | → | `thread/read` | the persisted thread, for a caller that wants its record |
//!
//! `approvalPolicy: untrusted` and `sandbox: workspace-write` are deliberate:
//! commands outside Codex's trusted read-only set ask, and valid approvals
//! for active turns reach the ACP client so its policy decides. Stale approvals
//! are cancelled and malformed requests refused. Server requests the provider
//! does not serve (user-input questions, MCP elicitations, permission
//! profiles, dynamic tools) are refused at once with a JSON-RPC error so a
//! turn can never hang on them. The OpenAI chat API provider (`openai`) is a
//! different thing and stays distinct.

mod approval;
mod frame;
mod handshake;
mod process;
mod provider;
mod routing;
mod rpc_error;
mod thread;
mod thread_record;
mod thread_spec;
mod translate;

pub use provider::CodexProvider;
pub use thread_record::ThreadRecord;
