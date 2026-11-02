---
name: focus
description: Sets or reports which Magistr feature this session works in and in which mode (shape, build or listen); it never does the work itself. Use for {{invoke}}focus, "switch to <feature>", "work on billing in build mode" or "what am I focused on".
---

# Focus

Read [the workflow contract](../craft/contract.md). Focus is selection only: it
writes no code, starts or finishes no task, processes no request and reorders
nothing. `{{invoke}}next` does one unit inside the focus.

| Mode | What `{{invoke}}next` does there |
|---|---|
| shape | Makes one plan decision or does one research step |
| build | Builds one ready task behind its checks |
| listen | Processes one request |

Focus is saved per team. Pass `team` as a stable identity for this caller: the
team the person named, otherwise the project's person (`MAGISTR_USER`) joined
with the host name, such as `sam-claude`. Use the same value every time.

- **No arguments:** read `plan_focus_get`, report the feature, mode and its
  frontier (ready, in progress, waiting, open requests), and change nothing.
- **A feature, and optionally a mode:** match the name against the plan's
  features. If it is ambiguous, list the candidates and stop. Keep the current
  mode when none is given; default to build when there was none. Save it with
  `plan_focus_set`, then report the new focus and its frontier.
- **"Clear focus":** `plan_focus_set` with `clear: true`.

Changing focus leaves open runs alone: they are saved and resume when their
task comes back into focus.
