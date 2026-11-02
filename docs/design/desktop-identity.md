# Desktop identity

2 November 2026. The owner blessed edition 10 of the desktop design round. This
document, [DESIGN.md](desktop/DESIGN.md) and [PRODUCT.md](desktop/PRODUCT.md)
are the desktop's design contract. The blessed edition is kept as
[edition-10.html](desktop/edition-10.html) (published at
<https://claude.ai/artifact/JchTCDrybJYN7TBe871bbU>). Where this contract and
[desktop-workspace.md](desktop-workspace.md) disagree, this contract wins.

| File | What it owns |
|---|---|
| [PRODUCT.md](desktop/PRODUCT.md) | Who the desktop serves, the register, the 28 journeys and five principles |
| [DESIGN.md](desktop/DESIGN.md) | Tokens, typefaces, spacing, components and the do's and don'ts, in the Google DESIGN.md format |
| This file | The shell, where each journey happens, and the rules the round settled |
| [edition-10.html](desktop/edition-10.html) | Every screen in night and day: the picture the cutover is compared against |
| [critique-edition-8.md](desktop/critique-edition-8.md) | The critique that shaped editions 9 and 10 |

All copy uses [the vocabulary](../reference/vocabulary.md).

## Brand

The desktop uses the brand that ships in `apps/website`: the open ember M with
its short bar (`apps/website/public/favicon.svg`), the green-black and sage
neutrals, and ember as the only brand accent. The typefaces are IBM Plex Sans
and IBM Plex Mono. The desktop app icon is the website mark; the gradient icon
in `crates/magistr-desktop/ui/icon.png` is retired by the cutover. Lilac,
Manrope and Newsreader are retired.

The desktop is a product-register tool. It first reads as a well-made tool in
its category; the brand supplies the typeface, the palette, the density and one
signature element, the task row. Nothing imitates instruments, terminals or
ledgers.

## Shell

One shell for every screen:

- **Sidebar of places** (224 px). The organisation's name with the mark,
  Waiting on you with its count, Company, then each project with its tasks done.
  The open project shows its features, each with its state mark. Add a project,
  then Find (⌘K) and Settings at the foot, and below them how fresh the view is
  ("Up to date, 8 s ago" or "Owner offline since 10:42"). The sidebar holds
  places only; it scrolls when the window is short.
- **Page header.** The trail to the page in small text, the title, and one line
  saying what the page is and how it stands. Actions sit on the right, with one
  primary action at most.
- **Tab row.** A place's views are tabs under its title: Company has Projects,
  Teams and Directory; a project has Features, Requests, Decisions and Code; a
  feature has Tasks, Plan and Decisions; Settings has You, Interruptions, Code
  hosts and Directory.
- **Sheet.** Selecting a task opens a sheet on the right (380–520 px) with the
  task's own tabs: Detail, Run, Packet, Review and Agent. Its actions sit at its
  foot.
- **Overlays.** Find and pickers open as dialogs over a dimmed page.
  Notices with Undo appear at the foot of the column that caused them.

These are the only ways to move. There are no header lenses, no zoomable start
surface and no breadcrumb-first header.

## Where each journey happens

| Journeys | Screen |
|---|---|
| J1–J3 | Waiting on you: only what needs you, each with its next action; the digest folded; healthy work as one line |
| J4 | The feature's Plan tab in its planning state: proposed tasks with responsible people, limits, why this plan; Approve, Ask for changes…, Refuse… |
| J5 | A question as its own page, with the agent's options written out and Send answer |
| J6 | The task sheet: Nudge as the primary action, Release… confirmed inside the sheet, Undo on the nudge |
| J7 | The Packet tab: checked by machines and approved by a person kept apart, the unverified item named |
| J8 | Review: files ordered by risk with the reason, the agent's note beside the changed line |
| J9, J15 | The feature's Tasks tab: Start on every ready row, Start N ready tasks in the header, limit, budget and due date above the list |
| J10, J11 | The Agent and Run tabs; Pause is primary, Stop… asks first |
| J12, J14 | The Detail tab: responsible person, who works on it, dependencies, code, decisions and the definition of done; the dependency dialog |
| J13 | The project's Requests tab: choose requests, then Make a feature from N… |
| J16 | The project's Decisions tab |
| J17 | The Detail tab of a waiting task: the wait chain down to the real hold-up and Ask the other project |
| J18 | Company › Teams: idle agents with the reason |
| J19 | A banner on the feature naming what was reopened and what to re-check |
| J20 | The task's history at the foot of its Detail tab |
| J21 | First run; Company › Projects; the project's Code tab |
| J22–J24 | Settings › Code hosts, Directory and Interruptions |
| J25 | Find: tasks, decisions and actions, with its keys shown |
| J26–J28 | Offline owner, silent agent, expired checkout, error, empty, loading and the small window |

## Rules the round settled

- **Colour has four jobs.** Ember: Magistr acting (the primary action, focus,
  the open item, checkouts, the mark). Red: something is wrong, always with the
  diamond mark and words. Violet: a person must decide. Neutrals: everything
  healthy.
- **State marks are for tasks only.** Idea, to do, in progress, checked out,
  waiting, something is wrong, done and retired each have one drawn shape.
  Requests, merges, code hosts and notes use their own icons.
- **The task row.** Mark, name, status in three cells. Every row in a list
  shares one set of columns, so dividers line up. The first and last rows take
  the list's radius, so focus rings and outlines are never clipped.
- **Selection.** The raised surface plus a 1 px ember outline marks the one item
  open in the sheet. Choosing several items is shown by checkboxes and the raised
  surface. No coloured bars on edges.
- **Type.** Plex Sans with tabular figures for every number, time and date.
  Plex Mono only for ids, paths, machine names, commands and code.
- **Actions.** Every action works without dragging. Release, Refuse and Stop ask
  first; safe actions offer Undo. Actions that ask for more end in "…".
- **States.** Every list has its empty, loading, error, stale and small-window
  forms. Loading is words, not placeholder shapes.
- **No instructions inside screens.** Screens show the product, not notes about
  how to read it.

## What the cutover must prove

The desktop identity cutover is accepted against this contract: tokens generated from DESIGN.md, one state component, the Web
Interface Guidelines audit, contrast and focus in both themes, and screenshots
of every view compared with edition 10.
