# Compact source toolbars and align code gutters

Task: Read source code with nearby copy and wrapping controls and consistent line alignment.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--source-code-compact-dark.png — FAIL.** Toolbar and source remain readable, but the sixth line and bottom padding are outside the captured transcript viewport. Capture must explicitly position the complete short block before review can pass.

- **conversation--source-code-compact-light.png — FAIL.** Toolbar and source remain readable, but the sixth line and bottom padding are outside the captured transcript viewport. Capture must explicitly position the complete short block before review can pass.

- **conversation--source-code-desktop-dark.png — FAIL.** Full six-line source, toolbar and aligned gutter are readable. Overall iteration remains failed until compact captures include the complete short block.

- **conversation--source-code-desktop-light.png — FAIL.** Full six-line source, toolbar and aligned gutter are readable. Overall iteration remains failed until compact captures include the complete short block.

- **conversation--wrapped-source-compact-dark.png — FAIL.** Toolbar and source remain readable, but the sixth line and bottom padding are outside the captured transcript viewport. Capture must explicitly position the complete short block before review can pass.

- **conversation--wrapped-source-compact-light.png — FAIL.** Toolbar and source remain readable, but the sixth line and bottom padding are outside the captured transcript viewport. Capture must explicitly position the complete short block before review can pass.

- **conversation--wrapped-source-desktop-dark.png — FAIL.** Full six-line source, toolbar and aligned gutter are readable. Overall iteration remains failed until compact captures include the complete short block.

- **conversation--wrapped-source-desktop-light.png — FAIL.** Full six-line source, toolbar and aligned gutter are readable. Overall iteration remains failed until compact captures include the complete short block.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
