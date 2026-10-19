# Command and protocol interfaces

`magistr` is a plain command-line application. With no subcommand it prints
help. `run`, `next` and `next-loop` stream progress to standard output;
`--headless` provides noninteractive execution. Commands never take over the
terminal, negotiate graphics, enable mouse tracking or enter an alternate screen.

The CLI owns one plain-text renderer for the shared session stream. It flushes
assistant text before tool activity, reports commands and tool outcomes, and
shows concise findings, phases and check results. Long tool results have bounded
previews that retain the beginning and end and identify omitted lines. JSON
execution emits tool updates and results as separate protocol records. Workflow
commands drain one turn; the loop owns continuation, while interactive `run`
alone asks for follow-up input.

`next-loop` holds the loop lease for its checkpoint and uses `NativeReader`
for roadmap progress and recovery. It can run beside Codex's background MCP
service without taking workflow ownership. The agent's MCP tools attach to the
project owner or admit one when needed; the loop publishes no workflow endpoint.

Workflow libraries return facts through native records and tool replies.
Routine retention, findings and capability diagnostics use opt-in debug tracing;
genuine failures retain their warning severity. They never print directly into
a host's terminal. Progress counts unfinished work separately from the single
candidate selected for an iteration.

The desktop has one independent launch route, described in
[desktop architecture](desktop.md). Presentation owns no provider, workspace
or workflow service lifetime.

## Retained protocols

Provider terminals remain application effects. A provider may create a command
process, read its output, wait, cancel and release it through the admitted
effect adapter. `magistr-process` owns PTYs, Unix process groups and reaping.
This capability is independent of the Magistr interface.

`magistr mcp --dir <project>` serves workflow and source tools through one connection. The private `--workspace-worker` role handles
bounded parser and sandbox requests; `acp-agent` serves the selected provider.
Both CLI and desktop executables dispatch these roles before initializing a
window or project. Protocol stdout contains only protocol frames.

SIGINT requests orderly shutdown of the run and its owned children. Hard exits
use the registered child guard. No terminal-restoration escape sequence is
required. CLI integration and desktop process tests exercise these boundaries.
