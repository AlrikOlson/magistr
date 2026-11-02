/// Build the prompt that instructs the agent to generate a plan.
///
/// The agent authors tasks through the magistr `plan_add_task`
/// tool (served by the `magistr` MCP server); nothing is written to disk by
/// hand and `PLAN.md` stays a generated view (`plan_export`).
pub fn build_plan_generation_prompt(goal: &str) -> String {
    format!(
        r#"# Task: Generate a Product Plan

## Goal
{goal}

## Research Phase (MANDATORY)

Before creating any plan items, you MUST research the goal thoroughly:

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

3. **Read the existing plan** with `plan_status` — extend it, never duplicate a task that is already there.

## How to Build the Plan

You MUST use the magistr MCP tools to build the plan. Do NOT write files directly.

### Create one task per coherent slice of work with `plan_add_task`

Call `plan_add_task` once per task. Parameters:
- `id` (required) — a stable kebab-case slug, e.g. `auth-session-store`
- `title` (required) — the full claim; a sentence is fine
- `name` — the short label a canvas node wears (max 24 characters), e.g. `Session store`
- `feature` — the feature this task belongs to (e.g. `Auth`, `API`, `Infra`); pass it at birth
- `priority` — `critical` | `high` | `medium` | `low` (lower sorts earlier)
- `deps` — ids of tasks that must be done first
- `acceptance` — 3–8 concrete, testable acceptance criteria
- `content` — structured body: `{{version: 1, summary: "<one plain sentence>", facts: [{{label, value}}], sections: [{{heading, prose, list: [{{text}}]}}]}}`
- `description` — short plain-prose fallback for the structured content
- `status` — leave at the default (`to_do`); use `idea` for ideas that are not ready

Record the research that shaped the plan with `decision_record` (one step for the synthesis is enough).

## Plan Design Rules

- Order tasks by user impact (most impactful first) and wire `deps` so `plan_next` walks them in a buildable order
- Aim for 4–8 tasks, each implementable in one coding session; split anything larger into dependent subtasks
- Every task names its acceptance criteria — a task without acceptance cannot be verified
- Include the test/verification work inside each task's acceptance rather than as separate "write tests" tasks
- DO NOT include time estimates
- DO NOT call `plan_start_task` or `plan_complete_task` — nothing is started or completed yet
- DO NOT use Write, Edit, or Bash tools — all plan mutations go through the `plan_*` MCP tools

## After Building

Once every task is created, call `plan_status` and summarize what you built:
- Number of tasks and the features they fall into
- Key themes and the dependency chain
- Suggested starting point (`plan_next`)
"#
    )
}
