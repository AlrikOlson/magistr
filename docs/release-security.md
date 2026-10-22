# Release security checks

The release security workflow scans each pull request and main revision on macOS
arm64. CLI packaging repeats the scans and records the checksums of the actual
Cargo and npm archives. Publication requires both a successful security run for
the selected revision and a passing artifact receipt less than 24 hours old.

Run a local diagnostic with the same scanners:

```sh
cargo install cargo-audit --locked --version 0.22.1
python3 -B scripts/release-security.py install-gitleaks --out /tmp/magistr-security-bin
python3 -B scripts/release-security.py scan --gitleaks /tmp/magistr-security-bin/gitleaks \
  --source "$(git rev-parse HEAD)" --out target/release-security/security-verification.json \
  --allow-dirty
```

`--allow-dirty` supports local investigation; its receipt explicitly records a
dirty tree and cannot authorize publication. The scan evaluates the macOS
release target graph (`cargo tree --target aarch64-apple-darwin`, `cargo audit
--target-os macos`) on either reviewed host: the Security workflow runs it on
Linux x86_64, package runs on macOS arm64; each host has its own pinned gitleaks
archive and checksum in `scripts/release-security.py`. Each npm audit request
has a 60 s fetch timeout and is attempted three times; a refusal carries npm's
log, and when the advisory service still has not answered (as during the
registry incident of 2026-10-22) the refusal says so and the scan is rerun once
https://status.npmjs.org is clear, because an unanswered audit is not a clean one.
A clean scan also requires complete Git history. The hosted checkout uses
`fetch-depth: 0`.

The gate checks:

- RustSec advisories and all maintenance/soundness warnings, intersected with
  the enabled CLI normal and build dependencies for `aarch64-apple-darwin`.
  Matches outside that graph remain in the receipt with their exclusion reason.
- Current npm registry advisories for the complete website and scripts locks,
  including development dependencies, at every severity.
- Gitleaks findings in the entire selected revision's ancestry. Inline
  allow-comments, repository allowlists and broad file exclusions cannot hide
  findings. Receipts omit secret values.
- Full commit pins for external actions in every workflow and their local
  composite actions; complete Rust, Node.js and Python toolchain versions.

A scanner error, unavailable registry, incomplete history, changed input or
expired receipt fails the gate. A failed rescan invalidates an earlier green
receipt. Advisory database commits, input hashes, scan times and tool versions
are saved in `security-verification.json`. The receipt is build evidence, not a
cryptographic signature or a claim that scanners detect every vulnerability.

## Reviewing a finding

Prefer upgrading or removing the affected dependency. The current
`scripts/release-security-policy.json` has no dependency exceptions.

An unavoidable, demonstrably inapplicable advisory needs a specific review:
`advisory`, `package`, `version`, `target`, `reason`, `featurePath`,
`upstream`, `reviewedOn`, `expiresOn`, and `evidence` entries containing
`file` and `sha256`. Reviews last no more than 90 days. A version change, expired
review or changed evidence requires a new assessment. Do not use an exception
for an uninvestigated scanner match or to postpone an available fix.

Secret reviews match one immutable commit, file, rule and line. The initial 21
entries reference the contextual review in the October 19 release audit.
A new match, including one in the same fixture at another commit or line,
requires review; a directory-wide test exclusion is not accepted.

## Updating scanners and build tools

Rust is pinned in `rust-toolchain.toml`, Node.js in `.node-version`, and Python in
`.python-version`. The shared `.github/actions/install-nextest/action.yml`
composite owns the exact tested Nextest version for all CI consumers.
Scanner versions and the Gitleaks download checksum live in
`scripts/release-security.py`. Update the Gitleaks version, URL and checksum
together using the upstream release; the installer extracts only its verified
executable. The Cargo scanner installer uses a pinned action and disables fallback
installers.

Run `python3 -B scripts/release-security.tests.py`,
`python3 -B scripts/release-security.py configuration` and a current scan after
changing these controls. Review upstream scanner coverage when updating them.

The policy follows [RustSec's report and advisory model](https://docs.rs/rustsec/latest/rustsec/report/index.html),
[Gitleaks' documented scanner controls](https://github.com/gitleaks/gitleaks/tree/v8.30.1),
and [GitHub's action-pinning guidance](https://docs.github.com/en/actions/reference/security/secure-use).
