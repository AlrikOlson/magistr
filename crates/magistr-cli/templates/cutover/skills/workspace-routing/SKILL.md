---
name: workspace-routing
description: Use Magistr workspace operations for source search, reading, verified changes, file operations, commands and session-scoped undo. Apply whenever the Magistr MCP connection is configured.
---

# Workspace routing

The `magistr` connection's instructions cover direct tools, `discover` and
`execute`. This skill is the reference for source work: which tool replaces
which shell habit, how to read a reply, how to edit safely, and what each error
means. For Rust patterns in `mode:structural` and `op:rewrite`, read
[Rust structural patterns](references/rust-patterns.md).

## When to reach for workspace (and when not)

Reach for workspace for everything that touches the workspace's files as *content*: finding,
reading, orienting, changing, moving, reverting, and batch manipulation. Every answer comes back
ranked and grouped by file, trimmed to a token budget, with line numbers verified fresh, and every
write is parse-verified and journaled in the same call.

The shell is still right for things that are not about file content:

- `git` (status, diff, log, commit, branch) — workspace never wraps git.
- Installing dependencies, compiling, starting servers — use `command` when the project has declared
  the command, otherwise the shell.
- Filtering another command's output (`… | grep`, `… | tail`) — that is output handling, not search.

Do not use the shell to grep, cat, sed, mv or rm workspace files when workspace is configured:
a raw grep returns a dump instead of a ranking, a raw sed can leave a file syntactically broken,
and neither is journaled, so `undo` cannot see it.

## Reading an envelope

Every tool returns the same envelope (`v` is its format version; pin yours with `_meta`
`magistr/maxEnvelopeVersion` if you need stability).

- `ok: true` → `results` (always ranked and grouped by file), `freshness` (`cold` | `warming` |
  `warm` — how warm the index was when this was answered), `budget` (`requested` vs `used`
  tokens), optional `notes` (degradations, progress, fidelity caveats — read them), and
  `truncation` **only if** results were cut: `dropped` (how many), `reason`, `narrow` (typed hints
  such as `kind:function` or `path:src/**` — apply one and re-call), `cursor` (continue where it
  stopped).
- `ok: false` → `error.code` (one of the taxonomy codes in the recovery table below),
  `error.recovery` (`retry` | `narrow` | `confirm` | `escalate` | `abort`), `error.message`, and
  code-specific payload (`candidates`, `parserMessage`, `rejectedDiff`, `contentHash`,
  `backoffMs`, `plugin`, `confirmToken`).

Budgets are parameters, not limits you fight: if you need more, raise `budget`; if you need less
noise, narrow `path` or `kind`. A cut is never silent, so you never have to guess whether you saw
everything.

Identity travels on the request: set `_meta` `magistr/sessionId` to a stable id for your session
so `undo` with `scope:session` only touches your own ops.

## Routing table

One row names exactly one tool. `facet:value` picks a selection (`mode`, `op`, `detail`, `surround`,
`scope`, `onConflict`); the last column lists the arguments that decide the answer.

