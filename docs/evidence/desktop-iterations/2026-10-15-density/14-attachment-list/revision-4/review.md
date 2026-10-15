# Show pending files in compact attachment rows

Task: Review filenames, previews and removal actions without crowding out the draft.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--file-preview-open-compact-dark.png — FAIL.** FAIL: The expanded file is readable but Latest reply still overlaps the transcript's lower padding. Measured grid track is 35.625 px while the transcript's vertical padding requires 48 px. Reduce the compact preview allowance to reserve that space.

- **conversation--file-preview-open-compact-light.png — FAIL.** FAIL: The expanded file is readable but Latest reply still overlaps the transcript's lower padding. Measured grid track is 35.625 px while the transcript's vertical padding requires 48 px. Reduce the compact preview allowance to reserve that space.

- **conversation--file-preview-open-desktop-dark.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

- **conversation--file-preview-open-desktop-light.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

- **conversation--staged-files-compact-dark.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

- **conversation--staged-files-compact-light.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

- **conversation--staged-files-desktop-dark.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

- **conversation--staged-files-desktop-light.png — FAIL.** File controls and content are readable; compact expanded-state overlap remains blocking.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
