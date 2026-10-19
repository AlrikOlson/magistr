# Magistr configuration and installation verification

Source revision: `ae2d79044ffe6b93401d41a87d99466b02a7e1ac`.
The configuration consolidation is in `fb9f61bcc722cb94ccd2e487cbac80e6f6d6af01`;
the following commit excludes local desktop and Python metadata from packages.
These records describe prepared artifacts, not a public release.

## Installation checks

All commands completed successfully on Apple Silicon macOS:

```sh
python3 -B scripts/distribution.py prepare --out target/consolidated-packages
python3 -B scripts/distribution.py verify-source --out target/consolidated-packages
python3 -B scripts/distribution.py verify-npm --out target/consolidated-packages
python3 -B scripts/distribution.py verify-cargo --out target/consolidated-packages --debug --build-dir target/extracted-cargo-build
```

The 14 Cargo archives reproduce from the recorded source revision. Each archive
has clean Git provenance and excludes local metadata and build directories.
The package inventories, checksums and dependency publication order are saved
beside this report.

The npm check installs the packed native executable globally and through npx,
with Rust and Bun absent from PATH. It checks a freshly cached, version-pinned
MCP registration, project paths containing spaces, native worker startup,
read-only context, usage exit codes, SIGTERM and removal.

The Cargo check installs only extracted archives using a checksum-verified
vendored registry, including the unpublished internal packages. No source
checkout is available to the installed executable. This local check uses the
debug profile and a reusable build directory; it is not evidence of a fresh
hosted release-profile build. The npm package contains the release executable.

No registry publication or declaration of public release occurred.

## Related verification

- 5,980 affected library tests passed, followed by 187 focused ownership,
  configuration, setup, plugin, schema and workflow tests.
- Unified MCP: eight scenarios covering authorization, cancellation, partial
  failure, undo, owner restart, worktrees and schema coverage. Onboarding: six
  scenarios covering selected agents, custom settings, repeat setup and repair.
- Claude adapter: its native integration scenario and 27 contract tests passed.
- Formatting and Clippy passed for the nine affected crates. Source ownership,
  architecture, structure and workflow/workspace source gates passed.
- [Setup visual review](../../desktop-iterations/2026-10-19-magistr-setup/review.md)
  records inspection of all 16 current headless WebKit captures.
  [Diagram review](../../desktop-iterations/2026-10-19-magistr-diagrams/review.md)
  records the two updated illustrations.

Native verification receipt for the complete packaging command sequence:
`2e6a0729-dbd9-41bc-afeb-f55fcdea473e`.

Normal CI runs target macOS for desktop builds. Windows is available only as an
explicit manual desktop workflow option; it is not a release-readiness dependency.
Linux continues to run the Rust and website quality checks.
