# CLI package release

**Current release boundary:** the first public version is **`1.0.0-beta.1`**, a
semver pre-release on the npm `beta` channel; the
[21 October beta-1 candidate record](evidence/cli-installation/2026-10-21-beta-1-candidate/README.md)
describes the version, channel and remaining owner actions. The
[19 October remediation audit and full checklist](evidence/cli-installation/2026-10-19-release-remediation/README.md)
record the corrected source, qualified Cargo/npm artifacts, current security results,
real-client checks and reviewed screenshots; its engineering results still apply
and a fresh hosted package run is required for the beta-1 source. Publication
still requires the owner's registry credentials, repository visibility decision
and approval of the exact candidate. Recheck the security receipt's 24-hour
validity before uploading. The
[original audit](evidence/cli-installation/2026-10-19-release-audit/README.md)
is retained as the historical record of findings that prompted these fixes.

The first package target is Apple Silicon running macOS 15 or later. Clean
installation is verified on hosted macOS 15; the executable's lower Mach-O
linking target is not a runtime certification of older macOS versions. Cargo installs package
`magistr-cli` and exposes the `magistr` executable. npm package `magistr`
contains the same versioned native executable and supports global installation
and `npx magistr init`. Its bin entry points directly to the executable, preserving
stdio, signals, exit status and native worker startup. npm consumers need no
Rust, Bun, download hook or source checkout.

Desktop distribution and other operating systems are separate release work.
The verified Cargo/npm installation paths do not establish notarization or
Gatekeeper behavior for a browser-downloaded archive.

## Version management

