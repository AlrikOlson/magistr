# Claude Code adapter

The optional plugin in `plugins/magistr` is a Claude-specific presentation
adapter. It depends on connected Magistr and Workbench MCP capabilities.
Neither native service depends on the plugin. Codex, ACP, CLI, cockpit,
desktop and headless paths retain their existing owners.

## Decision

Native family use cases own rules, mutations, recorded verification and recovery.
Workbench owns source transactions, confirmations and journal receipts. Claude
owns its session, permission dialogs and plugin lifecycle. The Mod owns only
disposable selections, loading states and bounded views; persistent preferences
contain the progress-band choice only.

Read-only calls use `$.mcp.call` on existing registrations. This API does not
prompt for host permission. Consequential actions therefore confirm the named
target, then use `$.tool.call` through Claude's normal tool/permission/hooks
path. The adapter never approves a permission, edits source, completes work,
submits a prompt, calls a model, starts a scheduler or opens native persistence.

The existing family query contracts supply work, roadmap and findings.
`magistr_context` adds only the admitted service's canonical root and owner epoch,
an explicitly
announced phase (nullable), bounded canonical task evidence, check receipts,
and configured check choices resolved by the
canonical gate resolver. Phase is an observation of this service's successful
`magistr_set_phase`, scoped to its objective creation time and active task.
A new owner, changed objective/task, or a phase announced only through another
writer means unknown; prose and timestamps are never used to infer a phase.
Check choices describe commands; they neither authorize nor execute them.

## Consistency and effects

Decode, validate and copy every reply at the adapter edge, including MCP errors,
native refusal envelopes and Workbench versioned envelopes. Keep the four
workflow families separate. A successful read means observed at that read,
not atomic with other queries. After a refresh, reject disagreement between
workflow project identities; task detail replies must match the selected task
and connected owner. Owner epoch, event sequence, record revision,
snapshot cursor and source revision retain their distinct meanings. The Mod
does not subscribe to or fabricate a feed cursor. Workbench's existing status
root is compared with the context's canonical root before actions. Failed or
mismatched connections refuse actions and mark source views out of date.
Workstream choices use native per-caller focus under this Claude session's lane;
the adapter reads it back and leaves other agents' focus untouched.

Rendering consumes bounded plain state and creates elements only. Reads,
actions, clock and preference effects run in handlers. Refresh is serialized,
coalesces acknowledged operations, fences responses by a disposable generation
and preserves previous data as stale on failure. Poll only while visible or
explicitly enabled, with bounded concurrency and no mutation retries.
Reads time out after twelve seconds but retain their raw in-flight slot until
settled; at most two can exist, even when a server never answers. Detail requests
retain only the latest queued selection. One action latch survives pane close
and session clear until the normal tool request ends; no mutation is retried.
Cancellation or owner loss never proves a dispatched action was undone;
read back before offering another submission. Reload drops observations and
host-owned timers; session end fences outstanding replies.

Observational hooks return the original event/result unchanged and ignore
their own calls. The band composes the next plugin's tree. Existing Magistr
skills, shell guards, MCP configuration and export denials remain independent.

## Packaging and compatibility

Claude owns installation, versioned cache, upgrade, disablement and removal.
The package contains no MCP launcher or generated project instructions, so
installing it cannot create a competing owner or rewrite managed/user files.
Use a local marketplace or a session-only directory. Keep edits in the source
directory; use a version bump for installed upgrades. Managed restrictions and
disabled Mods retain existing native tools and skills.

Target Claude Code 2.1.287+ with third-party Mods enabled by the host.
The public type bundle inspected was 2.1.277; it is supporting evidence only,
not the installed build contract. Release acceptance requires generated types
from the installed build and
the official hook/interaction harness. Terminal and Desktop use shared supported
Box/Text/Button/Input/Select elements; other hosts receive text replies.
Windows currently has no workflow MCP attachment to an already held Service
owner; the Mod reports this failure, never takes its lock. Test the real host
with a separate fixture project where no owner is already held. Desktop tree
tests establish element compatibility, not evidence of native Desktop painting.

Official contract: [overview](https://code.claude.com/docs/en/plugins/mods/overview),
[API](https://code.claude.com/docs/en/plugins/mods/api),
[events](https://code.claude.com/docs/en/plugins/mods/events),
[interface](https://code.claude.com/docs/en/plugins/mods/interface),
[reference](https://code.claude.com/docs/en/plugins/mods/reference),
[tests](https://code.claude.com/docs/en/plugins/mods/test),
[administration](https://code.claude.com/docs/en/plugins/mods/admin).
Reviewed 2 October 2026 alongside the official diff example.
