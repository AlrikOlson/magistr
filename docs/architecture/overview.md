# Architecture

Magistr separates project configuration, workflow state, agent transport, and
presentation. The root Cargo manifest groups the Rust crates into one workspace.

| Component | Responsibility |
| --- | --- |
| `magistr-cli` | Commands, project setup, runtime integration, and MCP service startup |
| `magistr-core` | Configuration, presets, and shared domain and project types |
| `magistr-engine` | Sessions, workflow policies, costs, gates, and native think-and-ship integration |
| `magistr-acp` | Agent Client Protocol transport |
| `magistr-acp-agent` | Coding-runtime and chat-API provider adapters |
| `magistr-mcp` | In-process MCP tool definitions and request handling |
| `magistr-cockpit` | Shared interface state, updates, layout vocabulary, and effect ports |
| `magistr-tui` | Terminal input, rendering, graphics protocols, and runtime |
| `magistr-gui2` | Native desktop rendering, input, and accessibility |
| `magistr-theme` | Shared color and theme vocabulary |
| `magistr-protocol` | Transport types |

## Workflow and tools

The CLI's MCP service embeds think-and-ship. Its roadmap, reasoning, execution,
and signal tools share native project state. Workbench is a separate workspace
service for navigation and edits. The [project authority](project-authority.md)
controls cooperating MCP owners; a second process must not open an independent
cached writer for the same project.

Project setup enters through the CLI's detection module, which calls the core
stack detector with the CLI's plugin results. Setup writes
`.magistr/project.json` and agent-specific files from the CLI crate's templates
and the core crate's presets. The project file supplies commands, custom gates,
testing conventions, and native-roadmap settings. Generated files carry
ownership rules so refresh and removal can preserve user changes.

## Sessions and interfaces

[Session configuration](sessions.md) chooses an agent provider. ACP carries
session updates and permission requests between the runtime adapter and Magistr.
Providers differ in model discovery, resumption, tool use, and cancellation;
chat APIs do not supply the same coding-session behavior as external runtimes.

The [terminal](terminal.md) renders the cockpit's `Model` as a cell tree and
sends `Msg` values to its update function. Effectful work returns messages to
the shared state instead of mutating it from renderer code.

The [peer protocol](peers.md) is a separate engine API for two-party plan agreement
and worker admission. Its domain types are implemented, but production runtime
authentication and governed dispatch are not wired into the CLI or interfaces.

## Applications and resources

`apps/website` contains the documentation website. Fixture descriptions, font
provenance, and licenses remain beside
the files they document.

The [desktop interface](desktop.md) renders the same cockpit model in a native window.