| Instead of | Use | Arguments that matter |
|---|---|---|
| `grep -rn`, `rg`, `ag` for text or a regex | `search` `mode:auto` — lexical always answers, and escalates to structural/references when the index is warm | `query`, `path`, `context`, `include`, `budget` |
| `grep -F` when you want exactly literal/regex hits and no escalation | `search` `mode:lexical` | `query`, `path`, `context` |
| grep for a *shape* ("every call to X", "every `await` in a loop") | `search` `mode:structural` — an ast-grep pattern with $VAR metavariables | `query`, `kind`, `path` |
| every macro invocation or attribute in Rust (`println!`, `#[derive]`) | `search` `mode:structural` — patterns like $M!($$$ARGS) or #[derive($$$D)]; inside a macro $X is ONE token, use $$$X for an expression (see references/rust-patterns.md) | `query`, `kind`, `path` |
| every impl / trait / fn shape in Rust | `search` `mode:structural` — a pattern like impl<$$$G> $T for $U { $$$B }; visibility, generics, where and async are part of the shape (see references/rust-patterns.md) | `query`, `kind`, `path` |
| `grep -rn` for callers, importers, usages of a symbol | `search` `mode:references` — needs a warm index (INDEX_UNAVAILABLE until then, see recovery) | `query`, `path` |
| `find`, `glob`, `ls` for files by name or pattern | `search` `mode:files` — filename/glob search in the same envelope | `query`, `path`, `budget` |
| "semantic" / embedding search | `search` `mode:semantic` — **rejected in v1** with a typed error; write a better lexical query with `mode:auto` instead | `query` |
| `ls -R`, `tree`, orientation greps in a new repo | `map` `detail:skeleton` — pruned directory tree, entry points, project commands, index health, plugin inventory; in a Cargo workspace also one ranked crate row per member (roles, entry points, features, member dependencies with importing-file counts, dependents) and a cargo summary on the overview | `path`, `depth`, `budget` |
| "what's in this module?" before reading it | `map` `detail:outline` — adds compressed per-file symbol outlines when the index is warm | `path`, `depth` |
| full outlines of a small subtree | `map` `detail:full` — outlines without the per-file symbol cap (the budget still trims) | `path`, `depth`, `budget` |
| `cat`, `head`, `tail`, `sed -n 10,40p` | `read` — whole file, or a 1-based inclusive line range | `file`, `lines`, `budget` |
| reading one function/class/method | `read` `surround:none` — exactly the addressed symbol, with line anchors and a content hash | `file`, `symbol` |
| reading a method *with* its class around it | `read` `surround:enclosing` — the addressed symbol expanded to its parent symbol (default) | `file`, `symbol` |
| reading a symbol plus the file's imports | `read` `surround:imports` — the symbol, its parent, and the leading import block | `file`, `symbol` |
| `sed -i 's/old/new/'` on one file | `edit` `op:replace_text` — anchored literal replacement for any file, source or not | `file`, `old`, `new`, `occurrence`, `expectedHash` |
| rewriting a whole function body | `edit` `op:replace_body` — replaces the addressed symbol's definition | `file`, `symbol`, `content` |
| adding code right after a symbol | `edit` `op:insert_after` | `file`, `symbol`, `content` |
| adding code right before a symbol | `edit` `op:insert_before` | `file`, `symbol`, `content` |
| renaming a symbol across the repo | `edit` `op:rename` — follows the configured confirmation policy | `file`, `symbol`, `newName`, `confirm` |
| `sed -i` / `awk` across many files (a codemod) | `edit` `op:rewrite` — an ast-grep pattern → replacement over a glob scope; always dry-run first | `pattern`, `rewritePattern`, `path`, `lang`, `dryRun`, `confirm` |
| `mv` | `files` `op:move` — import-aware: importer rewrites ride the same diff; follows the configured confirmation policy | `from`, `to`, `confirm`, `dryRun` |
| `cp` | `files` `op:copy` | `from`, `to` |
| `touch`, an empty new file | `files` `op:create` | `to` |
| creating a file with content, `cat > file`, replacing a whole file | `write` — exact bytes, parse-verified, journaled, undoable | `path`, `content`, `expectedHash` |
| `mkdir -p` | `files` `op:mkdir` | `to` |
| `rm` | `files` `op:delete` — a journal-backed move to trash, undoable | `from`, `expectedHash` |
| running the tests / build / lint | `command` — one declared command from the [commands] table, parsed output, no shell | `command`, `args`, `timeout` |
| a bash one-liner or pipeline over many files | `script` — typed Code Mode in a sandbox; reads live, writes queued behind the confirmation step | `source`, `dryRun`, `confirm`, `fuel`, `memoryCapBytes` |
| `git checkout --` on your own last edit | `undo` `scope:session` `onConflict:surface` — rewind your session's last op; conflicts are reported, nothing forced | `steps` |
| reverting an op from *any* session | `undo` `scope:workspace` — cross-session; say so to the human first | `steps` |
| undoing when someone changed the file since | `undo` `onConflict:force` to restore the journaled content, or `onConflict:keepCurrent` to keep their change and undo the rest | `steps`, `scope` |
| "is the server healthy / what tier is the index?" | `workspace_status` — no arguments | — |

