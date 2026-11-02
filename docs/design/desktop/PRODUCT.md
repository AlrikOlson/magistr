# Magistr desktop: product context

Status: part of the desktop's design contract since edition 10 was blessed on
2 November 2026, with [DESIGN.md](DESIGN.md) and
[desktop identity](../desktop-identity.md).

## What the desktop is

The Magistr desktop is where people direct and check the work that coding
agents do across a company's projects. Agents take tasks from a shared plan,
work on them under checkouts, and finish them with checked results. People
approve plans, answer agents' questions, chase work that is stuck, and decide
whether a finished task can be relied on.

## Register

**Product (operate).** The design serves the work. It is not the product.

- The desktop must first read as a well-made tool in its category, which is
  planning and supervision tools such as Linear, GitHub, Xcode and Mail.
  Layout, navigation and controls follow those conventions and the platform's.
- Magistr's brand supplies four things only: the typeface, the palette, the
  density, and one signature move (see [DESIGN.md](DESIGN.md)).
- No costume. The desktop does not imitate instruments, terminals, ledgers or
  air-traffic equipment. Ideas taken from those worlds are kept only where they
  do a job, such as a task row with fixed cells.

## Who uses it

| Person | What they come to do |
|---|---|
| Responsible person or approver | Approve plans, answer agents, release stalled holds, decide on done packets |
| Engineer | Start tasks, talk to the agent on a task, review changes, pause or stop runs |
| Product manager | Turn requests into features, follow a task from request to done |
| Leader | See whether the company's work is moving and what is stuck |
| Platform engineer | Connect code hosts and the company directory, keep owners online |
| Solo owner | All of the above, for one or a few projects |

## The 28 journeys

Every view must serve at least one of these. The screen for each journey is
listed in [desktop identity](../desktop-identity.md#where-each-journey-happens)
and drawn in [edition 10](edition-10.html).

- **Triage.** J1 see what needs me and act. J2 clear the digest. J3 glance at
  health and leave.
- **Decide.** J4 approve, amend or refuse a plan. J5 answer an agent. J6 nudge
  or release a stalled hold. J7 settle a done packet's gap. J8 review a risky
  change.
- **Steer.** J9 start a ready task. J10 talk to the agent on a task. J11
  pause, step or stop a run. J12 hand a task to a person or change who is
  responsible.
- **Shape.** J13 turn requests into a feature. J14 add or edit a task, its
  definition of done and dependencies. J15 reorder work, set a feature's limit
  and budget. J16 read the decisions behind a task.
- **Chase.** J17 find why something is late and ask the other team. J18 find
  why agents are idle. J19 react to a reopened dependency. J20 follow a task
  from request to done.
- **Set up.** J21 add or open a project and its code. J22 connect a code host.
  J23 connect the company directory. J24 set interruptions and when a wait is
  late. J25 find anything.
- **Recover.** J26 a stale view or an offline owner. J27 a silent agent or an
  expired checkout. J28 a small window.

## Principles

1. **Every screen says where you are.** A named sidebar of places, and a page
   title with one line saying what the page is and how it stands.
2. **One question, one primary action.** Each view answers one question and
   offers one primary action.
3. **Quiet when healthy.** Healthy work is drawn in neutrals. Colour marks only
   Magistr acting (ember), something wrong (red), or a person needed (violet).
4. **Proof before trust.** A finished task shows its proof and what is not
   verified. Checked by machines and approved by a person are separate.
5. **People stay responsible.** Every task in an approved feature names a
   responsible person beside the agent that holds it.

## Words

All copy uses [the vocabulary](../../reference/vocabulary.md).
