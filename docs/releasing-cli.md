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

## Registry setup for the owner

Check npm `magistr`, Cargo `magistr-cli` and every name in
`publication-order.txt` immediately before publication. Availability does not
reserve a name or establish permission.

The GitHub environment `cli-release` is restricted to the `main` branch.
Required environment reviewers are available on public repositories with
GitHub Pro; a private Pro repository cannot enable them. On the current private
repository, publication requires a separate manual workflow dispatch.
Changing repository visibility is a separate owner decision. See
[GitHub environment restrictions](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments).

Before the first release, the owner must:

1. Sign in to [crates.io](https://crates.io/) and verify the account's email.
   Create a short-lived [API token](https://crates.io/settings/tokens/new) that can
   publish **new crates** and new versions for every name in
   `publication-order.txt`. A token restricted to updating existing crates
   cannot create the first release. Add it as an environment secret using the
   masked terminal prompt:

   ```sh
   gh secret set CARGO_REGISTRY_TOKEN --repo AlrikOlson/magistr --env cli-release
   ```

2. Sign in to npm and enable account two-factor authentication. On the Mac that
   will perform the first upload, run:

   ```sh
   npm login --registry=https://registry.npmjs.org
   npm whoami --registry=https://registry.npmjs.org
   ```

   Complete the browser authentication yourself. Do not paste passwords, tokens
   or recovery codes into an agent conversation.

3. Review the successful hosted `CLI packages` run and explicitly authorize
   publication of its version. Preparation and verification never publish.

npm requires a package to exist before a trusted publisher can be configured.
The first upload therefore uses the **verified tarball from that hosted run**
with the owner's npm login and interactive 2FA; do not publish a placeholder:

```sh
npm publish target/cli-distribution/magistr-0.1.0.tgz --access public
```

This command is the public first release and must only run after authorization.
After that upload, configure the package's trusted publisher at
[npm package settings](https://www.npmjs.com/package/magistr/access):

| Field | Value |
| --- | --- |
| Provider | GitHub Actions |
| Organization or user | `AlrikOlson` |
| Repository | `magistr` |
| Workflow filename | `cli-publish.yml` |
| Environment | `cli-release` |
| Allowed action | `npm publish` |

With npm 11.15 or later, the equivalent owner command is:

```sh
npm trust github magistr --file cli-publish.yml --repo AlrikOlson/magistr --env cli-release --allow-publish
```

It requires interactive 2FA. The next successful trusted publication must occur
within two days of creating the configuration; otherwise recreate it when the
next release is ready. See [npm trust](https://docs.npmjs.com/cli/v11/commands/npm-trust/)
and [trusted publishing](https://docs.npmjs.com/trusted-publishers/).
OIDC publishing requires Node 22.14+ and npm 11.5.1+; the workflow uses Node 24.
npm provenance is generated for public packages published with OIDC from public
repositories. Private repositories can publish but do not receive that provenance.

## Publish reviewed artifacts

`CLI packages` only prepares, verifies and uploads `cli-distribution` on hosted
macOS arm64. It checks extracted Cargo installation in the release profile, npm
installation without Rust/Bun, unified protocol and onboarding, source revisions,
archive checksums and both installation receipts.

`Publish CLI packages` is a separate manual workflow on `main`. Supply the
successful `CLI packages` run ID and select one registry (`cargo` or `npm`).
It refuses an unsuccessful package run, another workflow or branch, and a source
revision whose latest public-release or Claude adapter CI has not passed. It
downloads that run's artifacts and checks the installation receipts against the
actual archive hashes. It does not rebuild the npm binary or repeat installation
verification. Cargo source is checked out at the verified revision and must
reproduce the reviewed packages before uploading.

Cargo publishes dependency-first and waits for registry indexing. The npm job
uses trusted publishing after first-publication setup. Selecting Cargo alone
allows the first Cargo release without prematurely attempting npm OIDC.
If publication fails, inspect registry state before continuing; published
versions are immutable. Never blindly rerun a partial release.
See [Cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).

No package is published by preparing or verifying artifacts. The owner declares
public release separately.

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
