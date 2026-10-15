# Improve feedback browsing and detail hierarchy

Task: Scan feedback, inspect its evidence and return without excessive summary text.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **feedback--dense-list-compact-dark.png — PASS.** The eight-result count and first five rows are visible; remaining rows scroll below. Kind and status badges align around readable titles and author lines. Inline filters stay on one row. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--dense-list-compact-light.png — PASS.** The eight-result count and first five rows are visible; remaining rows scroll below. Kind and status badges align around readable titles and author lines. Inline filters stay on one row. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--dense-list-desktop-dark.png — PASS.** All eight feedback rows fit, with distinct kind and status badges, aligned authors and readable titles. Filters share one compact row. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--dense-list-desktop-light.png — PASS.** All eight feedback rows fit, with distinct kind and status badges, aligned authors and readable titles. Filters share one compact row. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--no-research-compact-dark.png — PASS.** The full short idea appears once as the focused heading, with Copy beside it. Author/date and Research None follow closely without an empty prose section. Controls do not overlap. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--no-research-compact-light.png — PASS.** The full short idea appears once as the focused heading, with Copy beside it. Author/date and Research None follow closely without an empty prose section. Controls do not overlap. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--no-research-desktop-dark.png — PASS.** The full short idea appears once as the focused heading, with Copy beside it. Author/date and Research None follow closely without an empty prose section. Controls do not overlap. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--no-research-desktop-light.png — PASS.** The full short idea appears once as the focused heading, with Copy beside it. Author/date and Research None follow closely without an empty prose section. Controls do not overlap. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--no-results-compact-dark.png — PASS.** The unmatched query remains readable and focused; its clear control is visible. Zero results and the no-matches message are directly below the heading, without an obsolete record. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--no-results-compact-light.png — PASS.** The unmatched query remains readable and focused; its clear control is visible. Zero results and the no-matches message are directly below the heading, without an obsolete record. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--no-results-desktop-dark.png — PASS.** The unmatched query remains readable and focused; its clear control is visible. Zero results and the no-matches message are directly below the heading, without an obsolete record. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--no-results-desktop-light.png — PASS.** The unmatched query remains readable and focused; its clear control is visible. Zero results and the no-matches message are directly below the heading, without an obsolete record. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--overview-compact-dark.png — PASS.** The researched concern and promoted idea have separate readable title, author, kind and status positions. Kind/Status filters retain explicit labels and search has ample width. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--overview-compact-light.png — PASS.** The researched concern and promoted idea have separate readable title, author, kind and status positions. Kind/Status filters retain explicit labels and search has ample width. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--overview-desktop-dark.png — PASS.** The researched concern and promoted idea have separate readable title, author, kind and status positions. Kind/Status filters retain explicit labels and search has ample width. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--overview-desktop-light.png — PASS.** The researched concern and promoted idea have separate readable title, author, kind and status positions. Kind/Status filters retain explicit labels and search has ample width. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--research-compact-dark.png — PASS.** The complete expanded research note, recorded date, confidence, summary, reasoning link and both source rows are visible. Metadata now shares the content inset; Sources sits near its rows. Focus surrounds the disclosure. Preceding content is partially above the scroll boundary, not concealed within this note. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--research-compact-light.png — PASS.** The complete expanded research note, recorded date, confidence, summary, reasoning link and both source rows are visible. Metadata now shares the content inset; Sources sits near its rows. Focus surrounds the disclosure. Preceding content is partially above the scroll boundary, not concealed within this note. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--research-desktop-dark.png — PASS.** The complete expanded research note, recorded date, confidence, summary, reasoning link and both source rows are visible. Metadata now shares the content inset; Sources sits near its rows. Focus surrounds the disclosure. Preceding content is partially above the scroll boundary, not concealed within this note. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--research-desktop-light.png — PASS.** The complete expanded research note, recorded date, confidence, summary, reasoning link and both source rows are visible. Metadata now shares the content inset; Sources sits near its rows. Focus surrounds the disclosure. Preceding content is partially above the scroll boundary, not concealed within this note. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--selected-compact-dark.png — PASS.** The selected title, author/date, original request and both expectation markers fit without overlap. Related work continues below the scroll boundary; the Research story separately covers the expanded lower content. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--selected-compact-light.png — PASS.** The selected title, author/date, original request and both expectation markers fit without overlap. Related work continues below the scroll boundary; the Research story separately covers the expanded lower content. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

- **feedback--selected-desktop-dark.png — PASS.** The selected title, metadata, original request, expectations, related work and closed research disclosure are all readable with clear grouping. Copy actions sit beside their content. Dark surfaces preserve readable muted labels and lavender focus. No horizontal clipping or text/control overlap.

- **feedback--selected-desktop-light.png — PASS.** The selected title, metadata, original request, expectations, related work and closed research disclosure are all readable with clear grouping. Copy actions sit beside their content. Light surfaces preserve readable muted labels and purple focus. No horizontal clipping or text/control overlap.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
