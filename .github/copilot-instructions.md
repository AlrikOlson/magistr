# magistr

**Stack:** rust

## Quick Start

```bash
cargo check --workspace --all-targets && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

## Workflow

# Magistr workflow

The maintained workflow lives in the generated project skills. Read
`.claude/skills/craft/SKILL.md` and its
`.claude/skills/craft/references/runtime.md` contract before substantial work.
Those files define shared state, tool routing, roadmap export policy, recorded
verification, recovery, and commit scope. The runtime includes the Claude host
adapter; other hosts use their own generated adapter.

Use the skill matching the user’s request:

- `/next` or `/roadmap`: resume an authorized in-progress chunk first, otherwise
  select one ready chunk, through the native research, plan, execute, verify,
  completion, and commit lifecycle. Follow next's recovery contract: inspect
  `roadmap_status` and `ship_status`, keep a matching unfinished objective and
  plan, and never restart completed tasks. Select the authorized scope with
  `ship_set_objective` if another scope is selected; saved objectives and plans
  are restored unchanged. Preserve unrelated unfinished work without resetting it.
- `/next-loop`: bounded fresh-session handoff; one complete chunk per
  agent session, then a new session before the next chunk. The native CLI
  loop owns automatic chaining.
- `/validate`: configured gates with accurate results and recording status.
- `/roadmap-refresh` or `/signals`: research and native state updates within
  the requested scope, without implementing the resulting features.
- `/init`: tailor project configuration without starting implementation.

A manual fix, review, configuration task, or planning discussion does not
itself authorize starting a roadmap chunk or an implementation loop. Preserve
unrelated user changes and active work. Store iteration narrative in native
reasoning and the local receipt, not always-loaded project instructions.

Keep native phase indicators current during an implementation iteration with
`magistr_set_phase`: `research`, `plan`, `execute`, `verify`, then `done` only
after the commit exists. Claude guard hooks remain additional enforcement;
follow the skills’ explicit recorded gates and lifecycle verification even
when the host does not run those hooks.


## Conventions

# Rust Conventions

## Toolchain

- Edition 2024, stable toolchain.
- `cargo check --all-targets` before commit. `cargo clippy -- -D warnings` treats all warnings as errors.
- `cargo fmt` enforced — no style debates.

## Error Handling

- No `unwrap()` in library code. Use `?` for propagation, `expect("context")` only when invariants are truly guaranteed.
- `thiserror` for library error types, `color-eyre` / `anyhow` for application-level errors.
- Return `Result<T, E>` from fallible functions. Never use `panic!` for recoverable errors.
- Wrap errors with context: `.map_err(|e| ...)` or `.context("what failed")`.

## Naming

- `snake_case` for modules, functions, variables, and methods.
- `PascalCase` for types, enums, traits, and type aliases.
- `SCREAMING_SNAKE_CASE` for constants and statics.
- Prefix boolean functions/methods with `is_`, `has_`, `can_`, `should_`.

## Patterns

- Builder pattern for structs with many optional fields.
- Newtype wrappers (`struct UserId(u64)`) for domain types that deserve type safety.
- Accept `impl Into<T>` or `AsRef<T>` for flexible API boundaries.
- Mark return values with `#[must_use]` when ignoring them is almost certainly a bug.
- Prefer iterators and combinators over manual loops. Chain `.map()`, `.filter()`, `.collect()`.
- Use `enum` with variants over boolean flags or stringly-typed dispatch.

## Async

- Use `tokio` as the async runtime. Annotate entry points with `#[tokio::main]` or `#[tokio::test]`.
- Prefer `tokio::select!` for racing futures. Always include a cancellation branch.
- Use `tokio::spawn` for background work. Hold the `JoinHandle` and await it before shutdown.
- Never block the async runtime with synchronous I/O — use `tokio::task::spawn_blocking` instead.
- Prefer `tokio::sync::mpsc` for channels, `tokio::sync::Mutex` only when shared state is unavoidable.
- Cancel-safety: document whether async functions are safe to drop mid-`.await`.
- Use `async fn` in traits via `async-trait` crate or native async traits (Rust 1.75+).

## Traits

- Keep traits focused: one responsibility per trait (Interface Segregation).
- Provide blanket impls where sensible: `impl<T: Read> MyTrait for T`.
- Use associated types for 1:1 relationships, generics for 1:N: `type Output` vs `fn process<T>()`.
- Derive standard traits in this order: `Debug, Clone, Copy, PartialEq, Eq, Hash, Default`.
- Prefer `impl Trait` in return position over `Box<dyn Trait>` when the concrete type is fixed.
- Seal traits that shouldn't be implemented outside the crate using a private super-trait.

