---
name: workbench-routing
description: Route shell and harness habits (grep, find, ls, cat, sed, mv, rm, running tests, ad-hoc scripts, reverting) to the workbench-mcp tools, read the response envelope, and recover from every error code. Use whenever a workbench-mcp server is configured for the workspace.
---

# Workbench routing

workbench-mcp replaces the shell's file substrate with eight tools that return one ranked, budgeted,
verified envelope: `search`, `map`, `read`, `edit`, `files`, `run`, `script`, `undo`, plus the
schema-less `workbench_status`. This skill is the judgment layer on top of that surface: which
habit goes where, what the envelope is telling you, and what each error code wants you to do next.

It is written by hand. A CI check (`bun run lint:skill`) fails whenever the routing table, the
recovery table, or the worked examples below disagree with the published tool schemas or the error
taxonomy, in either direction. The prose between them is opinion; the tables are contract.

## When to reach for workbench (and when not)

Reach for workbench for everything that touches the workspace's files as *content*: finding,
reading, orienting, changing, moving, reverting, and batch manipulation. Every answer comes back
ranked and grouped by file, trimmed to a token budget, with line numbers verified fresh, and every
write is parse-verified and journaled in the same call.

The shell is still right for things that are not about file content:

- `git` (status, diff, log, commit, branch) — workbench never wraps git.
- Installing dependencies, compiling, starting servers — use `run` when the project has declared
  the command, otherwise the shell.
- Filtering another command's output (`… | grep`, `… | tail`) — that is output handling, not search.

Do not use the shell to grep, cat, sed, mv or rm workspace files when workbench is configured:
a raw grep returns a dump instead of a ranking, a raw sed can leave a file syntactically broken,
and neither is journaled, so `undo` cannot see it.

## Reading an envelope

Every tool returns the same envelope (`v` is its format version; pin yours with `_meta`
`workbench/maxEnvelopeVersion` if you need stability).

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

Identity travels on the request: set `_meta` `workbench/sessionId` to a stable id for your session
so `undo` with `scope:session` only touches your own ops.

## Routing table

One row names exactly one tool. `facet:value` picks a selection (`mode`, `op`, `detail`, `surround`,
`scope`, `onConflict`); the last column lists the arguments that decide the answer.

| Instead of | Use | Arguments that matter |
|---|---|---|
| `grep -rn`, `rg`, `ag` for text or a regex | `search` `mode:auto` — lexical always answers, and escalates to structural/references when the index is warm | `query`, `path`, `context`, `include`, `budget` |
| `grep -F` when you want exactly literal/regex hits and no escalation | `search` `mode:lexical` | `query`, `path`, `context` |
| grep for a *shape* ("every call to X", "every `await` in a loop") | `search` `mode:structural` — an ast-grep pattern with $VAR metavariables | `query`, `kind`, `path` |
| every macro invocation or attribute in Rust (`println!`, `#[derive]`) | `search` `mode:structural` — patterns like $M!($$$ARGS) or #[derive($$$D)]; inside a macro $X is ONE token, use $$$X for an expression (see Rust structural patterns) | `query`, `kind`, `path` |
| every impl / trait / fn shape in Rust | `search` `mode:structural` — a pattern like impl<$$$G> $T for $U { $$$B }; visibility, generics, where and async are part of the shape (see Rust structural patterns) | `query`, `kind`, `path` |
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
| renaming a symbol across the repo | `edit` `op:rename` — multi-file, so confirm-gated | `file`, `symbol`, `newName`, `confirm` |
| `sed -i` / `awk` across many files (a codemod) | `edit` `op:rewrite` — an ast-grep pattern → replacement over a glob scope; always dry-run first | `pattern`, `rewritePattern`, `path`, `lang`, `dryRun`, `confirm` |
| `mv` | `files` `op:move` — import-aware: importer rewrites ride the same diff; always confirm-gated | `from`, `to`, `confirm`, `dryRun` |
| `cp` | `files` `op:copy` | `from`, `to` |
| `touch`, creating a new file | `files` `op:create` | `to` |
| `mkdir -p` | `files` `op:mkdir` | `to` |
| `rm` | `files` `op:delete` — a journal-backed move to trash, undoable | `from`, `expectedHash` |
| running the tests / build / lint | `run` — one declared command from the [commands] table, parsed output, no shell | `command`, `args`, `timeout` |
| a bash one-liner or pipeline over many files | `script` — typed Code Mode in a sandbox; reads live, writes queued behind the confirm gate | `source`, `dryRun`, `confirm`, `fuel`, `memoryCapBytes` |
| `git checkout --` on your own last edit | `undo` `scope:session` `onConflict:surface` — rewind your session's last op; conflicts are reported, nothing forced | `steps` |
| reverting an op from *any* session | `undo` `scope:workspace` — cross-session; say so to the human first | `steps` |
| undoing when someone changed the file since | `undo` `onConflict:force` to restore the journaled content, or `onConflict:keepCurrent` to keep their change and undo the rest | `steps`, `scope` |
| "is the server healthy / what tier is the index?" | `workbench_status` — no arguments | — |

