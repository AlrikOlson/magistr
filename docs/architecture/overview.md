# Architecture

Magistr separates project configuration, workflow state, agent transport, and
presentation. The root Cargo manifest groups the Rust crates into one workspace.
The [contract baseline](contracts.md) maps current owners and entry paths,
records the refactor's intended policies, and names executable behavior scenarios.

| Component | Responsibility |
| --- | --- |
| `magistr-cli` | Commands, project setup, runtime integration, and MCP service startup |
| `magistr-domain` | Deterministic identities, run rules, policy, and verification values |
| `magistr-application` | Project-scoped use cases, typed outcomes, and narrow adapter ports |
| `magistr-core` | Configuration, presets, and shared project/interface types |
| `magistr-engine` | Sessions, workflow policies, costs, gates, and native think-and-ship integration |
| `magistr-acp` | Agent Client Protocol transport and configured local effect adapter |
| `magistr-acp-agent` | Coding-runtime and chat-API provider adapters |
| `magistr-mcp` | In-process MCP tool definitions and request handling |
| `magistr-cockpit` | Shared interface state, updates, layout vocabulary, and effect ports |
| `magistr-tui` | Terminal input, rendering, graphics protocols, and runtime |
| `magistr-theme` | Shared color and theme vocabulary |
| `magistr-protocol` | Transport types |

The [domain boundary](domain.md) enforces effect-free dependencies and preserves
engine compatibility exports while application and session ownership migrate.
The [application contracts](application.md) define project-scoped use cases and
effect ports. The [host effect adapter](effects.md) routes ACP filesystem and
terminal requests through configured capabilities; further host migration remains
separate.
The [permission service](permissions.md) supplies one decision implementation for
received agent requests and host effects across cockpit and noninteractive sessions.
[Durable operations](durability.md) record what a run dispatches before each
effect and reconcile interrupted attempts against their authorities on recovery.

## Workflow and tools

The CLI's MCP service embeds the repository-local
[think-and-ship source](../../subsystems/workflow/PROVENANCE.md). Its roadmap,
reasoning, execution, and signal tools share native project state. Workbench is
a separate workspace service for navigation and edits. The
[project authority](project-authority.md)
controls cooperating MCP owners; a second process must not open an independent
cached writer for the same project.

The accepted [first-party subsystem decision](owned-subsystems.md) makes
hard forks of think-and-ship and Workbench part of the refactor. It defines
workflow and workspace ownership, source certification, and migration
constraints. Workflow source is adopted into the root Cargo workspace; typed
use-case extraction, Workbench adoption, workspace ownership, and source
snapshots remain pending in native roadmap state.

Project setup enters through the CLI's detection module, which calls the core
stack detector with the CLI's plugin results. Setup writes
`.magistr/project.json` and agent-specific files from the CLI crate's templates
and the core crate's presets. The project file supplies commands, custom gates,
testing conventions, and native-roadmap settings. Generated files carry
ownership rules so refresh and removal can preserve user changes.

## Sessions and interfaces

[Session configuration](sessions.md) chooses an agent provider. ACP carries
session updates and permission requests between the runtime adapter and Magistr.
The CLI host's session supervisor owns turn admission, permission handling,
terminal observations, and provider residency for both headless and cockpit
sessions. The cockpit uses an application session handle. Providers differ in
model discovery, resumption,
tool use, and cancellation; chat APIs do not supply the same coding-session
behavior as external runtimes.

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

[Desktop development](desktop.md) is maintained on the `gui2` branch.