## Lifetimes

- Elide lifetimes whenever the compiler allows it — only annotate when required.
- Prefer owned types in public APIs. Use borrows (`&T`, `&str`) in internal hot paths.
- Use `Cow<'_, str>` when a function sometimes allocates and sometimes borrows.
- Avoid lifetime parameters on structs unless the struct genuinely borrows data it doesn't own.

## Testing

- Inline unit tests in `#[cfg(test)] mod tests {}` at the bottom of each module.
- Integration tests in `tests/` directory.
- Use `assert_eq!`, `assert_ne!`, `assert!(cond, "message")`. Prefer `assert_matches!` for enum variants.
- Test error paths, not just happy paths.
- Use `tempfile` for filesystem tests — never write to fixed paths.
- Use `#[tokio::test]` for async tests. Prefer `tokio::time::pause()` over real sleeps.
- Use `insta` for snapshot testing complex output (serialized structs, rendered UI, error messages).
- Prefer `proptest` or `quickcheck` for property-based testing of pure functions.

## Workspace

- Use Cargo workspaces for multi-crate projects. Shared deps go in workspace `Cargo.toml`.
- Feature flags: use `default = []` and let consumers opt in. Document each feature.
- Use `#[cfg(feature = "...")]` for conditional compilation. Keep feature-gated code minimal.

## Anti-Patterns

- No `clone()` to silence the borrow checker — fix the ownership model instead.
- No `Box<dyn Any>` — use concrete types or proper trait objects.
- No `unsafe` without a `// SAFETY:` comment justifying the invariant.
- No wildcard imports (`use foo::*`) except in test modules and preludes.
- No raw `println!` — use structured output, logging, or the TUI renderer.
- No `#[allow(clippy::...)]` without a comment explaining why the lint is wrong here.

## Dependencies

- Minimize dependency count. Prefer std-lib solutions when reasonable.
- Pin major versions in `Cargo.toml`. Use `cargo update` deliberately, not blindly.
- Audit new deps: check maintenance status, transitive dependency count, and unsafe usage.


<!-- Auto-generated by magistr — DO NOT EDIT. Re-run `magistr init` to update. -->
# Tool Guide

## Codebase Navigation & Editing (workbench-mcp)

| Tool | Replaces | Purpose |
|------|----------|---------|
| `search` | grep/rg/find/ls/glob | Ranked hits grouped by file; `mode:` lexical (default), structural (ast-grep), references, files. |
| `map` | tree/ls -R | Pruned tree with entry points, declared commands, per-file symbol outlines. |
| `read` | cat/head/tail/sed -n | A file, a line range, or a symbol by name — with line anchors and a content hash. |
| `edit` | sed -i/codemods | Parse-verified diff: replace body, insert around symbol, rename, structural rewrite, anchored replace. |
| `files` | mv/cp/rm/mkdir | Import-aware moves and journaled deletes; undoable. |
| `write` | cat > file/heredocs | A whole file as exact bytes: creates it with content or replaces it; journaled, undoable. |
| `run` | running tests/build/lint | One declared command from `workbench.toml [commands]`, failures as rows. |
| `script` | bash one-liners over many files | Typed Code Mode: reads live, writes queued into one reviewable diff. |
| `undo` | git checkout -- (own edits) | Rewind the journal one or more steps. |

Every call returns one envelope (`ok`, `results`, `freshness`, `budget`, `truncation`, `notes`). Read `.claude/skills/workbench-routing/SKILL.md` for the routing table and error recovery. Git stays in the shell.

Recommended workflow: `map` → `search` → `read` → `edit` (parse-verified) → `run` for the declared gates; `search mode:references` before touching shared code.

## Roadmap, Reasoning & Execution (think-and-ship)

