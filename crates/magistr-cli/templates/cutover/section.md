<!-- magistr -->
# Magistr

Magistr keeps this project's plan, runs, checks, decision log and requests. The
`magistr` MCP connection serves them and the source tools. Start with
`context`; the connection's own instructions explain the tools.

## Always

- Work from the plan. Resume unfinished work before starting new work; never
  reset or finish someone else's run to make room.
- Open a run and its steps before editing source. Record findings in the
  decision log as they arrive.
- Pass the checks through `check_step` or `magistr_check` before committing.
  The server runs them; a result you typed yourself is not a check.
- Finish the task in the plan, then commit it in one conventional commit.
  Preserve unrelated changes.
- A manual fix, review or discussion does not authorize starting a task.

The full contract is in the `craft` skill's `contract.md`; each skill below
links it.

## Skills

| Skill | Job |
|---|---|
| `{{invoke}}next` | Do one unit of work in the current focus |
| `{{invoke}}next-loop` | Do one unit, then hand off to a fresh session |
| `{{invoke}}plan` | Report or shape the plan without building |
| `{{invoke}}requests` | Capture, research or promote requests |
| `{{invoke}}focus` | Set or report the feature and mode |
| `{{invoke}}validate` | Run every configured check |
| `{{invoke}}init` | Tailor the project setup |
| `{{invoke}}craft` | Work the house way on any request |
{{verify_block}}<!-- /magistr -->
