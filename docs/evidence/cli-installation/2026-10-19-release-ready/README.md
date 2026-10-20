# Cargo/npm release candidate — 2026-10-19

Version `0.1.0`, source [`4fa3788f200c5c807f142f7542e6c159deae777a`](https://github.com/AlrikOlson/magistr/commit/4fa3788f200c5c807f142f7542e6c159deae777a), Apple Silicon macOS.

Registry publication and the declaration of public release have not occurred.
The selected artifacts come from [CLI packages run 39352947283](https://github.com/AlrikOlson/magistr/actions/runs/39352947283).
Their clean Cargo source stamps, checksums, inventories and installation receipts
were checked after download. All 14 Cargo archives reproduced byte for byte from
this commit with Cargo 1.99.1.

[Verification details](verification.json), [artifact inventory](artifact-inventory.json),
[SHA256SUMS](SHA256SUMS), [publication order](publication-order.txt),
[Cargo installation receipt](cargo-install-verification.json) and
[npm installation receipt](npm-install-verification.json) identify this candidate.
The hosted artifact also retains the complete Cargo/npm file lists and the
550-package dependency and license inventory. Binaries are retained in the hosted
artifact and in `target/release-candidate-4fa3788f/`, rather than committed to Git.

| Check | Result |
| --- | --- |
| [Full macOS Rust/website job](https://github.com/AlrikOlson/magistr/actions/runs/39352938183) | 8,065 tests pass, 18 skipped; all-targets check and Clippy, version tests, website build and tracked public assets pass |
| Extracted Cargo packages | Release-profile installation without a repository checkout; setup, doctor, MCP and workspace worker checks pass |
| Packed npm artifact | Global install and npx setup pass without Rust/Bun; fresh pinned configuration, native worker, signals, usage exit status and removal pass |
| Unified protocol | 8 process scenarios pass, including catalog coverage, simultaneous agents/worktrees, restart, stale confirmations, undo, cancellation and partial failure |
| Guided onboarding | 6 process scenarios pass, including repeated setup, selected agents, custom settings, unavailable agents, repair and removal |
| [Desktop gates](https://github.com/AlrikOlson/magistr/actions/runs/39352938183) | 47 Rust and 93 JavaScript tests; screenshot, contrast and icon checks pass |
| [Installable desktop verification](https://github.com/AlrikOlson/magistr/actions/runs/39352938348) | 83 Rust and 4 launcher tests; packaged process roles pass |
| Focused regressions | 91 loop/hygiene tests and 199 ACP agent tests pass across the full selections and corrected-fixture rerun; affected all-targets Clippy passes |
| [Unchanged adapter contracts](https://github.com/AlrikOlson/magistr/actions/runs/39346480131) | 27 tests each on Ubuntu and Windows, retained from unchanged adapter source at `83a98358` |

The standard Rust release jobs run on macOS. Windows desktop builds require an
explicit manual opt-in. The retained Windows adapter check runs JavaScript only.

The product version has one owner: `Cargo.toml [workspace.package].version`.
Use `python3 scripts/version.py set <version>` to update generated metadata and
lockfiles, then `python3 scripts/version.py check`. Private development packages
have no product version. Keep `0.1.0` for the first public release.

The [version-display review](../../desktop-iterations/2026-10-19-release-version/review.md)
covers all eight affected WebKit captures at desktop/compact sizes in light/dark.
The [real-agent comparison](../2026-10-19-unified-mcp/README.md) retains Codex and
Claude Code investigation, editing, planning, feedback, reasoning, verification
and resumption results. Those small design trials predate final binary hashing;
they are not a benchmark or certification of this exact artifact.

This record supersedes earlier candidate installation identities and pending CI
claims. Earlier evidence remains historical. The final selected source passed
its required hosted checks; failed candidates are not publication inputs.

The owner still needs to:

1. Verify the crates.io account email and create a short-lived token permitting
   publication of new crates and new versions. Enter it through the masked prompt:

   ```sh
   gh secret set CARGO_REGISTRY_TOKEN --repo AlrikOlson/magistr --env cli-release
   ```

2. Use an npm account with verified email and two-factor authentication. Log in
   on the publishing Mac and confirm the account:

   ```sh
   npm login --registry=https://registry.npmjs.org
   npm whoami --registry=https://registry.npmjs.org
   ```

3. Authorize the GitHub visibility change for public launch. `magistr.dev`
   responds HTTP 200; the advertised repository, releases and issues links
   return 404 while the repository is private. Public GitHub links and npm OIDC
   provenance require the corresponding public repository setup.

4. Explicitly authorize publication of `0.1.0` from package run
   `39352947283`. The guarded Cargo workflow uses that run's source and
   artifacts. The first npm upload uses its verified tarball and the owner's
   interactive login/2FA; subsequent uploads can use the documented trusted publisher.

[The release procedure](../../../releasing-cli.md) documents the exact publication,
upgrade, repair and removal commands. No credentials belong in this evidence or
in an agent conversation. Name availability in `verification.json` is dated,
does not reserve names, and must be rechecked at publication.

Public endpoint access remains an external launch gate: it was skipped in the
push workflow, and the separate anonymous probe reports the private GitHub links
as inaccessible. No Windows/Linux/Intel Mac acceptance or new native desktop
accessibility result is claimed.
