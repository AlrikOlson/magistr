# Upgrade contracts

User instructions for [updating and recovering an installation](https://magistr.dev/docs/upgrades)
live in the product manual. This contributor reference describes the recognized
conversion formats, owning components, and recovery copies.

Magistr upgrades recognized older project settings, agent launch commands and
saved work when you start it. `magistr repair . --yes` applies the same
conversions and checks the connection. `magistr repair . --dry-run` previews
configuration changes without writing files or importing history.

Successful automatic upgrades do not prompt or print notices. Custom commands,
other MCP servers, environment settings and agent preferences are preserved.

## What moves

| Existing data | Current location |
| --- | --- |
| `roadmap_backend: "think-and-ship"` | `"native"` in the existing project configuration |
| Saved `magistr mcp-launch magistr …` registration | `magistr mcp --dir …`, preserving the directory and extra connection settings |
| An older declared project identity, when the current settings do not declare one | `project.id` in `.magistr/project.json` |
| Recognized workflow documents for that identity in the default older workflow directory | The corresponding project records under `magistr/workflow` in the user data directory |
| `workbench.toml` | `.magistr/workspace.toml`, retaining custom values and comments |
| Durable operations in `.workbench/journal.db` | `.magistr/workspace/journal.db`, including snapshots, sessions, undo links and recorded write outcomes |

The workspace index is rebuilt as needed. Old sockets, process files, indexes and
confirmation keys are not reused. A migrated workspace journal keeps current
operation IDs stable; imported operations keep their relative order before
current history.

## Recovery copies

Original settings are saved under `.magistr/upgrades/configuration/`.
The retired workspace configuration is archived under
`.magistr/upgrades/workspace-config/`. The original `.workbench` journal and
older workflow records remain available as recovery copies. The old workspace
lock is marked as retired so an older server cannot reopen that journal.

`.magistr/upgrades/workflow.json` records a requested workflow move and its
completion. Interrupted upgrades retry safely. An unfinished workflow move is
reported as unfinished, rather than shown as an empty project.

## When an upgrade needs attention

If an older session still owns the project, close that session and retry.
Magistr never terminates it or deletes its lock.

If two settings or saved records disagree, Magistr identifies the conflict and
preserves both copies. Unknown storage versions, malformed data and symbolic
links are refused rather than replaced with defaults. Historical workflow
schemas that the current readers cannot validate require a separate conversion;
the original records remain untouched. An unrecognized custom storage layout
also requires locating the previous store.

Full scaffold repair remains explicit. Automatic upgrades do not regenerate
customized instructions or restore files you intentionally removed.
