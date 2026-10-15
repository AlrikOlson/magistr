# Align dense tables for comparison

Task: Compare table rows and numeric values without oversized cells or numbers.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--readiness-table-compact-dark.png — PASS.** The compact table shows its count, four column headings and first data row; remaining rows are reachable within its bounded scroll region. Check counts align right at the same size as row text; headers and zebra rows remain distinct in dark appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--readiness-table-compact-light.png — PASS.** The compact table shows its count, four column headings and first data row; remaining rows are reachable within its bounded scroll region. Check counts align right at the same size as row text; headers and zebra rows remain distinct in light appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--readiness-table-desktop-dark.png — PASS.** All four readiness rows fit in the desktop table. Check counts align right at the same size as row text; headers and zebra rows remain distinct in dark appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--readiness-table-desktop-light.png — PASS.** All four readiness rows fit in the desktop table. Check counts align right at the same size as row text; headers and zebra rows remain distinct in light appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--table-last-rows-compact-dark.png — PASS.** The bounded compact table shows Source documents and Attachments, with sticky column headings retained while earlier rows scroll underneath. Check counts align right at the same size as row text; headers and zebra rows remain distinct in dark appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--table-last-rows-compact-light.png — PASS.** The bounded compact table shows Source documents and Attachments, with sticky column headings retained while earlier rows scroll underneath. Check counts align right at the same size as row text; headers and zebra rows remain distinct in light appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--table-last-rows-desktop-dark.png — PASS.** All four readiness rows fit in the desktop table. Check counts align right at the same size as row text; headers and zebra rows remain distinct in dark appearance. No horizontal overflow, crowded values or detached controls.

- **conversation--table-last-rows-desktop-light.png — PASS.** All four readiness rows fit in the desktop table. Check counts align right at the same size as row text; headers and zebra rows remain distinct in light appearance. No horizontal overflow, crowded values or detached controls.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
