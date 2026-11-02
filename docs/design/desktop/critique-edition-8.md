# Critique of desktop edition 8

2 November 2026. Edition 8:
<https://claude.ai/artifact/RnrjnxRjDhtYnXi1AgGFkb>

**Method.** This critique follows the Impeccable critique procedure (Nielsen
heuristics, a specificity verdict, priority issues and persona red flags). It
ran in degraded mode: one reviewer and no deterministic detector, because this
project runs no sub-agents and the detector was not installed. The references
are the product (operate) register and the craft floor from Impeccable, and
[DESIGN.md](DESIGN.md).

## Specificity verdict

**Partly specific.** The ember M, the green-black neutrals and the three-cell
task row are recognisably Magistr. The frame around them reads as a generic
sidebar app. It also carries two navigation systems in its header, and uses
edge bars and text glyphs that the floor rules out.

## Heuristics

| # | Heuristic | Score (0–4) | Note |
|---|---|---|---|
| 1 | Visibility of system status | 3 | Rows show state, time left and waits. The shell does not show connection or freshness. |
| 2 | Match with the real world | 3 | Uses the vocabulary. Marks need learning, and there is no legend outside the catalog. |
| 3 | User control and freedom | 2 | No Undo. Release, Refuse and Stop act without confirmation. |
| 4 | Consistency and standards | 2 | Lenses in the header, tabs in the sheet and the sidebar are three navigation models. Selection is drawn three different ways. |
| 5 | Error prevention | 2 | Starting a task depends on dragging, with no preview of what will happen. |
| 6 | Recognition rather than recall | 3 | The sidebar and titles help. The lenses hide where Requests, Decisions and Code live. |
| 7 | Flexibility and efficiency | 3 | ⌘K exists. No keyboard shortcuts are shown and there is no multi-select. |
| 8 | Aesthetic and minimalist design | 3 | Calm. A few leftovers: edge bars and glyph icons. |
| 9 | Recognise, diagnose, recover from errors | 3 | Banners name the next action. Error rows do not always offer one. |
| 10 | Help and documentation | 1 | No first-run view and no inline help for marks or checkouts. |
| | **Total** | **25 / 40** | |

## Strengths

- Every page says where you are and how it stands.
- State is carried by shape and words, with colour kept for exceptions.
- One primary action per view is mostly held.

## Priority issues

1. **P0. Edge bars break the floor.** The current sidebar item has a 2 px ember
   bar, and selected rows and items have 3 px bars. **Fix:** show the current
   item and the selection with the raised surface, full-strength text and a 1 px
   ember outline.
2. **P1. Three navigation models.** Header lenses, sheet tabs and the sidebar
   compete. **Fix:** a place's views become one standard tab row under the page
   title. The sidebar holds places only, and the sheet keeps its own tabs.
3. **P1. Faint text fails contrast.** Day faint is 3.2:1 and night faint on
   raised panels is 4.1:1. **Fix:** the [DESIGN.md](DESIGN.md) tokens (5.2:1 and
   4.7:1 or better).
4. **P1. Text glyphs stand in for icons.** ▸, ▾, › and ◆ are used as icons.
   **Fix:** one drawn icon set with a 1.5 px stroke.
5. **P2. Drag-only and unconfirmed actions.** Starting work needs a drag, and
   Release, Refuse and Stop act at once. **Fix:** a Start button on ready rows
   and on the team line, inline confirmation for releases and refusals, and
   Undo where it is safe.
6. **P2. No first run or help.** **Fix:** a first-run page that explains the
   desktop in one sentence and offers Open a folder, plus an inline explanation
   of marks and checkouts on first sight.
7. **P2. No system status in the shell.** **Fix:** connection and freshness at
   the bottom of the sidebar.
8. **P3. The catalog page uses eyebrow labels.** **Fix:** plain headings.

## Persona red flags

- **First-time user.** Opens to rows of shapes with no explanation of
  checkouts, holds or marks. Cannot tell how to start work without dragging.
- **Power user.** No visible shortcuts, no multi-select for approving or
  starting several tasks, and lenses take two clicks to reach.
- **Keyboard and screen-reader user.** Drag-only start, marks without stated
  names in the catalog, and no visible focus style shown in any specimen.

## Questions

Skipped. The owner asked for edition 9 to be built directly from this critique.
