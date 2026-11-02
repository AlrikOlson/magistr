---
name: requests
description: Captures, researches, raises and promotes requests people made to a Magistr project (questions, suggestions, concerns, bugs, feedback), recording evidence without building anything. Use for {{invoke}}requests, "what have people asked for", "triage the feedback" or "log this request".
---

# Requests

Read [the workflow contract](../craft/contract.md). This skill never builds the
thing asked for and never messages the person who raised it.

Pick the mode from the request:

- **Status:** `request_status`, report, change nothing.
- **Capture:** `request_capture` with the source and meaning kept intact, then
  stop.
- **Raise:** read `request_ready` and the plan around it, present the most
  actionable request, and `request_raise` it. Respect snoozes and earlier
  raises.
- **Work one** (the default, and `{{invoke}}next` listen mode): process one
  open request end to end, then stop.

## Work one request

1. Read `request_status` and the chosen `request_get` record. Check what it says
   against the current code before researching its premise.
2. Record reasoning with `decision_record` and durable findings, sources and
   justified confidence with `request_research`. Link notes and tasks with
   `request_link`.
3. Decide:
   - **Refuted or not actionable:** record why, then `request_dismiss` or
     `request_snooze`.
   - **Actionable:** `request_promote` it to the plan's ideas when the person
     authorized promotion; otherwise raise it with a concrete recommendation.
     Promotion never starts building.
4. Report the request, the evidence, the recorded change and the next
   candidate. Do not repeat unchanged feedback that is not actionable.