| Family | Purpose | Key tools |
|--------|---------|-----------|
| `roadmap_*` | The long-horizon plan-of-plans: chunks with deps, acceptance and status. | `roadmap_status`, `roadmap_next`, `roadmap_get`, `roadmap_start_chunk`, `roadmap_complete_chunk`, `roadmap_add_chunk`, `roadmap_update_chunk`, `roadmap_export` |
| `ship_*` | The execution trace: objective, plan, actions, quality gates. | `ship_set_objective`, `ship_plan`, `ship_start`, `ship_record`, `ship_complete`, `ship_check`, `ship_finalize`, `ship_status` |
| `think_*` | The reasoning trace: record findings throughout the work, explore questions, and link conclusions to evidence. | `think_record_step`, `think_pin_step`, `think_trace_checkpoint`, `think_search_trace` |
| `signal_*` | Stakeholder signals: capture, research, surface, promote to the roadmap. | `signal_capture`, `signal_pending`, `signal_research`, `signal_promote` |

Served by the same `magistr` MCP server. A roadmap chunk is realized by a ship objective, motivated by think reasoning; the plan is `ship_plan` (there is no plan file) and `ROADMAP.md` is a generated view (`roadmap_export`), never the source of truth.

## Record findings while you work

The user should be able to follow what was learned, which alternatives were
checked, and why the work changed direction. Use `think_record_step` throughout
substantive work, not just to open and close an iteration. Record concise,
user-readable summaries of evidence and conclusions; do not write private
internal deliberation, tool transcripts, or an invented account after the work.

- Record a finding when research answers a consequential question, an approach
  is chosen over a credible alternative, a test changes the diagnosis, or a
  constraint changes the plan. State the question, evidence (source, code or
  check), conclusion, and next action. An execution log is not a substitute.
- For a concrete alternative, uncertain assumption, or diagnostic question that
  needs its own investigation, start a branch with `branch_from` set to the
  actual originating step and `branch_name` phrased as the question being
  explored. Omit `step_number` to allocate it. Read the returned `branch.id`.
- Continue that investigation with its exact `branch_id` on every follow-up
  finding. Do not accidentally append those findings to the main line or start
  a new branch for each observation. Parallel or nested investigations each
  keep their own question and recorded origin.
- When findings are used, record a main-line synthesis with `dependencies`
  pointing to the supporting branch steps. Then call `think_set_branch_status`
  with `status: "merged"` and `merged_into` set to that synthesis step.
  A rejected idea can still produce useful findings: mark the investigation
  merged when its findings informed the result. Use `abandoned` only for an
  investigation stopped without using its findings, and record why.
- Correct an earlier conclusion with `revises_step` and `revision_reason`;
  preserve the earlier record. Link related execution through `execution_ref`
  and `ship_record.think_step`. Use meaningful names in prose rather than raw
  identifiers. Omit confidence unless there is a defensible basis for it.
- Before closing, review investigations opened for this work. Record their
  conclusions and actual use, or leave them active with an explicit unresolved
  question and next action. Do not mark unfinished investigations merged.

Use branches for real investigations, not to satisfy a quota or decorate a
graph. Straightforward work can remain linear. Never invent alternatives,
evidence, outcomes, relationships, or confidence. Keep records brief enough to
read while the work is happening.


## Quality & Workflow (magistr)

| Tool | Purpose |
|------|---------|
| `magistr_gate` | Run quality gates (`all` / `check` / `test` / `lint` + custom); each gate is recorded through `ship_check`. |
| `magistr_format` | Run the project formatter. |
| `magistr_set_phase` | Announce workflow phase transitions. |
| `magistr_delete` | Delete files/directories (use instead of `rm`). |
| `magistr_list` | List directory contents. |

## Tool Preferences

- Use workbench `search`/`map` instead of Glob/find for file discovery.
- Use workbench `search` (lexical/structural/references) instead of Grep/grep.
- Use workbench `read` for exploration and `edit` for verified writes; built-in `Read`/`Edit` only when workbench is unavailable.
- Use workbench `run` for declared commands; `magistr_gate` runs the full gate set.
- Use the `roadmap_*` tools instead of editing `ROADMAP.md` — it is a generated view (`roadmap_export`).
- Record every gate through `ship_check` (`magistr_gate` forwards to it) — never a piped or self-reported result.
- Use workbench `write` to create a file with content or replace a whole file; `files` `op:create` only makes an empty one.
- Pass `autoApply: true` on a multi-file workbench write you intend to make (`rename`, `rewrite`, `move`, `script`) — a confirm-gated write left unanswered is a dangling CONFIRM_REQUIRED, and under `magistr next-loop` there is no human to answer one. Single-file `edit` and `write` calls apply on the first call in token confirm mode.
- Use `magistr_delete` instead of `rm` in Bash.
- Do not spawn sub-agents — work directly in the current session.