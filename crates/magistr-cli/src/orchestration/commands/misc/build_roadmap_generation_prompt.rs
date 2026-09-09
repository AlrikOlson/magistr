/// Build the prompt that instructs the agent to generate a roadmap.
///
/// The agent authors chunks through the think-and-ship `roadmap_add_chunk`
/// tool (served by the `magistr` MCP server); nothing is written to disk by
/// hand and `ROADMAP.md` stays a generated view (`roadmap_export`).
pub fn build_roadmap_generation_prompt(goal: &str) -> String {
    format!(
        r#"# Task: Generate a Product Roadmap

## Goal
{goal}

## Research Phase (MANDATORY)

Before creating any roadmap items, you MUST research the goal thoroughly:

1. **Use the `WebSearch` tool** (NOT `WebFetch`) to find:
   - Best practices and patterns for implementing this goal
   - Existing libraries, tools, or APIs relevant to this work
   - Common pitfalls and how to avoid them
   - `WebSearch` returns search results; `WebFetch` fetches a single URL. You need search results here.

2. **Analyze the codebase** to understand:
   - Current project structure and tech stack
   - Existing patterns and conventions
   - What already exists that relates to this goal
   - Integration points and constraints

3. **Read the existing roadmap** with `roadmap_status` — extend it, never duplicate a chunk that is already there.

## How to Build the Roadmap

You MUST use the think-and-ship MCP tools to build the roadmap. Do NOT write files directly.

### Create one chunk per coherent slice of work with `roadmap_add_chunk`

Call `roadmap_add_chunk` once per chunk. Parameters:
- `id` (required) — a stable kebab-case slug, e.g. `auth-session-store`
- `title` (required) — the full claim; a sentence is fine
- `name` — the short label a canvas node wears (max 24 characters), e.g. `Session store`
- `group` — the workstream this chunk belongs to (e.g. `Auth`, `API`, `Infra`); pass it at birth
- `priority` — `critical` | `high` | `medium` | `low` (lower sorts earlier)
- `deps` — ids of chunks that must be done first
- `acceptance` — 3–8 concrete, testable acceptance criteria
- `content` — structured body: `{{version: 1, summary: "<one plain sentence>", facts: [{{label, value}}], sections: [{{heading, prose, list: [{{text}}]}}]}}`
- `description` — short plain-prose fallback for the structured content
- `status` — leave at the default (`pending`); use `backlog` for ideas that are not ready

Record the research that shaped the plan with `think_record_step` (one step for the synthesis is enough).

## Roadmap Design Rules

- Order chunks by user impact (most impactful first) and wire `deps` so `roadmap_next` walks them in a buildable order
- Aim for 4–8 chunks, each implementable in one coding session; split anything larger into dependent sub-chunks
- Every chunk names its acceptance criteria — a chunk without acceptance cannot be verified
- Include the test/verification work inside each chunk's acceptance rather than as separate "write tests" chunks
- DO NOT include time estimates
- DO NOT call `roadmap_start_chunk` or `roadmap_complete_chunk` — nothing is started or completed yet
- DO NOT use Write, Edit, or Bash tools — all roadmap mutations go through the `roadmap_*` MCP tools

## After Building

Once every chunk is created, call `roadmap_status` and summarize what you built:
- Number of chunks and the groups they fall into
- Key themes and the dependency chain
- Suggested starting point (`roadmap_next`)
"#
    )
}
