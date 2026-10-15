# Use a compact readable hierarchy for agent replies

Task: Scan headings, paragraphs and lists without oversized document typography.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--reply-end-compact-dark.png — PASS.** The compact end position shows the concluding paragraph, Next step subheading and recommendation above the composer. Paragraph and list spacing is tighter without crowding. Dark appearance remains readable. Composer and navigation retain their placement.

- **conversation--reply-end-compact-light.png — PASS.** The compact end position shows the concluding paragraph, Next step subheading and recommendation above the composer. Paragraph and list spacing is tighter without crowding. Light appearance remains readable. Composer and navigation retain their placement.

- **conversation--reply-end-desktop-dark.png — PASS.** The desktop end position shows the reply heading, findings, two list items and Next step together. Paragraph and list spacing is tighter without crowding. Dark appearance remains readable. Composer and navigation retain their placement.

- **conversation--reply-end-desktop-light.png — PASS.** The desktop end position shows the reply heading, findings, two list items and Next step together. Paragraph and list spacing is tighter without crowding. Light appearance remains readable. Composer and navigation retain their placement.

- **conversation--transcript-start-compact-dark.png — PASS.** The sent request and 20 px reply heading are visible with clear author boundaries. At 760 px the beginning of the reply remains above the fixed composer; Latest reply is reachable. Dark text surfaces and purple accents remain distinct. No horizontal clipping or control overlap.

- **conversation--transcript-start-compact-light.png — PASS.** The sent request and 20 px reply heading are visible with clear author boundaries. At 760 px the beginning of the reply remains above the fixed composer; Latest reply is reachable. Light surfaces and body text retain clear contrast. No horizontal clipping or control overlap.

- **conversation--transcript-start-desktop-dark.png — PASS.** The sent request and 20 px reply heading are visible with clear author boundaries. At 1280 px the heading, subheading and two list rows form a readable hierarchy with shorter gaps. Dark text surfaces and purple accents remain distinct. No horizontal clipping or control overlap.

- **conversation--transcript-start-desktop-light.png — PASS.** The sent request and 20 px reply heading are visible with clear author boundaries. At 1280 px the heading, subheading and two list rows form a readable hierarchy with shorter gaps. Light surfaces and body text retain clear contrast. No horizontal clipping or control overlap.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