## Search: the auto → references escalation

Start with `mode:auto`. Lexical runs at Tier 0 (no index), so you get an answer within seconds of a
cold start. When the query names a symbol and the index is warm, structural classification enriches
and re-ranks the same results — you do not have to ask for it. Only `mode:references` *requires* the
warm index; until then it answers `INDEX_UNAVAILABLE` with `recovery:retry` and build progress in
`notes`. Do not loop on it: poll `tasks/get` for the `index/build` task or do something else and
come back; five identical retries in a row is the misuse pattern the flight recorder was built to
catch.

References are tree-sitter fidelity (name-resolution-free) unless a resolver plugin upgraded them;
each edge carries `confidence` and the envelope says so. Treat `high` as real, `medium` as "verify
before editing".

## Rust structural patterns

`mode:structural` and `op:rewrite` run ast-grep over tree-sitter-rust. Three facts decide whether a
Rust pattern hits (measured on the native core; `test/rust-structural.test.ts` keeps them true):

1. **Inside a macro's token tree `$X` matches one token.** `assert_eq!($A, $B)` matches
   `assert_eq!(v, 3)` but not `assert_eq!(1 + 1, 2)`; `dbg!($X)` misses `dbg!(&v)`. Use `$$$X` for
   an expression argument: `assert_eq!($$$A)`, `dbg!($$$X)`, `format!($FMT, $$$REST)`.
2. **Visibility, generics, `where` clauses and `async` are part of the shape.** A pattern must spell
   them to match items that carry them, and then matches only those: `impl $T for $U { $$$B }` never
   matches `impl<'a, T> Greeter for Config<'a, T>` (write `impl<$$$G> $T for $U { $$$B }`);
   `fn $F($$$P) -> $R { $$$B }` never matches a `pub fn`, a generic fn or an `async fn`.
3. **Attributes match on their own.** `#[derive($$$D)]`, `#[$A]`, `#[should_panic]` each match an
   `attribute_item`; joining the attribute to its item in one pattern (`#[test] fn $F() { $$$B }`)
   is rejected as multiple AST nodes — search the attribute, then the item.

