# Codex App Server fixtures

Protocol fixtures for `codex-cli 0.153.4`.

Each NDJSON file interleaves inbound runtime messages with `_sent` markers. The replay executable maps request IDs while preserving the relationships between thread, turn and item IDs.

- `turn_message`: handshake, account, catalog, thread start, streamed answer and thread read.
- `command_approval_accept` / `command_approval_decline`: command approval, decision, tool status and terminal turn.
- `turn_interrupt`: valid steering, stale `expectedTurnId` refusal, interrupt acknowledgment and terminal interruption.
- `thread_resume`: restored transcript, another turn and unknown-thread refusal.

Account details, paths and IDs use synthetic values. MCP inventories contain only server names and status. Event order and message schemas are preserved.

`schema/MANIFEST.json` records the version, generation command, original bundle hashes and method inventories. Adjacent files are the generated schema subset used by this adapter; complete bundles are not checked in. Re-record fixtures and regenerate this subset when adopting a new wire version.

Reference: [official App Server documentation](https://learn.chatgpt.com/docs/app-server). Current documentation may include fields newer than this installed version.
