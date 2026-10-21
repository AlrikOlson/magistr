# Verification follow-up

The product source and all rendered content remain identical to the reviewed captures and their source hashes. Only `apps/website/tests/browser.mjs` changed after capture; every other recorded source hash was checked against the checkout.

GitHub run 39651005697 first failed because its clean install omitted Pagefind's optional macOS binary. The same lockfile and npm version installed that binary in a clean local directory, and retrying the failed CI job passed the website build. The retry then reached the 120-second browser limit during the keyboard-playback wait. The same scenario passed locally without captures.

The final test keeps exhaustive 65-entry completeness, escaping and copy constraints in the data tests. Browser copy assertions now accompany the existing 11 explanation scenes across desktop/mobile and light/dark, removing 65 duplicate cumulative terminal redraws. Keyboard playback now has a 10-second deadline with focus, visibility and cursor diagnostics. The focused WebKit run passed in 59.2 seconds (Magistr receipt `e79cc904-06c3-470a-9161-9bb4c547aa9b`). The original 65-action browser run and its screenshots remain recorded in this folder.

This changes verification only. It introduces no new interface state or visual iteration and does not replace the full-frame review in REVIEW.md.
