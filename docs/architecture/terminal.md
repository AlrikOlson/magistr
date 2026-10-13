# Command and protocol interfaces

`magistr` is a plain command-line application. With no subcommand it prints
help. `run`, `next` and `next-loop` stream progress to standard output;
`--headless` provides noninteractive execution. Commands never take over the
terminal, negotiate graphics, enable mouse tracking or enter an alternate screen.

The desktop has one independent launch route, described in
[desktop architecture](desktop.md). Presentation owns no provider, workspace
or workflow service lifetime.

## Retained protocols

Provider terminals remain application effects. A provider may create a command
process, read its output, wait, cancel and release it through the admitted
effect adapter. `magistr-process` owns PTYs, Unix process groups and reaping.
This capability is independent of the Magistr interface.

`magistr mcp-serve` serves native workflow tools. `magistr mcp-launch` serves
workflow and source clients. The private `--workspace-worker` role handles
bounded parser and sandbox requests; `acp-agent` serves the selected provider.
Both CLI and desktop executables dispatch these roles before initializing a
window or project. Protocol stdout contains only protocol frames.

SIGINT requests orderly shutdown of the run and its owned children. Hard exits
use the registered child guard. No terminal-restoration escape sequence is
required. CLI integration and desktop process tests exercise these boundaries.