The authoritative product version is `workspace.package.version` in the root
`Cargo.toml`. The first public version is `1.0.0-beta.1`; the beta line
(`1.0.0-beta.N`) precedes the stable `1.0.0`, which signals a stable public
API. Pre-release identifiers follow [Semantic Versioning](https://semver.org/):
a hyphen, then dot-separated identifiers (`1.0.0-beta.1`, never `1.0.0.beta-1`).
The version owner refuses any other spelling before writing.

The version also decides the **release channel**. `python3 scripts/version.py channel`
prints the npm dist-tag for the canonical version: `beta` for a pre-release,
`latest` for a stable version. Every publication consumer derives the tag from
the reviewed artifact version rather than hard-coding it: the publish workflow
passes it to `npm publish --tag`, the release-stage public gate verifies the
package on that tag, and the hosted npm verification serves the packed artifact
on the same tag. `latest` therefore stays reserved for `1.0.0`.

Cargo has no channels. `cargo install` avoids pre-release versions unless one is
named explicitly, so pre-release installation instructions must pin the exact
version (`cargo install magistr-cli@1.0.0-beta.1 --locked`). Internal registry
requirements are exact (`=1.0.0-beta.1`) and need no change.

Use one command for a release version change:

```sh
python3 scripts/version.py set 1.0.0-beta.2
python3 scripts/version.py check
```

All Rust workspace crates inherit the version. The command updates the generated
registry requirements, npm and Claude plugin manifests, the separate desktop
Cargo manifest, and both Cargo lockfiles. Tauri reads its Cargo version; the
website reads the generated npm version. Private website and desktop build tools
have no separate release version. Protocol versions, dependency versions and
historical verification receipts retain their own meanings.

After editing the canonical field directly, run `python3 scripts/version.py sync`.
Version changes need Cargo's dependencies available locally; the command uses the
existing lockfiles offline and does not upgrade external dependencies. CI and
package preparation refuse drift. Review and commit the generated changes before
preparing packages. `python3 scripts/version.py show` prints the current version.

## Prepare and verify

From the exact source revision intended for release on macOS arm64:

```sh
python3 scripts/distribution.py prepare --out target/cli-distribution
python3 scripts/distribution.py verify-cargo --out target/cli-distribution
python3 scripts/distribution.py verify-npm --out target/cli-distribution
python3 scripts/release-security.py scan --source "$(git rev-parse HEAD)" \
  --artifacts target/cli-distribution \
  --out target/cli-distribution/security-verification.json
python3 scripts/cli-release.py verify --out target/cli-distribution --source "$(git rev-parse HEAD)"
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
Review the inventories and actual verification JSON before releasing. Rust, Node
and Python versions come from `rust-toolchain.toml`, `.node-version` and
`.python-version`; release actions use reviewed full commit SHAs.

Security scanning checks the enabled native dependency graph, website and release
script dependencies, and repository history against the reviewed secret fixtures.
See [release security](release-security.md) for scanner setup, reviewed fixtures,
exceptions and local commands. The `security-verification.json` receipt must
pass, describe a clean source revision, match every archive hash, and be less
than 24 hours old.
An expired or dirty receipt cannot authorize publication. Refresh the hosted
package verification for that same source before continuing.

Release readiness runs the full Rust workspace and website checks on macOS arm64.
It fetches the complete locked dependency graph for offline architecture checks.
Desktop checks and host archive verification run concurrently; none waits for a
Windows or Linux Rust build. The Rust/website job is bounded to 30 minutes, with
its dependency cache reused on later runs. The multi-server owner scenarios reserve
CPU capacity for cold sandbox compilation; their response deadlines and concurrent
client/restart assertions remain unchanged.

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
reviewed=$(python3 scripts/cli-release.py verify --out target/cli-distribution --source REVIEWED_SOURCE_SHA)
npm_archive=$(printf '%s' "$reviewed" | python3 -c 'import json,sys; print(json.load(sys.stdin)["npm"])')
npm_tag=$(printf '%s' "$reviewed" | python3 -c 'import json,sys; print(json.load(sys.stdin)["npmTag"])')
./scripts/public-link-check.sh --stage publication
npm publish "target/cli-distribution/$npm_archive" --access public --tag "$npm_tag"
```

This command publishes the first npm package. Run it only after authorization.
The dist-tag comes from the verified artifact (`beta` for `1.0.0-beta.1`), so a
pre-release never lands on `latest`; a wrongly tagged upload is corrected with
`npm dist-tag`, never by unpublishing.
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

## Verify deployed launch information

The public gate checks the rendered installation, agent setup, upgrade and
uninstall guides for their current commands and the planning boundary, as well
as anonymous reachability. HTTP 200 alone is insufficient: an old guide or a
redirect to an unrelated page fails.

Use the stage that matches the work being checked:

```sh
# Before publication, while repository/package access may still be private:
./scripts/public-link-check.sh --stage website --out target/website-readiness.json

# Before an authorized upload; public repository, support and private reporting must work:
./scripts/public-link-check.sh --stage publication --out target/publication-readiness.json

# After uploads and the installation guide's publication update:
./scripts/public-link-check.sh --stage release --out target/release-readiness.json
```

The website stage accepts truthful unpublished-package instructions and does not
probe private repository or registry availability. Use `--site http://127.0.0.1:PORT`
to inspect a local preview through the same content checks. The publication stage
adds anonymous repository/releases/issues/security-policy checks, verifies that
GitHub private vulnerability reporting is enabled, and checks that its sign-in
returns to the report form. It does not require packages to exist yet.
[GitHub reporting API](https://docs.github.com/en/rest/repos/repos#check-if-private-vulnerability-reporting-is-enabled-for-a-repository)

The release stage also requires the current Cargo version, the current npm
version on its channel dist-tag (`beta` for a pre-release, `latest` for a
stable version), and removal of the unpublished-package notice. The publisher
runs the publication stage before uploads. The **Public endpoints** workflow
checks the deployed website stage weekly and runs the release stage on manual
and tag runs, failing when any required check fails. The CI deploy job also
runs the website stage against the live site after every deployment from
`main` (the `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository
secrets exist since 2026-10-22). The website stage also runs locally with
`./scripts/public-link-check.sh --stage website` after changing the manual.
No stage changes repository visibility, enables reporting or publishes
packages; the website deploys only from the CI deploy job on `main`.

## Publish reviewed artifacts

`CLI packages` only prepares, verifies and uploads `cli-distribution` on hosted
macOS arm64. It checks extracted Cargo installation in the release profile, npm
installation without Rust/Bun, unified protocol and onboarding, source revisions,
archive checksums and both installation receipts.

`Publish CLI packages` is a separate manual workflow on `main`. Supply the
successful `CLI packages` run ID and select one registry: `cargo` (token,
required while a crate does not exist yet), `cargo-trusted` (crates.io Trusted
Publishing for crates that already exist and are configured for this workflow)
or `npm`. It refuses an unsuccessful package run, another workflow or branch,
and a source revision whose latest CI or Security push run has not passed. Every
downloaded archive must carry the build provenance attestation that `CLI
packages` created with `actions/attest-build-provenance`; `gh attestation verify`
checks each one before anything is uploaded. It downloads that run's artifacts and checks installation and current
security receipts against every archive hash. The publisher uses the current
reviewed release scripts from main, with no source checkout change, package
rebuild or repeated installation test.

Cargo publication uploads the exact reviewed `.crate` bytes through the
[documented registry API](https://doc.rust-lang.org/cargo/reference/registry-web-api.html).
Metadata comes from each archive's normalized manifest, including renamed,
target-specific, build and development dependencies. Internal dependencies must
have the reviewed exact version and appear earlier in the publication order.

Before any upload, the publisher checks all names and versions for known
conflicts. An already published version is accepted only when its API checksum,
index checksum and downloaded archive match the reviewed archive and it is not
yanked. A mismatch stops the entire batch before additional effects. Each newly
uploaded dependency must appear in the registry index and pass the downloaded
archive check before publication continues.

An explicit HTTP 429 permits a bounded retry after `Retry-After`; the registry
is checked again before retrying. Both seconds and HTTP-date headers are
supported. Missing headers use a ten-minute wait. A lost response, timeout or
server error triggers only read-only reconciliation, never another upload.
Completed and uncertain effects are saved in `cargo-publication.json` before
each next effect, and retained as the workflow's `cargo-publication` artifact.

First publication of fourteen new crates can take about ninety minutes under
crates.io's default quota of five initially and one more every ten minutes.
Account overrides can differ. This is registry waiting, not a build: the
publisher performs no Rust compilation and bounds waiting to two hours.
[Current registry quota implementation](https://github.com/rust-lang/crates.io/blob/6cf63268f7fac38f8d255bbb22497c8ecff35d57/src/rate_limiter.rs)

The npm job uses trusted publishing after first-publication setup and passes
the dist-tag reported by the artifact verification. Selecting Cargo alone
allows the first Cargo release without attempting npm OIDC.

### Continue an interrupted Cargo publication

1. Download and inspect the stopped run's `cargo-publication` artifact. Keep
   its reviewed package run, source revision and hashes unchanged.
2. Dispatch **Publish CLI packages** with the same `package_run`, registry
   `cargo`, and `resume_run` set to the stopped publication run's ID.
   Use a fresh hosted package run for the same source if its security receipt
   expired; the reviewed archive hashes must remain identical.
3. The publisher restores the previous receipt, rejects a different source or
   hash set, and reconciles registry state again. Completed versions are never
   re-uploaded. An earlier rate-limit deadline is still honored.
4. An uncertain upload is resolved only by observing that exact version in the
   registry. If it remains unresolved, the resumed run stops without replaying
   it. Inspect the registry and workflow logs and obtain explicit approval for
   any new attempt; do not discard the receipt to bypass this guard.

For an authorized local publication of verified artifacts, the same owner is
available as an explicit command:

```sh
python3 scripts/cli-release.py publish-cargo --out target/cli-distribution \
  --source REVIEWED_SOURCE_SHA --max-wait 7200
```

It reads `CARGO_REGISTRY_TOKEN` from the environment. Never place the credential
in command arguments or chat. Preserve `cargo-publication.json` in that output
directory when continuing locally.

If the runner is lost before its receipt artifact can be retained, treat the
last upload as uncertain and inspect registry state before authorizing another
run. Do not infer failure from a missing response.

### Correct an incorrect published version

Stop further uploads and preserve the reviewed artifacts and publication receipt.
Published versions cannot be overwritten. Identify affected packages and their
dependents, record the defect, and prepare a corrected version with
`scripts/version.py set`. Review and verify its full affected package set before
authorizing another upload. Yanking a Cargo version or deprecating an npm version
is a separate, reviewed owner action; neither is an automatic rollback. Do not
unpublish a package expecting to reuse its version.
[Cargo yank](https://doc.rust-lang.org/cargo/commands/cargo-yank.html),
[npm unpublish policy](https://docs.npmjs.com/policies/unpublish/)

No package is published by preparing or verifying artifacts. The owner declares
public release separately.

### Tag and announce

After both registry uploads pass their real-registry installation and hash
checks, and the installation guide no longer says packages are unpublished,
create the annotated tag `v1.0.0-beta.1` on the reviewed source and a GitHub
release marked **pre-release** with the candidate's release notes and checksums.
Pushing a `v*` tag runs the release-stage public gate, which must pass. Do not
tag before the uploads: the gate would fail on the missing registry versions.

## Install, upgrade, repair and remove

After publication of the beta:

```sh
cargo install magistr-cli@1.0.0-beta.1 --locked
# or:
npm install --global magistr@beta

magistr init
magistr doctor
magistr repair --dry-run
magistr repair --yes
magistr remove --dry-run
magistr remove --yes
```

The Cargo command names the exact version because `cargo install` skips
pre-releases otherwise. The npm `beta` tag resolves to the current beta.
Stable releases use `cargo install magistr-cli --locked` and
`npm install --global magistr` once `1.0.0` is published on `latest`.

Upgrade with `cargo install magistr-cli@<version> --locked --force` or
`npm install --global magistr@beta` (stable: `magistr@latest`), then explicitly
run `magistr repair` in each project. npx-generated configurations pin the package version; run the
new version's setup/repair to update them. Existing sessions keep their admitted
owners until closed; reopen clients to use a new installation.

Removal deletes unchanged owned integration files and entries while keeping
customizations, other servers and project workflow state. Uninstall the executable
separately with `cargo uninstall magistr-cli` or `npm uninstall --global magistr`.
