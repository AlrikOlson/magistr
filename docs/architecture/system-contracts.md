# System contracts

The [architecture overview](overview.md) and [desktop guide](desktop.md) describe
the current components. Source, workflow, session and history resources each
have one owning writer; interfaces issue typed commands and observe snapshots.

## Glossary and dependencies

| Object | Owner | Identity |
| --- | --- | --- |
| Project and workflow records | ProjectAuthority and native family engines | Canonical project identity and owner generation |
| Working copy and source journal | WorkspaceService | Canonical root, generation and admitted session |
| Provider conversation and turns | SessionSupervisor | Logical history identity and provider conversation identity |
| Resident desktop session | SessionRegistry | Full project/workspace/session address |
| Retained view | SessionObservation | Address, sequence, bounded replay and current requests/draft |
| Iteration and checkpoint | IterationLoop and LoopDriver | Run, attempt and operation identities |
| Checks | AdmittedCheck and native ship scope | Task and source-bound verification receipt |
| Setup, repair and removal | Administrative change plan | Expected per-path state and applied receipts |

Pure domain and application contracts depend on no runtime or renderer.
The semantic cockpit contracts depend on domain, application and workflow
contracts. The shared runtime supplies channels and session relays. The CLI
composes concrete stores and providers. The separate desktop workspace adds
Tauri, native menu/exit handling and semantic HTML/CSS; it depends on the host
application and its existing session client. No terminal renderer is present.

## Runtime and deployment

The desktop owns one SessionRegistry. Each resident host owns its provider and
shares admitted project/workspace resources. Native Quit first waits for registry
settlement, then allows the process to exit. Private worker, ACP and MCP roles
run before a window is created and keep protocol-only stdout.

Root workspace check/Clippy and focused shared-crate tests cover removal and the
changed dependency closure. Desktop CI explicitly runs its own formatter, check,
build, Clippy, Nextest, Node input/stream tests and current visual gate on macOS.
macOS 15 runs the shared Rust, website, release-security and extracted CLI package
gates. Windows only runs the bounded JavaScript adapter contracts; no CI job
builds Windows Rust. Build tool versions are read from the root toolchain files,
and external action commits are pinned and checked. The real macOS journey
retains light/dark/narrow screenshots, native accessibility and clipboard
observations, four-session settlement and child cleanup. Windows runtime
settlement and deeper native IME observation remain separate work.

`release-security.py` is the writer of current dependency/secret findings and
source/artifact identity in `security-verification.json`. `cli-release.py` is its
consumer: publication requires a clean matching source, passing scanners, matching
artifact checksums and a scan no more than 24 hours old. Missing evidence fails
closed. The registry remains the owner of uploaded package versions;
`cargo_publication.py` records effects and reconciles uncertain outcomes without
blindly replaying a write.

## Entry matrix

Every top-level command of the shipped binary, the desktop executable, and the
capability it holds. *Workflow* and *workspace* use the glossary's words;
"none" means the command touches neither store. Admission paths and per-row
tests for the first group are in the [client inventory](clients.md#client-inventory).

| Entry | Workflow | Workspace | Session |
| --- | --- | --- | --- |
| `magistr --help` | none | none | none |
| `magistr-desktop --dir /path/to/project` | admitted host or owner attachment | admitted/attached per working copy | resident SessionRegistry |
| `magistr roadmap` show, next, status, export | reader | none | none |
| `magistr roadmap generate` | none here; the agent's `mcp` writes | admitted for the run | `AgentSession` |
| `magistr run` plain and `--headless` | none here; the agent's `mcp` writes | admitted for the run | `AgentSession` |
| `magistr next` | reader; the agent's MCP tools use the project owner | admitted for the run | supervisor per attempt |
| `magistr next-loop` | NativeReader for progress; loop-owner lock for the checkpoint; agent MCP tools use the project owner | admitted per loop | supervisor per attempt |
| `magistr resolve-dispatch` | none; rewrites the loop checkpoint under the loop-owner lock | none | none |
| `magistr mcp` | Service writer or attached workflow bridge | Workspace owner when free, otherwise attached workspace bridge | none |
| `magistr mcp-proxy` (hidden) | none; filters a foreign MCP server's stdout | none | none |
| `magistr workspace` journal, snapshot, doctor | none (doctor reads the checkpoint as it lies) | lock-free readers; doctor adds one bounded probe | none |
| `magistr workspace` init, key | none | file administration; no service opened | none |
| `magistr acp-agent` (hidden) | none; its tools arrive through `mcp` and the bridge | none | the provider side of one session |
| `magistr codex-launch`, `magistr codex-remove` | none in the wrapper | none | the external runtime; removal is an administrative plan |
| `magistr guard` | reader | none | none |
| `magistr validate` | none; runs gates and records nothing | none | none |
| `magistr init`, `magistr repair`, `magistr remove` | setup checks use read-only context through MCP | administrative plan; connection checks use the workspace owner | none; setup offers a planning prompt |
| `magistr reset`, `magistr import`, `magistr export`, `magistr preset`, `magistr plugins` | none | none | none |
| `magistr detect`, `magistr lint`, `magistr doctor`, `magistr config`, `magistr archetypes`, `magistr policy`, `magistr team` | none | none | none |
| `magistr models` | none | none | a zero-turn provider probe |
| `magistr learn`, `magistr memory`, `magistr plans`, `magistr sessions`, `magistr costs`, `magistr permissions` | none; machine-local and project-local records | none | none |
| `magistr login`, `magistr logout` | none; identity cache | none | none |
| `magistr completions`, `magistr man-pages` | none | none | none |
| `magistr --workspace-worker` (private role, not a command) | none | none; child of one owner | none |

The engine `peer` module is an experiment behind `experimental-peer`
([peers](peers.md)).

