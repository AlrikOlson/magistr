# 1.0.0-beta.1 candidate preparation — 21 October 2026

**Status: prepared, not published.** This record changes the release identity
from the 19 October `0.1.0` candidate to **`1.0.0-beta.1`** on the npm **`beta`**
channel. No registry upload, public tag or public-release declaration occurred.
The [19 October remediation audit](../2026-10-19-release-remediation/README.md)
remains the engineering baseline: its 56 PASS results describe the same source
tree apart from the changes listed below, and its OWNER, LIMITED and PENDING
items still stand. A fresh hosted **CLI packages** run on the beta-1 source is
required before publication; the 19 October artifacts carry version `0.1.0` and
cannot be published as the beta.

## What changed

| Item | 19 October candidate | Beta-1 candidate |
| --- | --- | --- |
| Product version | `0.1.0` | `1.0.0-beta.1` (`python3 scripts/version.py set 1.0.0-beta.1`; `check` passes) |
| npm dist-tag | `latest` (implicit) | `beta`, derived from the version by `scripts/version.py npm_dist_tag`; `latest` is reserved for `1.0.0` |
| Publish workflow | `npm publish --access public` | `npm publish --access public --tag <npmTag>` with the tag reported by `cli-release.py verify` |
| Release-stage public gate | checks `registry.npmjs.org/magistr/latest` | checks `/magistr/<tag>` (`beta` for a pre-release) and reports `npmTag` |
| Hosted npm verification | loopback registry served `dist-tags: {latest}` | serves the same dist-tag the real registry will carry |
| Cargo install instruction | `cargo install magistr-cli --locked` | `cargo install magistr-cli@1.0.0-beta.1 --locked` |
| Git tag | `v0.1.0` | `v1.0.0-beta.1`, GitHub release marked pre-release |

The generated version consumers (`Cargo.toml` registry requirements, both
lockfiles, the desktop manifest, the npm and Claude plugin manifests) changed
only in their product version. The website renders the version from the npm
manifest and now shows `v1.0.0-beta.1` while still truthfully stating that
registry packages are not published.

## Why a pre-release needs these changes

- **Semver spelling.** `1.0.0.beta-1` is not a semantic version; Cargo rejects it
  and the version owner refuses it before writing. The valid form is
  `1.0.0-beta.1`, which sorts below `1.0.0`.
- **Cargo.** The Cargo reference states that `cargo install` avoids pre-release
  versions unless explicitly asked. `cargo install magistr-cli` would fail with
  no stable version published, so every install instruction pins the version.
  Internal dependencies already use exact requirements (`=1.0.0-beta.1`).
- **npm.** `npm publish` tags `latest` unless told otherwise, and `npm install
  magistr` resolves `latest`. Publishing the beta on `beta` keeps `latest` for the
  first stable release and makes the beta an explicit opt-in
  (`npm install -g magistr@beta`, `npx magistr@beta init`). Generated MCP
  configurations pin `magistr@<exact version>` and are unaffected by the tag.

## Owner actions, in order

1. Account readiness, unchanged from the audit: crates.io token able to create new
   crates (`CARGO_REGISTRY_TOKEN` in environment `cli-release`, items C07),
   npm login with two-factor authentication (N07), repository visibility and
   private vulnerability reporting (S10, W03, W04).
2. Dispatch **CLI packages** on the main commit that carries this preparation.
   Review its inventories, installation receipts and security receipt (24-hour
   validity) for version `1.0.0-beta.1`.
3. Authorize publication of that exact run (P01): **Publish CLI packages** with
   registry `cargo`, then the first manual npm upload with `--tag beta` as
   documented in [the release procedure](../../../releasing-cli.md), then the
   trusted-publisher configuration.
4. Post-publication checks (P02–P04): real-registry Cargo install of
   `magistr-cli@1.0.0-beta.1`, npm global and npx install of `magistr@beta`,
   registry archive hashes against the reviewed inventories.
5. Publication-day website update: replace the source-preview installation
   instructions with the registry commands above and remove the
   "registry packages are not published yet" notice; the release-stage public
   gate requires that removal and the registry versions on their channel.
6. Create the annotated tag `v1.0.0-beta.1` and a GitHub release marked
   pre-release using [RELEASE-NOTES.md](RELEASE-NOTES.md); the tag push runs the
   release-stage gate. Declare the public release only after it passes (P06).

The repository's [pre-release policy](../../../../AGENTS.md#before-public-release)
stays in force for the beta line.

## History rewrite before the repository became public (22 October 2026)

With the owner's authorization, the repository history was rewritten once so a
public clone is small: `docs/evidence/desktop-iterations` (89 dated iterations,
881 MB) moved to the [magistr-evidence](https://github.com/AlrikOlson/magistr-evidence)
repository with its own history and is mounted back as a submodule at the same
path, and media blobs that no current file uses (deleted screenshots, goldens
and fonts; 8,136 blobs, 1.25 GB uncompressed) were dropped from every commit.
Source and document history is complete; the pack went from 940 MB to 78 MB.

Commit hashes before the rewrite therefore no longer resolve in this repository.
[history-rewrite-commit-map.txt](history-rewrite-commit-map.txt) maps every old
commit to its rewritten commit. The identities recorded by the October audits:

| Record | Before | After |
| --- | --- | --- |
| 19 October release-ready candidate source | `4fa3788f200c5c807f142f7542e6c159deae777a` | `57c258a8` |
| 19 October remediation candidate source | `1e99aba7b9e3d9f76c953ddf03041166f45aa599` | `a66d7e9e` |
| Unchanged adapter contracts source | `83a98358` | `12455cc4` |
| Archived September release tag target | `3723c626bceb71679019029fb9f13feae92a583b` | `30507646` |
| Beta-1 preparation commit | `1cfe3a68` | `93bf0b3e` |

The reviewed secret fixtures in `scripts/release-security-policy.json` were
remapped through the same map (24 entries, none lost). Hosted workflow run ids
in the audits still identify their runs; the artifacts they produced were built
from the pre-rewrite commits and are byte-identical to what the mapped commits
contain. A complete pre-rewrite bundle of every ref is kept offline by the owner
(`magistr-pre-rewrite-20261021.bundle`).
