# Background desktop layout evidence

These captures render the production desktop HTML, CSS and JavaScript in
headless WebKit with realistic, isolated observation fixtures. They do not
open desktop windows or use the user's keyboard, pointer or clipboard.

Run the affected scenario with `npm --prefix crates/magistr-desktop run test:ui -- workspace.spec.mjs`.
The scenario writes images and a source-bound receipt to `.magistr/desktop-layout`.
Review the actual captures before copying them here and updating `review.json`.
The screenshot gate verifies their integrity and current UI source identity.

This evidence covers visual composition, DOM focus, themes and responsive
behavior. It does not establish Tauri command integration, native OS input,
IME, system file pickers, VoiceOver or Windows parity. Those claims belong to
separately opted-in [native journeys](../desktop-current/README.md).
