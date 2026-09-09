//! The Claude provider: drives the `claude` on PATH over the stream-json
//! control protocol Claude Code speaks on stdin/stdout in headless mode.
//!
//! # The wire, as recorded
//!
//! Everything here was recorded from `claude` 2.1.258 (the fixtures under
//! `tests/fixtures/claude/`), never typed from memory: the protocol is
//! private, undocumented, and versioned with the CLI, so a wire change must
//! fail a named test rather than a cockpit turn.
//!
//! One CLI process per ACP session, spawned as
//!
//! ```text
//! claude -p --input-format stream-json --output-format stream-json --verbose
//!        --include-partial-messages --permission-prompt-tool stdio
//!        --permission-mode default --strict-mcp-config [--mcp-config <json>]
//!        [--model <id>] (--session-id <uuid> | --resume <uuid>)
//! ```
//!
//! and closed by closing its stdin. Every line each way is one JSON object:
//!
//! | direction | frame | meaning |
//! |---|---|---|
//! | → | `control_request` `initialize` | the handshake; answered at once with the model catalog and the command list |
//! | → | `user` (`message.content` text blocks) | one prompt turn |
//! | ← | `system`/`init` | session id, model, MCP server statuses — repeated at the top of every turn |
//! | ← | `stream_event` | the Messages API stream: text and thinking deltas, block starts |
//! | ← | `assistant` | one completed block: thinking, text, or a `tool_use` with its full input |
//! | ← | `control_request` `can_use_tool` | a permission ask, after the `tool_use` block; answered with `control_response` |
//! | ← | `user` (`tool_result`) | the tool's output — a text block and, whole, `rawOutput` — `is_error` on a denial |
//! | ← | `result` | exactly one per turn: subtype, usage, cumulative cost, denials |
//! | → | `control_request` `interrupt` | cancel: the turn ends with an `error_during_execution` result |
//! | → | `control_request` `set_model` | switch the model for the next turns |
//!
//! `--permission-mode default` is deliberate: the user's own settings may
//! default to bypass, and every ask has to reach the ACP client so its policy
//! decides. `--strict-mcp-config` makes the session's MCP servers exactly the
//! ones the client passed on `session/new`.

mod control;
mod frame;
mod handshake;
mod launch;
mod permission;
mod process;
mod provider;
mod session;
mod tool_kind;
mod transcript;
mod translate;

pub use provider::ClaudeProvider;
