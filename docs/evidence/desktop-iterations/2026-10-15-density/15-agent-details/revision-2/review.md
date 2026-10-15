# Align agent settings and usage in the details panel

Task: Inspect configuration and usage with clear labels and compact controls.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--agent-details-desktop-dark.png — FAIL.** Desktop usage fields are readable. Compact capture was rejected because the arrange selector matched the hidden Agent settings button instead of the details section. Qualify the section selector before acceptance.

- **conversation--agent-details-desktop-light.png — FAIL.** Desktop usage fields are readable. Compact capture was rejected because the arrange selector matched the hidden Agent settings button instead of the details section. Qualify the section selector before acceptance.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