| Works | Matches | Kind |
|---|---|---|
| `$M!($$$ARGS)`, `println!($$$ARGS)`, `vec![$$$ITEMS]`, `todo!()` | macro invocations | `macro_invocation` |
| `#[derive($$$D)]`, `#[$A]`, `#[should_panic]` | attributes | `attribute_item` |
| `impl<$$$G> $T for $U { $$$BODY }`, `impl $T { $$$BODY }`, header-only `impl $T` (loose) | impl blocks | `impl_item` |
| `pub trait $T { $$$B }`, `trait $T { $$$B }` | traits | `trait_item` |
| `fn $F($$$P) -> $R { $$$B }`, `pub async fn $F($$$P) -> $R { $$$B }`, `fn $F<$$$G>($$$P) -> $R where $$$W { $$$B }` | functions | `function_item` |
| `fn $F(&self) -> $R;`, parameter-only `fn $F($$$P)` | trait method signatures | `function_signature_item` |
| `pub struct $S<$$$G> { $$$F }`, `enum $E { $$$V }`, `const $C: $T = $V;`, `static …`, `type $A = $T;`, `mod $M;` | items | `struct_item`, `enum_item`, `const_item`, `static_item`, `type_item`, `mod_item` |
| `use $P;`, `use $A::{$$$ITEMS};` | imports | `use_declaration` |
| `$X.unwrap()`, `$RECV.$METHOD($$$ARGS)`, `Box::new($X)`, `$A::$B($$$ARGS)`, `Some($X)`, `$X.iter().map($F).collect()` | function AND method calls | `call_expression` |
| `$X?` | try | `try_expression` |
| `let $V = $E;`, `let $V: $T = $E;`, `let [$$$P] = $E;` | bindings (`let _ = …` is skipped: `_` is not an identifier node) | `let_declaration` |
| `match $E { $$$ARMS }`, `if let $P = $E { $$$B }`, `if $C { $$$T } else { $$$E }`, `for $I in $IT { $$$B }`, `while $C { $$$B }`, `loop { $$$B }` | control flow | `match_expression`, `if_expression`, `for_expression`, `while_expression`, `loop_expression` |
| `unsafe { $$$B }`, `async { $$$B }`, `$E.await`, `\|$$$P\| $B` | blocks, await, closures | `unsafe_block`, `async_block`, `await_expression`, `closure_expression` |
| `$E as $T`, `&mut $X`, `&$X`, `$S { $$$F }`, `$A + $B`, `$E == $V` | casts, references, struct literals, operators | `type_cast_expression`, `reference_expression`, `struct_expression`, `binary_expression` |

| Never matches | Why |
|---|---|
| `pub fn $F($$$P)`, `fn $F($$$P) -> $R` | a fn header without its body or `;` — add `{ $$$B }` (or `;` for a signature) |
| `&'static str`, `&'$L $T` | a type in type position is not a node ast-grep can root a pattern at — match the `let` or `fn` that carries it |
| `#[derive($$$D)] struct $S { $$$F }` | two AST nodes — rejected with `PATTERN_INVALID`; search the attribute and the item separately |
| `impl $T for $U { $$$B }` on a generic impl, `trait $T { … }` on a `pub trait` | fact 2: spell the modifiers |

The `kind` filter takes these node kinds; the shorthand is substring: `function` keeps `function_item`
and `function_signature_item`, `call` keeps `call_expression` (method calls included), `macro` keeps
`macro_invocation`, `impl` / `trait` / `struct` / `enum` keep their `*_item`, `attribute` keeps
`attribute_item`, `item` keeps every `*_item`. Rewrites carry `$VAR` and `$$$VARS` through macro
token trees and attribute lists (`#[derive($$$D)]` → `#[derive($$$D, Default)]`,
`println!($$$ARGS)` → `log::info!($$$ARGS)`), and the parse-verify guard still holds.

## Editing safely

Three verb families, one pipeline: workspace lock → dry-run diff → confirm gate → parse-verified
apply → format → journal → atomic swap. A batch that would break syntax rolls back atomically with
`PARSE_REJECTED`; nothing half-applies.

- Prefer symbol verbs (`replace_body`, `insert_after`, `insert_before`, `rename`) for code; they
  address by `symbol` (same dialect as `read`) and need the warm index.
- Use `rewrite` for repo-wide codemods; it is structural on the native core and says in `notes`
  when a degraded lane applied it literally. For Rust, the same three facts as structural search
  apply: `$$$` carries macro arguments, modifiers are part of the shape, attributes rewrite alone.
- Use `replace_text` for non-code files or when you truly want a literal anchor; pass `expectedHash`
  from your `read` so a file changed under you answers `STALE_ANCHOR` instead of clobbering.
- Single-file symbol/text edits auto-apply (parse-verify is the guard) and return the diff.
  Multi-file ops (`rewrite`, `rename`, `move`) and anything large answer `CONFIRM_REQUIRED` with the
  aggregated diff and a `confirmToken`. Read the diff. If it is right, re-issue the **identical**
  call with `confirm` set to the token. If your harness supports elicitation the server may ask
  you in-call instead; answering accept does the same thing.
- `autoApply: true` skips the human-approvable diff. Do not set it unless the human told you to.
- `dryRun: true` runs the whole pipeline through re-verify and applies nothing — use it liberally.

## Files, run, script, undo

