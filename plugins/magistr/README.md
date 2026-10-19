# Magistr for Claude Code

Inspect your current work, roadmap, check evidence, recorded findings and
Magistr changes inside your coding session. Open **/magistr** and use the
Work, Roadmap, Checks, Findings and Changes tabs. The optional progress band
shows your current work, reported phase, check state and connection health.

This package is a preview pending live Claude host acceptance. The implementation
and independent checks are available; the verification report names the remaining
host restrictions.

## Install

You need Claude Code **2.1.287 or later**, with third-party Mods available to your
account and permitted by your organization. You also need the project's connected
`magistr` connection, launched with `magistr mcp --dir <project>`. Its `context`
query supplies phase, check choices and task evidence. Advanced workflow and
workspace queries use bounded `execute` requests on the same connection.

From a Magistr checkout:

```sh
claude plugin validate .
claude plugin marketplace add ./
claude plugin install magistr@magistr --scope user
```

After this revision is published, the marketplace can also be registered with
`claude plugin marketplace add AlrikOlson/magistr`.

Restart Claude normally after installation. Use **/mcp** to inspect the existing
connections, **/magistr diagnose** to check the views, and **/magistr** to open
the pane. For a session-only trial, use `claude --plugin-dir ./plugins/magistr`.

The plugin installs no servers and changes no project instructions or guards.
Set up the connection with `magistr init`. A second workflow
connection cannot attach to an occupied Service owner on Windows yet. Keep the
owner running; the pane reports the failed connection. A fresh project without
an existing owner can use the normal startup path.

## Use

- **Work:** inspect your objective, acceptance criteria and tasks; open a task
  to see check commands, results, recorded actions and source links.
- **Roadmap:** inspect readiness, blockers, acceptance criteria and dependencies.
  Choose a workstream for this Claude session or mark the next ready work as started. Other sessions keep their own workstream choices.
- **Checks:** inspect recorded evidence or run a check selected for the current
  change. Set a time limit from 1 to 1800 seconds (five minutes by default),
  then review the exact command and limit before confirming.
- **Findings:** inspect pinned or recent findings and their supporting findings.
- **Changes:** inspect Magistr's recorded changes and read the affected files.
  File changes made outside Magistr may also exist.

Task starts, workstream choices, work starts and checks require a specific
confirmation and then Claude's usual permission checks. Starting work records
your choice; continue implementation with your existing coding workflow.
The pane never completes work or launches a coding loop.

Use Tab to move, Enter to choose, 1–5 for tabs, R to refresh, P for the progress
band, B to return from details and Esc to close. Controls stack in narrow windows.
The pane uses your host's text and button theme. It draws on terminal and Desktop
surfaces supported by Claude; elsewhere commands return text.

Useful text commands:

```text
/magistr current
/magistr roadmap
/magistr checks
/magistr findings
/magistr changes
/magistr task <full title>
/magistr work <full title>
/magistr finding <number>
/magistr band on
/magistr band off
/magistr refresh
/magistr diagnose
/magistr help
```

If /magistr is already used by another plugin, its command is left intact.
Use **/magistr:status** to inspect work through the ordinary skill instead.

If Mods cannot load, **/magistr:status** is the separately named, read-only
plugin skill. Existing /craft, /next and /validate skills keep their roles.
A disabled plugin also disables its skill; use your existing Magistr skills
or native commands in that case.

## Progress and recovery

Each view is observed separately. “Current at last refresh” means the latest
read succeeded; it does not promise that every service changed at the same time.
The reported phase can be unknown. Recorded checks describe what ran, and
their source receipts describe the files checked at the time.

Visible views refresh every 30 seconds and after relevant acknowledged tool
operations. After 45 seconds without a successful read they show “Out of date”.
Failed refreshes retain the previous view with its error. There are at most two
read calls and one action in flight. A read times out after 12 seconds, but its
slot remains held until the server answers. No action is retried automatically.

After an interrupted or ambiguous request, refresh and inspect its record before
requesting it again. Closing a pane cancels a choice that has not been submitted;
it does not undo a submitted native action. Reload drops the display state.
The only saved preference is whether to show the progress band.

## Diagnose, upgrade and remove

Check `claude --version`, `claude plugin details magistr` and
`claude plugin list`. If the host reports **the rollout switch served off**,
Anthropic has disabled third-party Mods for that process. No local setting
reenables them. Managed restrictions can also refuse a module or API call.
The plugin respects those refusals. Use the text skill or existing workflows
where allowed. Restore an expired Claude sign-in through the normal sign-in flow.

For a local checkout, pull the new revision, then run:

```sh
claude plugin marketplace update magistr
claude plugin update magistr@magistr --scope user
```

Restart Claude to apply the update. The plugin shares Magistr's release version;
maintainers change it with `python3 scripts/version.py set <version>`. Local source
edits do not replace a versioned installed cache. Use `--plugin-dir` while developing. Repair by validating the source and
reinstalling through Claude's plugin manager; do not copy over project settings,
managed files or user edits.

```sh
claude plugin disable magistr@magistr --scope user
claude plugin enable magistr@magistr --scope user
claude plugin uninstall magistr@magistr --scope user
claude plugin marketplace remove magistr
```

Claude owns its installation and cache. These commands preserve the checkout,
native state, MCP registrations, project instructions and user changes.

[Architecture](https://github.com/AlrikOlson/magistr/blob/main/docs/architecture/claude-mod.md)
and [verification evidence](https://github.com/AlrikOlson/magistr/blob/main/docs/reference/claude-mod-verification.md)
describe ownership, tested boundaries and exact host limitations.
