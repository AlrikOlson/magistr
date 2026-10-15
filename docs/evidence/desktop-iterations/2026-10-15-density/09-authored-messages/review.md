# Present sent messages as compact conversation entries

Task: Distinguish authored messages from replies while retaining copy and selectable content.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--transcript-start-compact-dark.png — PASS.** The sent request uses a shorter card with a clearly readable Copy message action and retained author label. At 760 px the full one-line request remains visible and the reply heading has more clearance above the composer. Dark panel contrast is distinct. No clipping or overlapping controls.

- **conversation--transcript-start-compact-light.png — PASS.** The sent request uses a shorter card with a clearly readable Copy message action and retained author label. At 760 px the full one-line request remains visible and the reply heading has more clearance above the composer. Light panel and accent edge distinguish the request. No clipping or overlapping controls.

- **conversation--transcript-start-desktop-dark.png — PASS.** The sent request uses a shorter card with a clearly readable Copy message action and retained author label. At 1280 px the request aligns with the reply reading column; card and reply remain clearly separated. Dark panel contrast is distinct. No clipping or overlapping controls.

- **conversation--transcript-start-desktop-light.png — PASS.** The sent request uses a shorter card with a clearly readable Copy message action and retained author label. At 1280 px the request aligns with the reply reading column; card and reply remain clearly separated. Light panel and accent edge distinguish the request. No clipping or overlapping controls.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
