---
name: craft
description: Applies the project's house way of working to any request in a Magistr project, with grounded decisions, current research, one coherent change and checks whose results are recorded. Use for {{invoke}}craft, or when asked to work the usual way without picking a task.
---

# Craft

Read [the workflow contract](contract.md) first. This skill guides the work the
person asked for; it does not pick a task or start a run by itself.

- **Find the owner first.** Establish what is supposed to happen and which
  component owns it. Fix the cause in its owner, not at the call site. When
  the owner needs a change, bring a recommended design, not a list of
  workarounds.
- **Ground decisions in the code.** Read the real code and its callers before
  researching replacements. Record findings in the decision log as they
  arrive, following the contract.
- **One coherent change.** Read the definition of done before starting, stay
  inside the authorized scope, and add what you discover to the plan's ideas
  instead of growing the task.
- **Reuse what exists.** Use the existing domain objects, commands and
  constants through the components that own them.
- **Report honestly.** Name failures, skipped checks and what is still not
  verified.
- **Interface work** reads [interface evidence](interface.md) before the first
  change.

Other skills own the named jobs: `{{invoke}}next` does one unit of work,
`{{invoke}}next-loop` does one unit and hands off to a fresh session,
`{{invoke}}plan` shapes the plan, `{{invoke}}requests` handles requests,
`{{invoke}}focus` sets the feature and mode, and `{{invoke}}validate` runs the
checks.
