# Agent and model settings

The [configuration reference](https://magistr.dev/docs/configuration) owns user
instructions for agent, model and effort preferences, their precedence, and
check configuration. [Sessions](https://magistr.dev/docs/sessions) explains
conversation resume and changing agents.

For integration contributors:

- [Session architecture](../architecture/sessions.md) owns provider selection,
  session opening, authentication and saved conversation behavior.
- [Codex integration](codex.md) owns the project launcher and skill-selection
  contract.
- [Development](../development.md) describes focused verification of these
  boundaries.

Native launch preferences apply to Magistr-managed sessions. They do not
reconfigure an existing Codex or Claude Code host conversation.
