# GUI iteration screenshots

Every GUI iteration keeps its updated screenshots here before it is committed.
Use `<date>-<pass>/<number>-<area>/`. Keep earlier iteration folders intact.

Each folder contains consistently named PNG views, a JSON receipt per image, and
a review note. Receipts record the actual UI and scenario hashes, base commit,
capture time, appearance, viewport, renderer version, and image hash. The source
hashes identify uncommitted changes more precisely than the base commit alone.

Capture the affected meaningful states with the production renderer. For layout
changes, include dark and light appearances and the supported compact window.
A screenshot is reviewed only after the actual image has been inspected.
Record gaps explicitly: headless WebKit does not establish native input, IME,
OS dialogs, VoiceOver, Rust command integration or Windows parity.

The desktop browser helper `tests/browser/iteration_evidence.mjs` writes a
source-bound capture under `.magistr/desktop-iterations/` when
`MAGISTR_GUI_ITERATION=<date>-<pass>/<number>-<area>` is set. Inspect and copy
that iteration folder here, add a review note, then run final focused checks.
Ordinary test runs leave retained evidence untouched.

The initial `2026-10-15-ia/13-followup-review` bundle rechecks the first thirteen
changes after this rule was requested. It records their combined current source;
it does not claim to capture each earlier commit individually.
