---
name: plan
description: Reports and shapes a Magistr project's plan without building anything - status, research-driven refresh of an area, adding or reshaping tasks and features, and priority proposals. Use for {{invoke}}plan, "where are we", "the plan feels stale", "revisit priorities" or "add this to the plan".
---

# Plan

Read [the workflow contract](../craft/contract.md). This skill changes the plan,
never the code: building a task is always `{{invoke}}next`. It never opens a
run and never disturbs one that is open.

Pick the mode from the request.

## Status

Read `plan_status` and the `plan_get` records that matter, report them, and
stop. If the plan is empty, say so; create one only when the request asks for
it.

## Shape

Make the plan changes the person asked for: `plan_add_task` with a definition
of done, `plan_update_task`, `plan_set_feature`, dependencies and links. New
work goes to ideas unless the person chose another state. A plan decision in
`{{invoke}}next` shape mode is one of these changes plus its note.

## Refresh

One focused research pass over the person's topic, or the one area most likely
to be stale.

1. Read the relevant tasks and earlier notes. Find the real entry points,
   dependencies and current behavior in the code. Record the question with
   `decision_record`.
2. Research current primary sources for APIs, versions, alternatives and
   limits. Compare them with the code and each definition of done. Record
   findings as they arrive, including changed conclusions and refuted ideas.
3. Correct descriptions, definitions of done and dependencies with
   `plan_update_task`; link evidence with `plan_link`. Add discoveries as
   ideas. Retire a task only with evidence and inside the requested scope.
4. Close with a summary note and `plan_record_refresh`. Report what changed,
   the open proposals and the next decision for the person.

## Priorities and states

`plan_reprioritize` records a proposal; it changes nothing. Apply priority or
state changes only when the person authorized them; otherwise present the
proposal. A smaller priority number is more urgent.

Follow the contract's `plan_exports` rule. Generated plan files are views,
never the source of truth.
