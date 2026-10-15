# Background desktop layout evidence

These captures render the production desktop HTML, CSS and JavaScript in
headless WebKit with realistic, isolated observation fixtures. They do not
open desktop windows or use the user's keyboard, pointer or clipboard.

Run the affected scenario with `npm --prefix crates/magistr-desktop run test:ui -- workspace.spec.mjs`.
The scenario writes images and a source-bound receipt directly under
`docs/evidence/desktop-iterations/<iteration>/gallery/`. Set `MAGISTR_GUI_ITERATION`
to a new dated, named iteration; without it, the gallery uses a fresh timestamp.
It refuses to overwrite evidence. Review the actual archived captures, record
per-image findings beside them, then promote accepted images and their receipt
here and update `review.json`. This directory holds only the accepted baseline.
The screenshot gate verifies their integrity and current UI source identity.

This evidence covers visual composition, DOM focus, themes and responsive
behavior. It does not establish Tauri command integration, native OS input,
IME, system file pickers, VoiceOver or Windows parity. Those claims belong to
separately opted-in [native journeys](../desktop-current/README.md).
The current accepted baseline is the [2026-10-15 final gallery](../desktop-iterations/2026-10-15-restart/22-focus-reveal/gallery/review.md). Its per-image review records observations and exact-hash reuse, with [additional graph and shell review](../desktop-iterations/2026-10-15-restart/22-focus-reveal/review.md). Rejected attempts remain in the iteration archive.