## Search: the auto → references escalation

Start with `mode:auto`. Lexical runs at Tier 0 (no index), so you get an answer within seconds of a
cold start. When the query names a symbol and the index is warm, structural classification enriches
and re-ranks the same results — you do not have to ask for it. Only `mode:references` *requires* the
warm index; until then it answers `INDEX_UNAVAILABLE` with `recovery:retry` and build progress in
`notes`. The same refusal can follow a file change while its outline refreshes.
If your client exposes MCP task polling, poll `tasks/get` for `index/build` using
its returned interval and inspect failed builds. Otherwise use `read` without a
symbol, then `edit(op:"replace_text")` with a current anchor and `expectedHash`.
A completed build does not guarantee every file has an outline; inspect the reply's
notes for excluded, oversized or changed files. Do not repeat an unchanged request
or add arbitrary waits.

References are tree-sitter fidelity (name-resolution-free) unless a resolver plugin upgraded them;
each edge carries `confidence` and the envelope says so. Treat `high` as real, `medium` as "verify
before editing".


## Editing safely

Three verb families, one pipeline: workspace lock → dry-run diff → confirmation → parse-verified
apply → format → journal → atomic swap. A batch that would break syntax rolls back atomically with
`PARSE_REJECTED`; nothing half-applies.

- Prefer symbol verbs (`replace_body`, `insert_after`, `insert_before`, `rename`) for code; they
  address by `symbol` (same dialect as `read`) and need the warm index.
- Use `rewrite` for repo-wide codemods; it is structural on the native core and says in `notes`
  when a degraded path applied it literally. For Rust, the same three facts as structural search
  apply (see references/rust-patterns.md): `$$$` carries macro arguments, modifiers are part of the shape, attributes rewrite alone.
- Use `replace_text` for non-code files or when you truly want a literal anchor; pass `expectedHash`
  from your `read` so a file changed under you answers `STALE_ANCHOR` instead of clobbering.
- `edit.confirm_mode = "token"`, the default, applies verified edits, writes, file operations
  and script batches on the first call. The caller does not need to echo its own confirmation
  token. Parse verification, path protection, staleness checks, journaling and undo still apply.
- Human confirmation mode (`edit.confirm_mode = "elicit"`, opt-in only) and
  `edit.force_confirm = true` hold multi-file changes, rewrites, renames, moves, overwriting
  copies, script batches and single-file writes over 200 changed lines. Review the returned
  diff and re-issue the identical call with `confirm` set to `confirmToken`; elicitation-capable
  clients can approve in-call. Token mode never opens an elicitation dialog.
- Existing user authorization carries across calls. Do not ask the user to authorize the same
  work again because the server returned `CONFIRM_REQUIRED`. For already-authorized work on a
  running server, use `autoApply: true`; review broad or uncertain effects with `dryRun: true`
  first. An operation outside the authorized task still needs its own scope decision.
- `write` is how a new file gets its content and how a whole file is replaced: `path` plus
  `content`, exact bytes, no formatter. Pass `expectedHash` from your `read` when replacing.
  Prefer `edit` when only part of a file changes.
- `autoApply: true` skips the additional confirmation round trip for authorized work. It does
  not skip verification, path protection, staleness checks, journaling or returned applied diffs.
- `dryRun: true` runs the whole pipeline through re-verify and applies nothing — use it liberally.

## Files, command, script, undo

