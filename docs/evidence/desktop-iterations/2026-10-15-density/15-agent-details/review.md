# Align agent settings and usage in the details panel

Task: Inspect configuration and usage with clear labels and compact controls.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--agent-details-compact-dark.png — FAIL.** FAIL: Details opened at its top after the deferred focus operation, so Agent settings are below the captured frame. The story must wait for focus placement and verify the usage fields are in the viewport.

- **conversation--agent-details-compact-light.png — FAIL.** FAIL: Details opened at its top after the deferred focus operation, so Agent settings are below the captured frame. The story must wait for focus placement and verify the usage fields are in the viewport.

- **conversation--agent-details-desktop-dark.png — FAIL.** The desktop agent block shows a readable provider, native model control and compact labeled usage. Compact coverage remains missing.

- **conversation--agent-details-desktop-light.png — FAIL.** The desktop agent block shows a readable provider, native model control and compact labeled usage. Compact coverage remains missing.

- **conversation--usage-unavailable-compact-dark.png — FAIL.** FAIL: Details opened at its top after the deferred focus operation, so Agent settings are below the captured frame. The story must wait for focus placement and verify the usage fields are in the viewport.

- **conversation--usage-unavailable-compact-light.png — FAIL.** FAIL: Details opened at its top after the deferred focus operation, so Agent settings are below the captured frame. The story must wait for focus placement and verify the usage fields are in the viewport.

- **conversation--usage-unavailable-desktop-dark.png — FAIL.** The desktop agent block shows a readable provider, native model control and compact labeled usage. Compact coverage remains missing.

- **conversation--usage-unavailable-desktop-light.png — FAIL.** The desktop agent block shows a readable provider, native model control and compact labeled usage. Compact coverage remains missing.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
