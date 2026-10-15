# Show pending files in compact attachment rows

Task: Review filenames, previews and removal actions without crowding out the draft.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--file-preview-open-compact-dark.png — FAIL.** FAIL: File text is now readable, but the expanded attachments leave too little transcript height and Latest reply overlaps the remaining text. Bound the preview to the available short-window height.

- **conversation--file-preview-open-compact-light.png — FAIL.** FAIL: File text is now readable, but the expanded attachments leave too little transcript height and Latest reply overlaps the remaining text. Bound the preview to the available short-window height.

- **conversation--file-preview-open-desktop-dark.png — FAIL.** The text preview and removal control are readable; the iteration remains failed for the compact overlap.

- **conversation--file-preview-open-desktop-light.png — FAIL.** The text preview and removal control are readable; the iteration remains failed for the compact overlap.

- **conversation--staged-files-compact-dark.png — FAIL.** Compact thumbnails and readable filenames are correctly applied; closed previews retain the draft and send controls.

- **conversation--staged-files-compact-light.png — FAIL.** Compact thumbnails and readable filenames are correctly applied; closed previews retain the draft and send controls.

- **conversation--staged-files-desktop-dark.png — FAIL.** Compact thumbnails and readable filenames are correctly applied; closed previews retain the draft and send controls.

- **conversation--staged-files-desktop-light.png — FAIL.** Compact thumbnails and readable filenames are correctly applied; closed previews retain the draft and send controls.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
