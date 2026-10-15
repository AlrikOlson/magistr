# GUI iteration screenshots

Every GUI iteration captures its updated screenshots directly here, before review.
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
source-bound capture directly under `docs/evidence/desktop-iterations/` when
`MAGISTR_GUI_ITERATION=<date>-<pass>/<number>-<area>` is set. It creates a pending
review note and refuses to overwrite an existing PNG or receipt. Use a new
numbered folder for a recapture. Open and review the images at these canonical
paths; never rely on a later copy from an ignored scratch directory. Record each
image’s verdict here, then run final focused checks. Ordinary test runs leave
retained evidence untouched.

The [restart capture index](2026-10-15-restart/README.md) distinguishes rejected,
inspected but unaccepted, and unreviewed captures. Historical captures retain
their original source receipts; relocation does not refresh or approve them.

The initial `2026-10-15-ia/13-followup-review` bundle rechecks the first thirteen
changes after this rule was requested. It records their combined current source;
it does not claim to capture each earlier commit individually.
## Visual review gate

The capture step and the visual acceptance step are separate. Before completion:

1. Open every changed state at full frame and inspect its text and controls at
   readable detail. Review the actual default view, not just a specially adjusted
   camera. Cover desktop and compact sizes, light and dark, selected states,
   realistic dense data, and empty or missing data where relevant.
2. Write a row per image: filename, user task, concrete composition and detail
   observations, verdict, and any defect. Record exactly which transition or
   interaction was exercised separately from what the still image shows.
3. For connected views, trace each relationship from source to destination.
   Check attachment points, direction, separation at forks and returns, label
   clearance, node clearance, and whether shared prerequisites remain legible.
   An unexplained line or a visually misleading arrangement fails review.
4. Block completion on clipping, overlap, unreadable default scale, excessive
   empty geometry, awkward grouping, confusing controls or ambiguous connections.
   Record a required `ship_check(name: "GUI visual review", type: "review",
   passed: false)` while any defect remains. Passing interaction tests does not
   override a failed visual review.
5. Fix the owning layout or component, capture fresh images, inspect them, and
   record each defect's resolution against the new filename. Preserve the
   rejected capture and its verdict. Recheck adjacent states affected by the fix.
6. Only then record a passing visual review, link the inspected evidence, and
   complete or commit the iteration. Do not batch future visual iterations on
   top of a known failed review. State remaining native or assistive technology
   gaps honestly.

A review must explain why the user can complete the task comfortably. “Looks
good”, “screenshots inspected”, or a list of test results is not a critique.
The reviewer must not generate a passing review from capture metadata. Hash
checks establish integrity; they cannot establish taste, clarity or actual
inspection. The tool history and concrete per-image findings make the review
auditable without pretending subjective quality can be proved automatically.