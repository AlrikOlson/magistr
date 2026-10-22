# CI workflow rewrite, 2026-10-22

Measurements behind the rewrite of the GitHub workflows (`ci.yml`, `security.yml`,
`public-links.yml`, `desktop-apps.yml`, `cli-packages.yml`, `cli-publish.yml`),
landed as 44893321 and 395026f1 with the required checks renamed in 4de7389d.
Job times are wall-clock durations from the Actions API; stage times are the
`scripts/ci_timing.py` receipts, which since this change record whether the Rust
cache was restored (`cold`).

## Before: why every verification cycle took twenty minutes

| Cause | Evidence |
| --- | --- |
| Rust caches keyed on `hashFiles('crates/**/*.rs')`; rust-cache restores only an exact key, so every Rust change missed every cache | `public-release.yml` and `desktop-apps.yml` before 616b02d7 |
| Caches saved only from verify branches; `main` reused the verify run and built nothing, so `main` owned no Rust cache and no other ref could read a sibling's | inventory at 02:35 UTC: 23 caches, 10.46 GB, three copies each of `desktop` 1.25 GB, `ci-rust` 1.09 GB, `cli-release` 0.76 GB under dead `verify/*` and `refs/pull/*` refs, none on `main` |
| Repository over the 10 GB cache limit, so GitHub evicted caches as fast as they were written | same inventory |
| Stage deadlines calibrated on warm runs killed cold runs | desktop lint 480 s, package 720 s and website-ui 120 s budgets exceeded in runs 39702214174 and 39707719582 |
| One serial macOS desktop job on the critical path | warm run 39700275551: desktop 1011 s against rust-build 321 s |
| Five concurrent macOS jobs on this plan, all contended by builds, website, contracts, package, desktop and security | GitHub usage limits; queued test partitions in every run |

## Job wall clock (seconds)

| Job | Old layout, warm (39700275551) | New layout, cold verify branch (39715950002) | New layout, first `main` run, cold (39722089260) | New layout, `main` warm (39726926659) |
| --- | --- | --- | --- | --- |
| plan | 9 | 6 | 6 | 6 |
| policy (now actionlint + zizmor) | 14 | 24 | 16 | 17 |
| rust-build | 321 | 770 | 816 | 341 (stage 285) |
| rust-tests | 4 × 108–133 | 3 × 184–194 | 3 × 182–217 | 3 × 157–203 |
| website | 141 | 209 | 214 | 225 (ui 167) |
| contracts (macOS → Linux) | 74 | 36 | 32 | 42 |
| visual | 33 | 18 | 19 | 17 |
| desktop, serial | 1011 | — | — | — |
| desktop checks | — | 1575 (lint 602, test build 775) | 1375 | 301 (lint 32, test build 194, tests 17) |
| desktop installer | — | 1984 (package 1230, lint 201, tests 386) | 1530 (package 954) | 1040 (package 443, lint 107, tests 384) |
| package | 361 | 897 | 1084 | 375 (stage 333) |
| security (macOS → Linux) | 60 | 64 | 92 | 85 |
| required aggregate after push | 17 min | 33 min | 57 min (installer rerun after a flaky test) | 17.5 min |

A warm verify branch carrying one change in `crates/magistr-process` (run
39729279284, seven Rust packages selected, website and package skipped):
rust-build 474 s (stage 412 s), test slices 4 min each, desktop checks 6 min
(lint 38 s, test build 244 s), desktop installer 20.4 min (package 485 s,
packaged lint 142 s, packaged tests 457 s); CI aggregate 20.6 min after the push,
12.5 min without the installer.

Cold figures are what a lockfile change costs; rust-cache keys on the lockfiles,
so every `Cargo.lock` change rebuilds cold and `main` re-saves.

## What the first cold runs corrected before landing

- Two deadlines per compile stage. The split installer job was killed at its
  900 s warm budget while still compiling; `ci_timing` now takes `--cold-budget`,
  selected only when the rust-cache composite reports that the exact key was not
  restored, and the receipt records `cold`.
- The security scan refused `apps/website` three times on Linux with an empty
  npm error. The cause was npm's registry incident (opened 02:33 UTC): the bulk
  advisory request hung from Linux, macOS and a developer machine alike. The
  scanner now attempts each audit three times with a 60 s fetch timeout, keeps
  npm's log in the refusal, and names an advisory-service outage; a first
  diagnosis blaming Linux runner egress was withdrawn.
- The clean npm install verification in the CLI packages workflow let
  `npm install` audit by default and hung for its whole 300 s budget during the
  same incident; it now installs with audits and fund lookups off.
- Dead caches were deleted (19 entries, 9.8 GB to 277 MB) so `main`'s first
  save was not evicted.

## Cache inventory after the first `main` run

16 caches, 3.85 GB, all Rust caches owned by `main`: desktop 1127 MB, ci-rust
1090 MB, cli-release 757 MB, plus node, WebKit, test-registry and wasmtime caches.

## Release workflows

CLI packages run 39722091766 on `main` built cold in 34 min (build 15 min, Cargo
clean-install verification 16 min, npm clean-install verification 49 s), scanned,
and attested the reviewed archives. The deploy job on `main` skipped with the
notice to add the Cloudflare secrets.

## What remains

The warm end-to-end time is bound by the desktop installer job (17 to 20 min:
package 443–485 s, packaged lint 107–142 s, packaged tests 384–457 s), which
`ci_plan` selects for any change in the seven crates the desktop depends on;
without it a core-crate change would reach the aggregate in about 12.5 min,
with rust-build (412 s for a change under seven packages) and the test slices
(4 min) next in line. Reaching the ten-minute target needs that path
restructured (follow-up chunk `ci-installer-critical-path`), not more caching. Two flaky tests surfaced and
were rerun, never patched: the process settlement test
(`process-group-exit-test-wait-bound`) and the owner test's fixed 15 s first-call
deadline (`mcp-project-owner-first-call-deadline`).
