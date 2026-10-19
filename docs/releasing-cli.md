# CLI package release

The first package target is Apple Silicon macOS. Cargo installs package
`magistr-cli` and exposes the `magistr` executable. npm package `magistr`
contains the same versioned native executable and supports global installation
and `npx magistr init`. Its bin entry points directly to the executable, preserving
stdio, signals, exit status and native worker startup. npm consumers need no
Rust, Bun, download hook or source checkout.

Desktop distribution and other operating systems are separate release work.

## Prepare and verify

From the exact source revision intended for release on macOS arm64:

```sh
python3 scripts/distribution.py prepare --out target/cli-distribution
python3 scripts/distribution.py verify-cargo --out target/cli-distribution
python3 scripts/distribution.py verify-npm --out target/cli-distribution
```

Preparation builds the release executable and packages its Cargo dependency
closure in topological order. Each internal dependency has an exact registry
version as well as its development path. Package inventories, dependency notices,
QuickJS provenance and SHA-256 checksums accompany the artifacts. The binary inventory
covers the macOS runtime and build dependency closure, including nested native-library
notices. For crates that omit notices from their archives, checked-in copies come from
the exact source commit recorded by Cargo; packaging verifies their hashes and refuses
missing notices. After a dependency update, maintain those copies explicitly with
`python3 scripts/distribution_licenses.py refresh` and review its manifest.

Cargo verification extracts the actual normalized packages into an isolated
directory and installs the CLI offline against a checksum-verified vendor source.
Unpublished internal packages are supplied as registry packages, without path
patches or a checkout dependency. The installed executable performs setup,
doctor, MCP context, sandbox and workspace checks. `--build-dir` may select an
external build cache for a subsequent verification of corrected artifacts.

npm verification uses the packed tarball, an isolated global prefix and a PATH
without Rust or Bun. It checks global installation, npx setup, exit codes, signals,
worker startup and removal. A loopback registry serving only that tarball and a
fresh npm cache exercise the generated version-pinned npx configuration.

The manual **CLI packages** GitHub workflow performs these checks on a hosted
macOS arm64 runner. Its default prepares and uploads artifacts without publishing.
Review the inventories and actual verification JSON before releasing.

## Registry prerequisites

Check npm `magistr`, Cargo `magistr-cli` and every name in
`publication-order.txt` immediately before publication. Availability does not
reserve a name or establish permission. On 2026-10-19 the proposed top-level names
were unregistered; this workstation had no authenticated npm publisher.
Cargo publishing access must be verified by the release owner.

Configure the protected GitHub environment `cli-release`, with a required
reviewer, and a Cargo token authorized for all listed crates. For npm, establish
package ownership through the registry's first-publication process, then configure
a trusted publisher for this repository, workflow `cli-packages.yml` and
environment `cli-release`. npm requires Node 22.14+ and npm 11.5.1+ for OIDC.
GitHub-hosted publishing from a public repository to a public package generates
provenance automatically. A private repository does not receive that provenance.
See [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).

The publication job is separately selected by the `publish` workflow input and
waits for environment approval. Cargo publishes dependency-first and waits for
registry indexing. If any publication fails, inspect registry state before
continuing; published versions are immutable. Do not blindly rerun a partial
release. See [Cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).

No package has been published by preparing or verifying these artifacts.
The owner declares public release separately.

## Install, upgrade, repair and remove

After publication:

```sh
cargo install magistr-cli --locked
# or:
npm install --global magistr

magistr init
magistr doctor
magistr repair --dry-run
magistr repair --yes
magistr remove --dry-run
magistr remove --yes
```

Upgrade with `cargo install magistr-cli --locked --force` or
`npm install --global magistr@latest`, then explicitly run `magistr repair`
in each project. npx-generated configurations pin the package version; run the
new version's setup/repair to update them. Existing sessions keep their admitted
owners until closed; reopen clients to use a new installation.

Removal deletes unchanged owned integration files and entries while keeping
customizations, other servers and project workflow state. Uninstall the executable
separately with `cargo uninstall magistr-cli` or `npm uninstall --global magistr`.