- `files` `op:move` rewrites importers (specifier-based, name-resolution-free — the notes say
  exactly what was rewritten); for a `.rs` file it is module-aware: the declaring `mod` item, the
  crate's `crate::`/`super::`/`self::` paths and Cargo.toml target `path` entries ride the same
  diff, and the notes name what was left for you (brace-grouped `use` members, non-leaf modules).
  `op:delete` is a move to `.magistr/workspace/trash/`, so it is undoable.
- `command` executes one declared command as a single argv, never a shell. Shell operators in `args`
  are rejected, not interpreted. If the command is not declared (`COMMAND_NOT_DECLARED`) the human
  adds it to `.magistr/workspace.toml` (`magistr workspace init` seeds it) — you cannot, and you must not try to
  reach it through `script` or any other door. Text or file munging through `command` answers
  `RUN_REDIRECTED`: that job belongs to `search`/`read`/`edit`/`files`. Results come back as
  rows, not text: a `report` row (test failures — name, file, assertion, trimmed stack — from
  bun/jest/vitest/pytest/cargo test/cargo nextest/go), a `diagnostics` row (rustc/clippy
  diagnostics with `file:line:column` and the help text, errors first, from `cargo
  build`/`check`/`clippy` or a test run that failed to compile), or an `output` row (head+tail
  with a truncation manifest) when nothing claimed the command.
- `script` is for the long tail bash used to own: ad-hoc batch work over many files. The guest sees
  one global, `workspace` (`search`, `read`, `glob`, `edit.queue`, `files.queue`); reads execute
  live, writes are queued and come back as one dry-run diff behind the same confirmation step. It is
  fuel-metered and memory-capped; `fuel` and `memoryCapBytes` only clamp downward.
- `undo` rewinds whole ops (write batches), newest first, from the durable journal. Default scope
  is your session; `scope:workspace` can revert another agent's work, so say so. On conflict the
  default `onConflict:surface` applies nothing and shows expected/current hashes.

## Plugin tools

Installed plugins add operations under `<publisher>.<pkg>/<tool>` to the catalog. Their names
depend on the workspace, so this skill does not route them statically: discover them with
`discover`, and read their trust tier in `map`'s plugin inventory. Every plugin answer is wrapped
in the same envelope; plugin writes queue into the same write engine. A breakered or crashed plugin
answers `PLUGIN_CRASHED` and the core tools keep working at full fidelity.

## Recovery table

`recovery` is the taxonomy's; the last column is what it means for you.

