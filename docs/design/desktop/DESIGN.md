---
version: alpha
name: Magistr desktop
description: Product-register design system for the Magistr desktop, built on the brand that ships in apps/website.
colors:
  primary: "#ff9b66"
  neutral: "#141916"
  night-bg: "#141916"
  night-panel: "#1b211d"
  night-raise: "#222a24"
  night-line: "#2c352e"
  night-line-strong: "#374138"
  night-text: "#edf0e7"
  night-text-muted: "#b0b9ae"
  night-text-faint: "#8a9589"
  night-mark: "#a3ada1"
  night-ember: "#ff9b66"
  night-on-ember: "#22160d"
  night-red: "#ff5c7a"
  night-violet: "#b799ff"
  day-bg: "#fafaf7"
  day-panel: "#ffffff"
  day-raise: "#efefea"
  day-line: "#e2e5de"
  day-line-strong: "#d4d8d0"
  day-text: "#202522"
  day-text-muted: "#59615b"
  day-text-faint: "#646c65"
  day-mark: "#5f665f"
  day-ember: "#ad3f0d"
  day-on-ember: "#ffffff"
  day-red: "#c4213d"
  day-violet: "#7a3fc0"
typography:
  page-title:
    fontFamily: IBM Plex Sans
    fontSize: 20px
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: -0.01em
  section-title:
    fontFamily: IBM Plex Sans
    fontSize: 14px
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: IBM Plex Sans
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.45
  body-strong:
    fontFamily: IBM Plex Sans
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1.45
  small:
    fontFamily: IBM Plex Sans
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.4
  data:
    fontFamily: IBM Plex Mono
    fontSize: 12px
    fontWeight: 400
    fontFeature: tnum
rounded:
  row: 4px
  control: 6px
  panel: 12px
spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.night-on-ember}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: 6px 12px
    height: 28px
  button-secondary:
    backgroundColor: "{colors.night-panel}"
    textColor: "{colors.night-text}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: 6px 12px
    height: 28px
  sidebar:
    backgroundColor: "{colors.night-bg}"
  sidebar-item:
    textColor: "{colors.night-text-muted}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: 6px 8px
    height: 28px
  sidebar-item-current:
    backgroundColor: "{colors.night-raise}"
    textColor: "{colors.night-text}"
  task-row:
    backgroundColor: "{colors.night-panel}"
    textColor: "{colors.night-text}"
    typography: "{typography.body}"
    rounded: "{rounded.row}"
    height: 34px
  task-row-selected:
    backgroundColor: "{colors.night-raise}"
  task-status-checked-out:
    textColor: "{colors.night-ember}"
    typography: "{typography.small}"
  task-status-problem:
    textColor: "{colors.night-red}"
    typography: "{typography.small}"
  task-status-person:
    textColor: "{colors.night-violet}"
    typography: "{typography.small}"
  task-status-quiet:
    textColor: "{colors.night-text-faint}"
    typography: "{typography.small}"
  state-mark:
    textColor: "{colors.night-mark}"
    size: 14px
  divider:
    backgroundColor: "{colors.night-line}"
    height: 1px
  panel-edge:
    backgroundColor: "{colors.night-line-strong}"
    height: 1px
  page:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.night-text}"
  button-primary-day:
    backgroundColor: "{colors.day-ember}"
    textColor: "{colors.day-on-ember}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: 6px 12px
    height: 28px
  page-day:
    backgroundColor: "{colors.day-bg}"
    textColor: "{colors.day-text}"
  task-row-day:
    backgroundColor: "{colors.day-panel}"
    textColor: "{colors.day-text}"
    typography: "{typography.body}"
    rounded: "{rounded.row}"
    height: 34px
  task-row-selected-day:
    backgroundColor: "{colors.day-raise}"
  sidebar-item-day:
    textColor: "{colors.day-text-muted}"
    typography: "{typography.body}"
  task-status-quiet-day:
    textColor: "{colors.day-text-faint}"
    typography: "{typography.small}"
  task-status-problem-day:
    textColor: "{colors.day-red}"
    typography: "{typography.small}"
  task-status-person-day:
    textColor: "{colors.day-violet}"
    typography: "{typography.small}"
  state-mark-day:
    textColor: "{colors.day-mark}"
    size: 14px
  divider-day:
    backgroundColor: "{colors.day-line}"
    height: 1px
  panel-edge-day:
    backgroundColor: "{colors.day-line-strong}"
    height: 1px
  page-header:
    textColor: "{colors.night-text}"
    typography: "{typography.page-title}"
    padding: 16px 24px 12px
---

# Magistr desktop

`primary` is ember and `neutral` is the night ground; the `night-` and `day-` tokens give each theme in full, and day components carry a `-day` suffix. The
product context, the register and the 28 journeys are in
[PRODUCT.md](PRODUCT.md).

