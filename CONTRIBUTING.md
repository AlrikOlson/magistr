# Contributing to Magistr

Magistr is in early public development. Contributions that improve roadmap and workflow management, make recorded work easier to review, or clarify the source-first release path are welcome.

## Start here

1. Read the [development guide](docs/development.md) and the [architecture overview](docs/architecture/overview.md).
2. Search the native roadmap and existing issues before starting work.
3. Keep a change focused on one problem and describe the user-visible result.
4. Add or update meaningful checks when behavior changes.

Magistr's own workflow uses one ready roadmap item at a time. A typical implementation runs research, planning, execution, verification, and an atomic commit. The repository keeps roadmap state in native Magistr storage; do not generate `ROADMAP.md` or `ROADMAP.json`.

## Required checks

Run formatting and the commands in `.magistr/project.json` before opening a pull request. Record the results through Magistr's gate or ship-check workflow when possible. Website changes also require `npm test` from `apps/website`; release changes should run `./scripts/package-release.sh` and verify the matching checksum.

Keep shared contracts independent of presentation. Read the desktop and session architecture guides before changing those boundaries. Preserve reduced-motion behavior, contrast, icon, screenshot, and architecture-index checks.

## Pull requests

Explain the problem, the behavior after the change, and the checks you ran. Include screenshots for intentional visual changes. Keep generated files and unrelated formatting out of the diff.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). For security issues, use the private reporting guidance in [SECURITY.md](SECURITY.md).