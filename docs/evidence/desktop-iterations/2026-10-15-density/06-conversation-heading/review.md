# Balance conversation titles and nearby controls

Task: Read long conversation titles while reaching their current actions.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--long-title-compact-dark.png — PASS.** The complete long title wraps onto two readable lines without hiding text. Planned work, status, Details and overflow stay aligned and separate from the title. At 760 × 550 the title controls fit without overlap and the unsent message stays visible. Light/dark text contrast and tab selection are clear.

- **conversation--long-title-compact-light.png — PASS.** The complete long title wraps onto two readable lines without hiding text. Planned work, status, Details and overflow stay aligned and separate from the title. At 760 × 550 the title controls fit without overlap and the unsent message stays visible. Light/dark text contrast and tab selection are clear.

- **conversation--long-title-desktop-dark.png — PASS.** The complete long title wraps onto two readable lines without hiding text. Planned work, status, Details and overflow stay aligned and separate from the title. At 1280 × 820 the title uses the available row width; reply and project navigation remain distinct. Light/dark text contrast and tab selection are clear.

- **conversation--long-title-desktop-light.png — PASS.** The complete long title wraps onto two readable lines without hiding text. Planned work, status, Details and overflow stay aligned and separate from the title. At 1280 × 820 the title uses the available row width; reply and project navigation remain distinct. Light/dark text contrast and tab selection are clear.

- **conversation--release-plan-compact-dark.png — PASS.** The short title remains on one line with a quiet 16px hierarchy. Planned work, status, Details and overflow stay aligned and separate from the title. At 760 × 550 the title controls fit without overlap and the unsent message stays visible. Light/dark text contrast and tab selection are clear.

- **conversation--release-plan-compact-light.png — PASS.** The short title remains on one line with a quiet 16px hierarchy. Planned work, status, Details and overflow stay aligned and separate from the title. At 760 × 550 the title controls fit without overlap and the unsent message stays visible. Light/dark text contrast and tab selection are clear.

- **conversation--release-plan-desktop-dark.png — PASS.** The short title remains on one line with a quiet 16px hierarchy. Planned work, status, Details and overflow stay aligned and separate from the title. At 1280 × 820 the title uses the available row width; reply and project navigation remain distinct. Light/dark text contrast and tab selection are clear.

- **conversation--release-plan-desktop-light.png — PASS.** The short title remains on one line with a quiet 16px hierarchy. Planned work, status, Details and overflow stay aligned and separate from the title. At 1280 × 820 the title uses the available row width; reply and project navigation remain distinct. Light/dark text contrast and tab selection are clear.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