## Overview

A planning and supervision tool, dense and calm. It should read first as a
well-made tool in its category (Linear, Xcode, Mail). Magistr's brand
contributes four things: IBM Plex, the green-black and sage neutrals from the
website, ember as the single accent, and one signature move, the task row.

The mark is the website's: an open ember M with a short light bar beneath it
(`apps/website/public/favicon.svg`). The desktop app icon uses the same mark.

## Colors

The neutrals are the website's own tokens: green-black and moss at night,
off-white and sage by day. Text tokens are tuned to at least 4.5:1 on every
surface they sit on.

Colour has four jobs and no others:

- **Ember** (night `#ff9b66`, day `#ad3f0d`): Magistr is acting here. Used for
  the primary action, focus, the current selection, every checkout, and the
  mark. It should cover a small share of any screen.
- **Red** (night `#ff5c7a`, day `#c4213d`): something is wrong. Late, failing,
  silent or refused. Always shown with the diamond mark and words.
- **Violet** (night `#b799ff`, day `#7a3fc0`): a person must decide.
- **Neutrals**: everything healthy. State is carried by the mark's shape.

## Typography

IBM Plex Sans for all interface text, with tabular figures for every number,
time and date. IBM Plex Mono only for ids, file paths, machine names, commands
and code. Hierarchy comes from
size and weight. There are five sizes: page title 20, section title 14, body
13, small 12, data 12 mono. No italics for emphasis. No all-caps labels.
Tracking never tighter than -0.01em at these sizes.

## Layout

The shell is a named sidebar (224 px) and a main column.

- **Sidebar.** Waiting on you, Company, then projects. The current project
  opens to show its features. Find and Settings sit at the bottom.
- **Page header.** Every page opens with a title, one line on what it is and
  how it stands, and the trail to it in small text.
- **Content.** The work area for the page. A detail sheet (380–520 px) opens on
  the right for a selected item.

Spacing uses a 4 px base: 4, 8, 12, 16, 24, 32. Related items sit 4–8 px
apart, groups 16–24 px. Every element aligns to the column edges of its region.

## Elevation & Depth

Flat. Surfaces are separated by a 1 px line in the line token, not by shadow.
The only shadows are on overlays (the find field, sheets in a small window):
two layers, a short soft one and a long faint one.

## Shapes

Rows 4 px, controls 6 px, panels and sheets 12 px. Nested radii are never
larger than their parent. No pills except small status counts.

## Components

**Task row (the signature).** One row per task, 34 px high, in three cells
divided by 1 px lines:

1. **State mark** (28 px cell). One drawn shape per state: dashed square for
   ideas, square for to do, half-filled square for in progress, ringed square
   for checked out, diamond for waiting, filled square with a check for done,
   struck square for retired.
2. **Name.** Wraps to at most two lines.
3. **Status.** One short phrase, such as "ready", "11 min left" or "waiting on
   identity, 5 h". It is ember when checked out, red when something is wrong,
   violet when a person is needed, and muted otherwise.

A selected row takes the raised surface and a 1 px ember outline. The agent's
latest action shows as a second line only on the selected row or when the agent
is silent.

Every row in a list shares one set of columns, so the cell dividers line up
from the first row to the last. The first and last rows take the list's corner
radius, so focus rings and outlines follow the curve instead of being clipped.
The ember outline marks the one item open in the sheet; choosing several items
is shown by checkboxes and the raised surface. State marks belong to tasks
only. Merge and pull requests, code hosts and notes use their own icons.

**Sidebar item.** 28 px high, sentence-case label, optional count on the
right. The current item takes the raised surface and full-strength text.

**Buttons.** One primary (ember) per view. Secondary buttons are outlined.
Destructive actions ask for confirmation or offer Undo.

**Focus.** A 2 px ember ring with a 2 px offset on every focusable element,
using `:focus-visible`.

**Icons.** One drawn set of marks and interface icons, 1.5 px stroke at 16 px.
No emoji or text glyphs stand in for icons.

## Do's and Don'ts

Do:

- Start every page with its title and a sentence of state.
- Keep one primary action per view.
- Fold healthy work into a count ("2 features moving fine").
- Show state with a shape and a word, never colour alone.
- Use the product's words from the vocabulary.
- Design every state: empty, loading, error, stale and a small window.
- Respect reduced motion. Animate only to show cause and effect.

Don't:

- Put eyebrow labels above headings, or number sections that are not a
  sequence.
- Put coloured bars thicker than 1 px on the edge of rows, cards or callouts.
- Use monospace for anything but data and code.
- Use gradients, glass, glows, grid backgrounds or decorative textures.
- Use a display face, italics or a single coloured word for emphasis.
- Imitate instruments, terminals or ledgers.