| Code | Recovery | What to do |
|---|---|---|
| `PATH_OUTSIDE_WORKSPACE` | `abort` | The path escapes the workspace root. Do not try `..` or absolute tricks; stay inside the root or ask the human. |
| `PROTECTED_PATH` | `escalate` | `.git/**`, `.magistr/workspace/**`, config layers and the `[commands]` table are read-only to every tool. A human edits those by hand; tell them what you wanted. |
| `AMBIGUOUS_SYMBOL` | `narrow` | Pick one of `candidates` (qualified, e.g. `UserService.authenticate`) and re-call. |
| `SYMBOL_NOT_FOUND` | `narrow` | `search` `mode:auto` (or `mode:references`) for the name, then use the file + address it returns. |
| `FILE_NOT_FOUND` | `narrow` | The in-root path does not exist. `search` `mode:files` to find it; do not create it by accident. |
| `PARSE_REJECTED` | `narrow` | Your edit would break syntax; nothing was applied. Read `parserMessage` and `rejectedDiff`, fix `content`/`new`, re-issue. |
| `STALE_ANCHOR` | `retry` | The file changed under you. `read` it again, take the fresh `contentHash` into `expectedHash` (or re-anchor `old`), re-issue once. |
| `PATTERN_INVALID` | `narrow` | The ast-grep/glob pattern did not parse. Fix it (balanced `$VAR`s, valid glob) and re-issue. |
| `BUDGET_TOO_SMALL` | `narrow` | Raise `budget`, or narrow `path`/`kind` so the answer fits. |
| `COMMAND_NOT_DECLARED` | `abort` | Not in `[commands]`. Tell the human to declare it in `.magistr/workspace.toml`; never route it through `script` or a shell. |
| `RUN_REDIRECTED` | `abort` | You tried text/file munging or a shell through `command`. Use `search`/`read`/`edit`/`files` for that job. |
| `SANDBOX_FUEL_EXCEEDED` | `narrow` | The script did too much in one round. Scope fewer files (`path`) or split the work across calls. |
| `SANDBOX_CAP_DENIED` | `escalate` | The guest asked for a capability it does not have (network, ambient filesystem). By design; tell the human if the task truly needs it. |
| `SANDBOX_MEMORY_EXCEEDED` | `narrow` | Reduce the working set: process fewer files per call, stream instead of accumulating. |
| `INDEX_UNAVAILABLE` | `retry` | A current outline is unavailable; inspect `notes`. Use lexical search or plain `read` plus anchored `replace_text`. If exposed by the client, poll `tasks/get` for `index/build` at its returned interval; inspect failures and do not loop on an unchanged refusal. |
| `CONFIRM_REQUIRED` | `confirm` | Review the returned diff and continue within existing authorization: use `autoApply: true` for already-authorized work, or re-issue the identical call with `confirm` = `confirmToken`. Do not ask the user to repeat authorization. |
| `CONFIRM_MISMATCH` | `confirm` | The token is not for this exact call. Redo the dry-run to mint a fresh token; never reuse tokens across calls. |
| `UNSUPPORTED_VERSION` | `abort` | Envelope or on-disk format mismatch. Do not retry; tell the human to upgrade the client/binary. |
| `ENGINE_UNAVAILABLE` | `retry` | The engine child is restarting. Retry after a short pause; lexical search still answers meanwhile. |
| `ENGINE_BUSY` | `retry` | The bounded queue is full. Wait `backoffMs`, then retry. |
| `INPUT_QUARANTINED` | `escalate` | This file repeatedly crashed the parser and is quarantined. Stop parse-verified verbs on it; point the human at `magistr workspace doctor .`. |
| `PLUGIN_CAP_DENIED` | `escalate` | A plugin exceeded its granted capabilities. Not yours to fix; tell the human which plugin. |
| `PLUGIN_CRASHED` | `abort` | The named plugin trapped or ran out of fuel. Do not retry the same plugin call; fall back to the core tools. |
| `CONFIG_POLICY_DENIED` | `abort` | Org policy forbids it; the message names the layer. Tell the human; do not route around it. |

## Worked examples

One per tool. Each block is a complete, valid call.

```json magistr:workspace_status
{}
```

```json magistr:search
{ "query": "parseConfig", "mode": "references", "path": "src/**", "context": 2, "budget": 3000 }
```

```json magistr:map
{ "path": ".", "depth": 2, "detail": "outline", "budget": 6000 }
```

```json magistr:read
{ "file": "src/auth/service.ts", "symbol": "UserService.authenticate", "surround": "enclosing" }
```

```json magistr:edit
{
  "op": "rewrite",
  "pattern": "console.log($ARGS)",
  "rewritePattern": "logger.debug($ARGS)",
  "path": "src/**",
  "lang": "ts",
  "dryRun": true
}
```

```json magistr:files
{ "op": "move", "from": "src/utils/date.ts", "to": "src/lib/date.ts", "dryRun": true }
```

```json magistr:write
{ "path": "src/lib/clock.ts", "content": "export const now = (): number => Date.now();\n" }
```

```json magistr:command
{ "command": "test", "args": ["--filter", "auth"], "timeout": 300 }
```

```json magistr:script
{
  "source": "const hits = workspace.search({ query: 'TODO', mode: 'lexical', path: 'src/**' });\nreturn hits.results.map((g) => g.file);",
  "dryRun": true
}
```

```json magistr:undo
{ "steps": 1, "scope": "session", "onConflict": "surface" }
```