- `files` `op:move` rewrites importers (specifier-based, name-resolution-free — the notes say
  exactly what was rewritten); for a `.rs` file it is module-aware: the declaring `mod` item, the
  crate's `crate::`/`super::`/`self::` paths and Cargo.toml target `path` entries ride the same
  diff, and the notes name what was left for you (brace-grouped `use` members, non-leaf modules).
  `op:delete` is a move to `.workbench/trash/`, so it is undoable.
- `run` executes one declared command as a single argv, never a shell. Shell operators in `args`
  are rejected, not interpreted. If the command is not declared (`COMMAND_NOT_DECLARED`) the human
  adds it to `workbench.toml` (`workbench-mcp init` seeds it) — you cannot, and you must not try to
  reach it through `script` or any other door. Text or file munging through `run` answers
  `RUN_REDIRECTED`: that job belongs to `search`/`read`/`edit`/`files`. Results come back as
  rows, not text: a `report` row (test failures — name, file, assertion, trimmed trace — from
  bun/jest/vitest/pytest/cargo test/cargo nextest/go), a `diagnostics` row (rustc/clippy
  diagnostics with `file:line:column` and the help text, errors first, from `cargo
  build`/`check`/`clippy` or a test run that failed to compile), or an `output` row (head+tail
  with a truncation manifest) when nothing claimed the command.
- `script` is for the long tail bash used to own: ad-hoc batch work over many files. The guest sees
  one global, `workspace` (`search`, `read`, `glob`, `edit.queue`, `files.queue`); reads execute
  live, writes are queued and come back as one dry-run diff behind the same confirm gate. It is
  fuel-metered and memory-capped; `fuel` and `memoryCapBytes` only clamp downward.
- `undo` rewinds whole ops (write batches), newest first, from the durable journal. Default scope
  is your session; `scope:workspace` can revert another agent's work, so say so. On conflict the
  default `onConflict:surface` applies nothing and shows expected/current hashes.

## Plugin tools

Installed plugins add tools under `<publisher>.<pkg>/<tool>` beside the core eight. Their names
depend on the workspace, so this skill does not route them statically: discover them with
`tools/list`, and read their trust tier in `map`'s plugin inventory. Every plugin answer is wrapped
in the same envelope; plugin writes queue into the same write engine. A breakered or crashed plugin
answers `PLUGIN_CRASHED` and the core tools keep working at full fidelity.

## Recovery table

`recovery` is the taxonomy's; the last column is what it means for you.

