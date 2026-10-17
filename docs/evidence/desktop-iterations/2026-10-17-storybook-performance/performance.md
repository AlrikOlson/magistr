# Storybook development loading

The catalog was watching native build output inside the desktop crate. Vite's project root is `crates/magistr-desktop`, which also contains Cargo's `target/`. In this environment Chokidar registered polling watchers at 100 ms intervals. Excluding generated folders fixes the boundary without deleting build artifacts or disabling source watching.

## Observed measurements

Measured locally on 2026-10-17, macOS 26.5.2 arm64, Storybook 10.6.2, Vite 8.3.4, Playwright 1.64.1/headless WebKit. Baseline source: commit `45f3a2e8`; corrected catalog hashes are recorded in the adjacent capture receipts.

| Measurement | Before | After |
|---|---|---|
| Polling registrations during server startup | 179,175 | 236 |
| Registrations under generated output | Mostly Rust target: 132,133 debug; 43,306 target-platform; 3,421 release; 62 app-icons | 0 |
| Manager globals JavaScript transfer, fresh WebKit context | 29,220 ms | 22 ms |
| First usable directory story, fresh WebKit context | Not ready at 49,844 ms; observation timed out | 512 ms |
| Conversation / roadmap, fresh document navigation | Not measured before | 397 / 424 ms |
| Recorded regression: directory load then sidebar switches to conversation / roadmap / research | Not measured before | 621 / 279 / 313 / 518 ms |

Methods: Playwright navigation and story-ready observation; browser resource timings; temporary `fs.watchFile` instrumentation on an owned server process; macOS `sample` (2 seconds) and `ps`. The baseline sample showed libuv polling/stat work, about 428% CPU and a 1.7 GB footprint. Startup itself reported 85 ms for the manager and 49 ms for preview, so the server's ready message did not reveal its stalled resource delivery. The temporary instrumentation was removed after measuring. Timings are individual local observations, not a population percentile or a cross-machine guarantee. No cache directories or native build artifacts were deleted between measurements.

Vite documents that its watcher covers the project root and does not exclude Cargo output by default; polling can cause high CPU use. [Vite server.watch](https://vite.dev/config/server-options#server-watch). The catalog extends the existing configuration through the supported `viteFinal` hook. [Storybook Vite configuration](https://storybook.js.org/docs/builders/vite#configuration).

## Verification

- `node --test crates/magistr-desktop/tests/storybook_watch.test.mjs` checks actual watch registration and changes for UI, story and fixture sources, while excluding generated directories. The initial test exposed that Chokidar's ready event can precede nested registration; it now waits for observable source registration with a deadline.
- `npm --prefix crates/magistr-desktop run test:stories:dev` starts the development server and navigates through the manager with five-second stage limits.
- Captured these 16 images with `MAGISTR_GUI_STORY=projects-directory--list,conversation--release-plan,roadmap--overview,feedback--research MAGISTR_GUI_ITERATION=2026-10-17-storybook-performance npm --prefix crates/magistr-desktop run test:stories:dev`. Three checks passed: development navigation, verified captures, and rejection of a wrong-screen capture.
- See [per-image visual review](review.md) for inspection and native verification limits.
