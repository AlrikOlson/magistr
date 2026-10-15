# Show pending files in compact attachment rows

Task: Review filenames, previews and removal actions without crowding out the draft.

Each image was opened from this canonical folder and inspected at full frame and readable detail. Adjacent JSON receipts record viewport, appearance, renderer, source hashes and capture time.

- **conversation--file-preview-open-compact-dark.png — FAIL.** FAIL: Opening Preview leaves the actual file text below the bounded attachment region. The preview is technically open but unreadable without another scroll; fix the attachment region sizing and inspect again.

- **conversation--file-preview-open-compact-light.png — FAIL.** FAIL: Opening Preview leaves the actual file text below the bounded attachment region. The preview is technically open but unreadable without another scroll; fix the attachment region sizing and inspect again.

- **conversation--file-preview-open-desktop-dark.png — FAIL.** FAIL: Opening Preview leaves the actual file text below the bounded attachment region. The preview is technically open but unreadable without another scroll; fix the attachment region sizing and inspect again.

- **conversation--file-preview-open-desktop-light.png — FAIL.** FAIL: Opening Preview leaves the actual file text below the bounded attachment region. The preview is technically open but unreadable without another scroll; fix the attachment region sizing and inspect again.

- **conversation--staged-files-compact-dark.png — FAIL.** File names, metadata and removal controls are readable. The expanded preview state remains blocked, and ID-specific rules override the intended thumbnail reduction.

- **conversation--staged-files-compact-light.png — FAIL.** File names, metadata and removal controls are readable. The expanded preview state remains blocked, and ID-specific rules override the intended thumbnail reduction.

- **conversation--staged-files-desktop-dark.png — FAIL.** File names, metadata and removal controls are readable. The expanded preview state remains blocked, and ID-specific rules override the intended thumbnail reduction.

- **conversation--staged-files-desktop-light.png — FAIL.** File names, metadata and removal controls are readable. The expanded preview state remains blocked, and ID-specific rules override the intended thumbnail reduction.

Headless WebKit with isolated observations. Native OS input, assistive technology and Rust command integration were not exercised.