| Code | Recovery | What to do |
|---|---|---|
| `PATH_OUTSIDE_WORKSPACE` | `abort` | The path escapes the workspace root. Do not try `..` or absolute tricks; stay inside the root or ask the human. |
| `PROTECTED_PATH` | `escalate` | `.git/**`, `.workbench/**`, config layers and the `[commands]` table are read-only to every tool. A human edits those by hand; tell them what you wanted. |
| `AMBIGUOUS_SYMBOL` | `narrow` | Pick one of `candidates` (qualified, e.g. `UserService.authenticate`) and re-call. |
| `SYMBOL_NOT_FOUND` | `narrow` | `search` `mode:auto` (or `mode:references`) for the name, then use the file + address it returns. |
| `FILE_NOT_FOUND` | `narrow` | The in-root path does not exist. `search` `mode:files` to find it; do not create it by accident. |
| `PARSE_REJECTED` | `narrow` | Your edit would break syntax; nothing was applied. Read `parserMessage` and `rejectedDiff`, fix `content`/`new`, re-issue. |
| `STALE_ANCHOR` | `retry` | The file changed under you. `read` it again, take the fresh `contentHash` into `expectedHash` (or re-anchor `old`), re-issue once. |
| `PATTERN_INVALID` | `narrow` | The ast-grep/glob pattern did not parse. Fix it (balanced `$VAR`s, valid glob) and re-issue. |
| `BUDGET_TOO_SMALL` | `narrow` | Raise `budget`, or narrow `path`/`kind` so the answer fits. |
| `COMMAND_NOT_DECLARED` | `abort` | Not in `[commands]`. Tell the human to declare it in `workbench.toml`; never route it through `script` or a shell. |
| `RUN_REDIRECTED` | `abort` | You tried text/file munging or a shell through `run`. Use `search`/`read`/`edit`/`files` for that job. |
| `SANDBOX_FUEL_EXCEEDED` | `narrow` | The script did too much in one round. Scope fewer files (`path`) or split the work across calls. |
| `SANDBOX_CAP_DENIED` | `escalate` | The guest asked for a capability it does not have (network, ambient filesystem). By design; tell the human if the task truly needs it. |
| `SANDBOX_MEMORY_EXCEEDED` | `narrow` | Reduce the working set: process fewer files per call, stream instead of accumulating. |
| `INDEX_UNAVAILABLE` | `retry` | The index is not warm; `notes` carry progress. Use `mode:auto`/`mode:lexical` now, poll `tasks/get` for `index/build`, retry later — not five times in a row. |
| `CONFIRM_REQUIRED` | `confirm` | Review the dry-run diff in the envelope. Re-issue the identical call with `confirm` = `confirmToken` (or accept the elicitation). Do not reach for `autoApply`. |
| `CONFIRM_MISMATCH` | `confirm` | The token is not for this exact call. Redo the dry-run to mint a fresh token; never reuse tokens across calls. |
| `UNSUPPORTED_VERSION` | `abort` | Envelope or on-disk format mismatch. Do not retry; tell the human to upgrade the client/binary. |
| `ENGINE_UNAVAILABLE` | `retry` | The engine child is restarting. Retry after a short pause; lexical search still answers meanwhile. |
| `ENGINE_BUSY` | `retry` | The bounded queue is full. Wait `backoffMs`, then retry. |
| `INPUT_QUARANTINED` | `escalate` | This file repeatedly crashed the parser and is quarantined. Stop parse-verified verbs on it; point the human at `workbench-mcp doctor`. |
| `PLUGIN_CAP_DENIED` | `escalate` | A plugin exceeded its granted capabilities. Not yours to fix; tell the human which plugin. |
| `PLUGIN_CRASHED` | `abort` | The named plugin trapped or ran out of fuel. Do not retry the same plugin call; fall back to the core tools. |
| `CONFIG_POLICY_DENIED` | `abort` | Org policy forbids it; the message names the layer. Tell the human; do not route around it. |

## Worked examples

One per tool. Each block is a complete, valid call.

```json workbench:workbench_status
{}
```

```json workbench:search
{ "query": "parseConfig", "mode": "references", "path": "src/**", "context": 2, "budget": 3000 }
```

```json workbench:map
{ "path": ".", "depth": 2, "detail": "outline", "budget": 6000 }
```

```json workbench:read
{ "file": "src/auth/service.ts", "symbol": "UserService.authenticate", "surround": "enclosing" }
```

```json workbench:edit
{
  "op": "rewrite",
  "pattern": "console.log($ARGS)",
  "rewritePattern": "logger.debug($ARGS)",
  "path": "src/**",
  "lang": "ts",
  "dryRun": true
}
```

```json workbench:files
{ "op": "move", "from": "src/utils/date.ts", "to": "src/lib/date.ts", "dryRun": true }
```

```json workbench:run
{ "command": "test", "args": ["--filter", "auth"], "timeout": 300 }
```

```json workbench:script
{
  "source": "const hits = workspace.search({ query: 'TODO', mode: 'lexical', path: 'src/**' });\nreturn hits.results.map((g) => g.file);",
  "dryRun": true
}
```

```json workbench:undo
{ "steps": 1, "scope": "session", "onConflict": "surface" }
```

## Iterating this skill

The server's flight recorder writes one record per served request to `.workbench/trace.jsonl`
(`name` = `tool:<name>`, `status.code` + `status.recovery` on failures, `durationMs`). Misuse shows
up there as patterns, not opinions: the same `tool:` with the same `status.code` five times in a
row is a retry loop; a run of `RUN_REDIRECTED` means agents are still reaching for the shell
through `run`; `CONFIRM_MISMATCH` churn means the confirm flow above is unclear. When you see one,
revise the row or the recovery line that should have prevented it, by hand, and let `lint:skill`
confirm the tables still agree with the product.
<!-- vendored verbatim from workbench-mcp@acab4d5 — re-run `magistr init` after upgrading workbench-mcp -->
