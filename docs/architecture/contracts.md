# Architecture contract baseline

Magistr's current contract is one owner for each resource and narrow commands and
observations for every interface. The [pre-release policy](../../AGENTS.md#before-public-release)
applies to APIs, names, formats and stored data. Current callers change together;
historical behavior does not impose compatibility requirements.

| Object | Owning writer | Readers and commands |
| --- | --- | --- |
| Project workflow | ProjectAuthority and its admitted NativeEngines | NativeReader, attached OwnerClient and native MCP families |
| Workspace source | One WorkspaceService per canonical working-copy root | Typed source tools, admitted SourceWriter and lock-free evidence |
| Execution plan and checks | Native ship scope and AdmittedCheck | Agent tools, loop verification and recorded check receipts |
| Run and attempts | IterationLoop and operation ledger | LoopDriver, recovery and diagnostics |
| Provider session and turn | SessionSupervisor | Application session handles and bounded observations |
| Resident application session | SessionRegistry | SessionClient and SessionSubscription, addressed by project/workspace/session |
| Session history | The provider session owner's settlement writer | Recent projects and resume/configuration readers |
| Permission request | Resident supervisor and shared policy | Correlated pending observation with complete requested action and offered answers |
| Interface | Tauri host and semantic HTML/CSS | Typed commands, one acknowledged in-flight frame and native accessible controls |
| Administrative change | Existing setup/repair/removal plan | CLI preview and apply commands |

## Implemented run consolidation

Shared domain run policy owns attempt limits and stop decisions.
The supervisor retains queued turns and acknowledges cancellation before another
turn starts. Loops recover their selected unfinished item and saved execution plan.
The loop owns its iteration checkpoints and handles Quit during every phase;
no renderer action is required to write a boundary checkpoint or settle work.

The current CLI is streamed or headless. The desktop composes the same admitted
provider, workflow and source services. It offers project/workspace navigation
and resident conversations; additional supervision, planning and review views
remain separate roadmap work.

## Evidence

The application and domain boundary suites check dependency direction.
Session registry/observation, supervisor, loop-driver and source-pipeline suites
exercise real contracts. The native desktop journey checks keyboard, pointer,
permissions, drafts, clipboard and ordered Quit settlement.
See [desktop](desktop.md), [sessions](sessions.md), [clients](clients.md) and the
[system entry matrix](system-contracts.md#entry-matrix).
