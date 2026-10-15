# Reduce the space used by reply navigation

Task: Move through reply sections using a concise outline that preserves content width.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--reply-sections-compact-dark.png — PASS.** The navigator occupies one small disclosure row; the first heading and body text retain the full reading column. At 760 px navigation stays above the scrollable reply and does not overlap the composer. Dark appearance has clear text and focus contrast.

- **conversation--reply-sections-compact-light.png — PASS.** The navigator occupies one small disclosure row; the first heading and body text retain the full reading column. At 760 px navigation stays above the scrollable reply and does not overlap the composer. Light appearance has clear text and focus contrast.

- **conversation--reply-sections-desktop-dark.png — PASS.** The navigator occupies one small disclosure row; the first heading and body text retain the full reading column. At desktop size the former narrow side column is gone, reducing wrapping in the reply. Dark appearance has clear text and focus contrast.

- **conversation--reply-sections-desktop-light.png — PASS.** The navigator occupies one small disclosure row; the first heading and body text retain the full reading column. At desktop size the former narrow side column is gone, reducing wrapping in the reply. Light appearance has clear text and focus contrast.

- **conversation--reply-sections-open-compact-dark.png — PASS.** The expanded navigator shows four distinct section controls in two compact columns, with a visible focus ring on the toggle. At 760 px navigation stays above the scrollable reply and does not overlap the composer. Dark appearance has clear text and focus contrast.

- **conversation--reply-sections-open-compact-light.png — PASS.** The expanded navigator shows four distinct section controls in two compact columns, with a visible focus ring on the toggle. At 760 px navigation stays above the scrollable reply and does not overlap the composer. Light appearance has clear text and focus contrast.

- **conversation--reply-sections-open-desktop-dark.png — PASS.** The expanded navigator shows four distinct section controls in two compact columns, with a visible focus ring on the toggle. At desktop size the former narrow side column is gone, reducing wrapping in the reply. Dark appearance has clear text and focus contrast.

- **conversation--reply-sections-open-desktop-light.png — PASS.** The expanded navigator shows four distinct section controls in two compact columns, with a visible focus ring on the toggle. At desktop size the former narrow side column is gone, reducing wrapping in the reply. Light appearance has clear text and focus contrast.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
