# Initial release audit — 19 October 2026

**Release verdict: HOLD.** The Cargo/npm artifacts install and the functional
suite passes, but this candidate is not ready for public release. Current
dependency advisories, incomplete first-publication recovery and inconsistent
release information require work. The earlier
[release-candidate record](../2026-10-19-release-ready/README.md) establishes
functional and packaging readiness only; this audit supersedes its implied
conclusion that only account setup remained.

This review covers the first Apple Silicon macOS CLI release. Desktop
distribution, Intel Macs, Linux and Windows remain outside acceptance. Nothing
was published, repository visibility was not changed, and release metadata was
not altered.

The checklist contains **73 checks: 41 PASS, 8 BLOCKED, 7 OWNER, 10 LIMITED,
2 WARN and 5 PENDING**. These are checks, not independent defect counts:
several blocked rows share an owning fix.

## Candidate and evidence

| Item | Audited identity |
| --- | --- |
| Product version | `0.1.0` |
| Candidate source | `4fa3788f200c5c807f142f7542e6c159deae777a` |
| Audit starting HEAD | `4a2abaef3fd34635bcd28691756d71c2415d205b`, clean `main`, matching origin; added evidence only |
| Hosted packages | [Run 39352947283](https://github.com/AlrikOlson/magistr/actions/runs/39352947283) |
| Full quality checks | [Run 39352938183](https://github.com/AlrikOlson/magistr/actions/runs/39352938183) |
| Artifact | `12234132955`; expires 2027-01-18 |
| Artifact ZIP digest | `7afc502338df94fc1cca7164fa99f3d6a72a2dd3fe3d3352a9e63fb3fceada81` |
| npm tarball SHA-256 | `9bd932fbf869c4c1c9be9aecae0bad6a05c707db0633ac7dfbe0351afaa17e15` |
| Binary SHA-256 | `f663e784c973c09fe5f468af352cad680cf80b4adccb2df112e75c90f593ded6` |

[Machine-readable audit](audit.json) retains the complete checklist, live probes,
normalized Cargo manifests, exact skipped tests and prior hosted results.
[Dependency review](dependency-review.json) contains advisory IDs, affected
versions and reported fixed ranges.
[Secret-scan review](secret-scan-review.json) records all 21 reviewed matches
without credential strings.
[Candidate inventories and receipts](../2026-10-19-release-ready/README.md)
retain the package files and installation evidence.

New checks in this audit include current Cargo/npm advisory scans, redacted
Git-history scanning, a Rust 1.95 build check, normalized archive inspection,
963 license notice hash checks, Mach-O/signature inspection, registry and
repository administration probes, and deployed-page inspection. Successful
unchanged-source hosted tests were reviewed rather than rerun.

## Findings that prevent release

**1. Clear native dependency security findings.**

The RustSec database contained 1,338 advisories at commit
`999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b`, updated 17 October.
The scan at 20 October 01:49 UTC found **38 package/advisory matches**.
The actual macOS CLI normal/build dependency graph includes **33** of them;
the other five concern QUIC dependencies disabled in this CLI build.

| Dependency | Locked version | Matches | Enabled in CLI | Reported patch baseline covering this set |
| --- | --- | ---: | --- | --- |
| Wasmtime | 48.0.2 | 16 | Yes | 48.0.6 |
| Wasmtime WASI | 48.0.2 | 6 | Yes | 48.0.6 |
| Wasmtime, grammar engine | 36.0.15 | 2 | Yes | 36.0.18 |
| rustls | 0.23.37 | 4 | Yes | 0.23.46 |
| rustls-webpki | 0.103.9 | 4 | Yes | 0.103.13 |
| RSA | 0.9.10 | 1 | Yes | No patch listed; assess usage |
| quinn-proto | 0.11.14 | 5 | No | 0.11.18 if enabled |

A matching version is not proof of an exploitable product path. However, the
sandbox uses Wasmtime 48.0.2 with fuel accounting and default Cranelift, which
overlaps the configuration affected by the fuel-amplification advisory.
Actual triggering through Magistr's inputs was not demonstrated.
Patch the runtime and examine proposal/call-path conditions rather than treating
functional sandbox tests as security clearance.
See the [upstream fuel advisory](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-m63x-6p34-q65x)
and [sandbox configuration](../../../../crates/magistr-workspace/src/sandbox.rs).

There are six additional warning records: unmaintained unsafe-libyaml, unsound
event-listener, libyml, two rand versions and serde_yml. The current
[roadmap YAML store](../../../../crates/magistr-core/src/roadmap/yaml_store.rs)
uses serde_yml/libyml in production. Replace their owning serialization
dependency with a maintained implementation and verify behavior.
RustSec lists no patched serde_yml release.
[serde_yml advisory](https://rustsec.org/advisories/RUSTSEC-2025-0068.html)

Two important qualifications prevent overstating the findings:

- The current WASI runner grants no preopened directories; the macOS WASI
  symlink issue therefore does not demonstrate a host-filesystem escape here.
  [WASI advisory](https://rustsec.org/advisories/RUSTSEC-2026-0352.html)
- Production RSA usage found in authentication verifies RS256 signatures.
  Private-key RSA decryption was not found. The timing advisory needs a
  documented applicability decision, not an impossible instruction to install a
  nonexistent patch. [RSA advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)

The dependency fix invalidates this candidate's release approval. Build and
verify new artifacts afterward, even if the first public version stays 0.1.0.

**2. Make Cargo publication safely resumable.**

The [publisher](../../../../.github/workflows/cli-publish.yml) loops over fourteen
new crates and stops at the first error. It does not reconcile already-published
versions against archive checksums or handle rate limits.

The current crates.io implementation defaults to an initial burst of **five new
crates**, replenishing **one every ten minutes**; deployments and accounts can
override these values. We did not establish this account's actual quota.
The fourteen-crate first release therefore has a credible partial-upload risk.
[crates.io rate limiter, pinned source](https://github.com/rust-lang/crates.io/blob/6cf63268f7fac38f8d255bbb22497c8ecff35d57/src/rate_limiter.rs)

The publisher should derive progress from registry name/version/checksum state,
skip only verified completed uploads, stop on mismatches, respect retry timing,
and resume in dependency order. Test this against a controlled registry fixture.
The existing instruction to inspect failures does not implement recovery.
Published Cargo versions are immutable.
[Cargo publication rules](https://doc.rust-lang.org/cargo/reference/publishing.html)

**3. Require security evidence in the release decision.**

The current workflows do not run Cargo/npm advisory or Git-history secret scans,
and the publish selector does not require them. This is why a passing release
build did not expose the dependency findings. Add current, source-bound security
results with narrowly reviewed exceptions to publication selection.

All **21 secret-scanner matches** were reviewed in current or historical source:
synthetic redaction fixtures, source-content hashes and schema example digests.
No live credential was demonstrated. Keep that context in a narrow scanner
configuration rather than ignoring whole directories.

Publication actions still use mutable tags and a floating Rust toolchain.
Pin reviewed action commits and packaging compiler versions. The release
environment correctly permits only `main`, but `main` is currently unprotected
and there are no repository rulesets. Configure appropriate owner controls
before adding publication credentials.
[GitHub Actions security guidance](https://docs.github.com/en/actions/reference/security/secure-use)

**4. Reconcile the private release identity before making it public.**

There is already a **private GitHub release named v0.1.0**, created
9 September 2026. Its tag points to
`3723c626bceb71679019029fb9f13feae92a583b`, not the reviewed candidate.
It is neither a draft nor a prerelease. Its old arm64 archive has SHA-256
`31d24a50169ea39ce775577451d076bf0360d251fc538243a96ebd042265a99b`.

No npm or Cargo registry package was found under the proposed fifteen names.
The private historical release does not establish an official public release
and introduces no backward-compatibility requirement. Keep `0.1.0` as the
first public version; explicitly reconcile or retire the stale private tag,
release body and assets before a visibility change. Deleting/replacing them
requires the owner's authorization.

**5. Bring the deployed website and support routes up to date.**

The [live installation guide](https://magistr.dev/docs/installation/) returns
200 but incorrectly says Claude Code setup is always written. Current setup
writes selected integrations. Its source-install-only guidance must also be
updated to reflect actual registry availability at launch. The other listed
agent integrations remain supported; their names alone are not defects.

The website dependency audit reports **17 affected packages: 1 critical,
12 high, 1 moderate and 3 low**. The critical Astro advisory concerns processing
untrusted AVIF images. This site is built as static output; the scan is not
evidence that the deployed site offers a public remote-code-execution path.
Upgrade and review the build/development exposure, then build and visually
verify the resulting site.
[Astro advisory](https://github.com/withastro/astro/security/advisories/GHSA-26w7-cxv4-gfx2)

Anonymous repository, releases, issues and security links all return 404 while
the repository is private. The existing public-link job is skipped on push and
uses `continue-on-error`; it also checks status rather than current page content.
Require the real launch checks before announcement.

Enable and verify the private reporting route described by
[SECURITY.md](../../../../SECURITY.md), or provide a concrete private contact.
The current audit could not establish a usable reporting route.
[GitHub private reporting setup](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository)

## Full checklist

**PASS** means the stated check has evidence, not that every related behavior
has been proven. **BLOCKED** must be corrected before release. **OWNER** needs
account setup or an owner-controlled action. **LIMITED** identifies a narrower
claim or missing evidence. **WARN** is a recorded improvement. **PENDING**
requires an actual publication or a later release step.

| ID | Area | Result | Check | Evidence or remaining action |
| --- | --- | --- | --- | --- |
| I01 | Identity | **PASS** | First-release scope is Apple Silicon macOS CLI | Cargo/npm scope is explicit; desktop, Intel Macs, Linux and Windows are outside acceptance. |
| I02 | Identity | **PASS** | One product version owner | Cargo.toml workspace.package.version is 0.1.0; scripts/version.py updates generated manifests and lockfiles; six version tests passed. |
| I03 | Identity | **PASS** | Audited candidate is identified and tree is clean | Artifacts identify source 4fa3788f; audit began on clean main at 4a2abaef, whose additional changes are evidence only. |
| I04 | Identity | **BLOCKED** | Public tag, release notes and assets identify the candidate | Private v0.1.0 release/tag still identifies 3723c626 from September. Reconcile it before visibility changes. |
| I05 | Identity | **PASS** | Requested registry names are currently unused | All 14 Cargo names and npm magistr returned 404. This neither reserves names nor proves publishing access. |
| C01 | Cargo | **PASS** | All internal dependencies can be published | Fourteen normalized archives have matching 0.1.0 registry requirements and no remaining internal path dependency; package licenses retained. |
| C02 | Cargo | **PASS** | Required assets are in extracted packages | Isolated release-profile Cargo installation and MCP/workspace smoke checks passed without a checkout. |
| C03 | Cargo | **PASS** | Cargo package source matches reviewed source | All fourteen archives reproduced byte for byte with Cargo 1.99.1; inventories and checksums retained. |
| C04 | Cargo | **PASS** | Advertised Rust minimum builds the CLI | Fresh cargo +1.95.0 check --locked -p magistr-cli --bin magistr passed; this is a check, not a full minimum-toolchain test suite. |
| C05 | Cargo | **PASS** | Internal publication order follows dependencies | Fourteen-package publication-order.txt is derived from the dependency closure. |
| C06 | Cargo | **BLOCKED** | First publication tolerates quotas and partial failure | Current loop has no rate-limit handling or registry/checksum reconciliation for safe resumption. |
| C07 | Cargo | **OWNER** | Account email and a suitable token are ready | CARGO_REGISTRY_TOKEN is absent from cli-release; owner must verify account email and authorize creation of new crates. |
| N01 | npm | **PASS** | Packed package contains the versioned native CLI | magistr@0.1.0, macOS arm64 restriction, direct native bin, no runtime JavaScript dependencies or install hooks. |
| N02 | npm | **PASS** | Global installation needs no Rust, Bun or checkout | Hosted packed-artifact install passed in an isolated prefix with those tools absent from PATH. |
| N03 | npm | **PASS** | npx works from a fresh cache | Loopback registry and fresh-cache test passed, including the generated version-pinned MCP configuration. |
| N04 | npm | **PASS** | Stdio, exit codes, signals and worker startup survive installation | Actual packed binary checks passed; there is no wrapper process to lose native signals. |
| N05 | npm | **PASS** | Package file inventory is intentional | 971 files; no symlinks or unexpected paths; executable mode 0755. Compressed size 31,876,176 bytes. |
| N06 | npm | **PASS** | Uninstall works | Packed npm uninstall and executable removal passed. |
| N07 | npm | **OWNER** | Publishing account and interactive 2FA are ready | npm whoami returned ENEEDAUTH. Owner must sign in and complete first-publication authentication. |
| N08 | npm | **OWNER** | Subsequent trusted publisher is configured | No package exists yet. Configure the exact repository/workflow/environment and allow-publish after first creation; verify current expiry rules. |
| M01 | macOS | **PASS** | Executable is arm64 and uses expected system libraries | Mach-O arm64 inspection passed; no non-system dylib dependency found; strict ad-hoc code-signature verification passed. |
| M02 | macOS | **LIMITED** | Minimum operating-system claim is tested | Binary targets macOS 11.0; hosted clean install is proven on macOS 15.7.9. macOS 11–14 were not tested; target metadata is not runtime certification. |
| M03 | macOS | **LIMITED** | Browser-downloaded archive passes Gatekeeper | No Developer ID/notarization proof. Cargo/npm installation is proven; do not advertise a notarized direct download. |
| U01 | Unified MCP | **PASS** | One public MCP connection owns workflow and workspace access | magistr mcp --dir is used by current integrations; workflow/project and workspace/working-copy ownership remain distinct internally. |
| U02 | Unified MCP | **PASS** | All capabilities have catalog coverage | One typed catalog covers direct common tools plus discover/execute and advanced workspace/workflow operations; schema/handler coverage passed. |
| U03 | Unified MCP | **PASS** | Composition keeps authorization and bounded execution | Native operation checks, sandbox limits, sequential mutation and stop-on-failure receipts passed functional checks; dependency security remains separately blocked. |
| U04 | Unified MCP | **PASS** | Edits are verified, journaled and reject stale requests | Source/hash checks, confirmation behavior and session-scoped undo are covered in retained tests. |
| U05 | Unified MCP | **PASS** | Concurrent agents and worktrees keep correct owners | Simultaneous clients, separate worktrees and owner restart passed in the eight process scenarios. |
| U06 | Unified MCP | **PASS** | Cancellation and partial execution are recoverable | Cancellation, stale confirmations and partial failure tests passed; completed effects remain visible without automatic replay. |
| U07 | Unified MCP | **PASS** | Response failures and truncation are explicit | Typed envelope/catalog checks passed; discovery needs no client-specific extension. |
| U08 | Unified MCP | **WARN** | Current fixtures use current state names | Four tas-data references remain across three Rust test files. No production legacy endpoint or required directory found in the scoped search. |
| O01 | Onboarding | **PASS** | Setup detects the project and proposes changes | Guided interview and administrative plan service are present; configuration and verification commands are reviewed before application. |
| O02 | Onboarding | **PASS** | Only selected integrations are generated | Codex, Claude Code and portable setup are supported; selected-agent and multi-agent process cases passed. |
| O03 | Onboarding | **PASS** | Repeated setup preserves custom settings and other servers | Customized-config, repeated setup, paths with spaces, repair and removal cases passed. |
| O04 | Onboarding | **PASS** | Unavailable clients produce actionable diagnostics | Unavailable-agent checks and doctor behavior passed in the six process scenarios. |
| O05 | Onboarding | **PASS** | Setup finishes with a real connection and read-only context | Protocol handshake/context checks passed; setup suggests planning and does not start implementation. |
| O06 | Onboarding | **PASS** | MCP startup does not automatically repair project files | Repair belongs to explicit setup/repair through the administrative change service. |
| O07 | Onboarding | **PASS** | Upgrade, repair and removal are documented | Current release procedure documents ownership-preserving repair/removal and reopening installed clients. |
| O08 | Onboarding | **LIMITED** | Human interactive onboarding is demonstrated | Process tests and interview unit tests exist; no new human TTY walkthrough was performed in this audit. |
| E01 | Agent evaluations | **LIMITED** | Codex and Claude Code were exercised on realistic workflows | Recorded small trials cover investigation, edits, planning, feedback, reasoning, verification and resumption; they predate final artifact hashing. |
| E02 | Agent evaluations | **LIMITED** | Efficiency claims reflect measured evidence | Small comparisons record success/calls/context; Claude unified timing was worse. No broad performance or reliability claim is justified. |
| T01 | CI | **PASS** | Exact candidate passes full Rust checks | 8,065 tests passed, 18 skipped, no failures; all-targets check and Clippy passed in hosted run 39352938183. |
| T02 | CI | **PASS** | Protocol and onboarding processes pass on release packaging | Eight unified MCP scenarios and six onboarding scenarios passed in run 39352947283. |
| T03 | CI | **PASS** | Release does not wait for Windows Rust builds | Rust release gates run on macOS; Windows desktop requires manual opt-in. Remaining Windows adapter coverage is JavaScript only. |
| T04 | CI | **PASS** | CI duration is bounded and measured | Rust/website job took 901 seconds and has a 30-minute limit; independent packaging/desktop jobs ran concurrently. |
| T05 | CI | **LIMITED** | Skipped live scenarios are accounted for | 18 skip records retained. Live Codex/Claude, cloud/tracker/keychain and reference checks are not covered merely by the full-suite pass. |
| T06 | CI | **PASS** | Reviewed artifacts are tied to checks and checksums | Publication selector requires the intended successful package run/source and matching installation receipts; source archive reproduction passed. |
| T07 | CI | **PASS** | Artifacts are retained long enough for review | Hosted artifact 12234132955 expires 2027-01-18; local copy and tracked inventories exist. Rebuild after dependency fixes. |
| S01 | Security | **BLOCKED** | Enabled native dependencies have no unresolved applicable advisories | 38 lockfile matches, 33 in enabled CLI dependencies; six warnings. Wasmtime/TLS/YAML work remains. |
| S02 | Security | **BLOCKED** | Sandbox runtime is on an acceptable security baseline | Wasmtime 48.0.2 uses fuel accounting affected by an advisory. Other findings require proposal/call-path triage. |
| S03 | Security | **BLOCKED** | Website dependencies have a reviewed security baseline | 17 affected packages: 1 critical, 12 high, 1 moderate, 3 low. Static output limits exposure but does not clear build-time findings. |
| S04 | Security | **PASS** | Release-script JavaScript dependencies pass current audit | scripts npm audit reported zero findings. |
| S05 | Security | **PASS** | Git-history secret matches are reviewed | 21 gitleaks matches reviewed in source context: synthetic redaction fixtures, source hashes and schema digests; no live credential demonstrated. |
| S06 | Security | **PASS** | Embedded JavaScript engine has provenance | QuickJS-ng 0.16.2 WASI hash matches the official release asset; reviewed upstream advisories do not establish an unfixed matching issue. |
| S07 | Security | **LIMITED** | Prebuilt embedded engine is reproducible from source | Upstream asset/source/license provenance is retained; original toolchain inputs are insufficient for an independent byte-for-byte rebuild. |
| S08 | Security | **BLOCKED** | Security checks are enforced before publication | Current release workflows omit dependency/history scans and the publication selector does not require their results. |
| S09 | Supply chain | **WARN** | Action and compiler inputs are immutable | Publication still uses action tags and Rust stable. Pin reviewed action SHAs and packaging toolchain before adding credentials. |
| S10 | Supply chain | **OWNER** | Main and release environment have appropriate protection | cli-release accepts only main, but main is unprotected and repository rulesets are empty; reviewers require owner configuration. |
| S11 | Supply chain | **LIMITED** | Package provenance is accurately described | Checksums/source receipts exist. First interactive npm upload has no automatic OIDC provenance; later public-repository trusted uploads can have it. |
| L01 | Legal and privacy | **PASS** | Package license notices match their inventory | 550 dependency records and 963 notice hashes checked; required upstream attribution retained. This is a technical inventory review. |
| L02 | Legal and privacy | **LIMITED** | Data and permission behavior is fully observed | Code defaults telemetry consent off and requires an endpoint; local records/provider permissions are documented. No packet-capture audit of every optional integration was performed. |
| W01 | Public launch | **PASS** | Public website is reachable | Homepage and installation guide return HTTP 200. |
| W02 | Public launch | **BLOCKED** | Deployed instructions match current setup | Installation guide incorrectly says Claude Code setup is always written; source-install instructions must be updated for actual registry availability at launch. |
| W03 | Public launch | **OWNER** | Repository and support links are public | Repository/releases/issues/security URLs return anonymous 404 while repository is private. Owner visibility decision is outstanding. |
| W04 | Public launch | **OWNER** | Private vulnerability reporting works | Current API/page check could not establish a usable route. Enable and verify private reporting or document a concrete private contact. |
| W05 | Public launch | **BLOCKED** | Launch checks verify deployed content and required links | Current public endpoint job is skipped on pushes and continue-on-error; status checks do not catch outdated page content. |
| W06 | Public launch | **LIMITED** | Visual evidence describes what is actually deployed | Local version-display captures exist; they do not prove the current public website deployment. Any website fix needs fresh saved visual review. |
| P01 | Publication | **OWNER** | Owner approves exact corrected artifacts and public launch | No publication authorization for this audit; candidate must be rebuilt after blocker fixes and approved by run ID, version and checksums. |
| P02 | Publication | **PENDING** | Cargo installs successfully from the real registry | Run after publication using an isolated Cargo home/root and --locked; unpublished names cannot be installed from crates.io now. |
| P03 | Publication | **PENDING** | npm global and npx install successfully from the real registry | After publication, verify integrity and exact version with fresh cache/prefix and a disposable project. |
| P04 | Publication | **PENDING** | Published archives match reviewed hashes and metadata | After upload, fetch registry archives and compare all 14 Cargo checksums, npm integrity/version/bin metadata and release assets. |
| P05 | Publication | **PENDING** | Support and correction procedure is ready for a bad upload | Document safe partial-publication recovery before upload; after an immutable bad version, use reviewed deprecation/yank policy and a corrected version. |
| P06 | Publication | **PENDING** | Public release is explicitly declared | Only after publication checks, deployed docs and support routes pass; the private historical GitHub release is not this declaration. |

## What the positive evidence establishes

The exact candidate passed **8,065 Rust tests with 18 skipped and no failures**,
all-targets checking and Clippy. The hosted macOS Rust/website job took
**901 seconds**, within its thirty-minute limit. Release verification does not
wait for a Windows Rust build; the retained Windows adapter job runs JavaScript.
[Hosted quality run](https://github.com/AlrikOlson/magistr/actions/runs/39352938183)

All fourteen Cargo packages installed after extraction without a checkout.
The packed npm artifact passed global install, fresh-cache npx, pinned MCP setup,
stdio, signals, exit-code, native-worker and removal checks without Rust or Bun.
Eight unified-MCP and six onboarding process scenarios passed.
[Hosted package run](https://github.com/AlrikOlson/magistr/actions/runs/39352947283)

The fresh minimum-toolchain check passed:
`cargo +1.95.0 check --locked -p magistr-cli --bin magistr`.
Its native receipt is `f6d94827-9875-41c7-99d6-e7be65c7b820`.
The failing dependency/secret scan receipt is
`53db3db5-eb11-4ab3-821d-49e8b374ffe0`; the subsequent secret review explains
those matches but does not clear dependency failures.

The archive review checked 550 dependency records and 963 license notice hashes.
The embedded QuickJS-ng 0.16.2 WASI digest matches its official asset:
`d2939e98c808e8b9f4164cd0d7b0398cbc0121ddf52862bcd92157d923e461cc`.
The upstream advisory review did not establish an unfixed matching QuickJS issue.
The original build inputs are insufficient for an independent byte-for-byte
engine rebuild.
[QuickJS release](https://github.com/quickjs-ng/quickjs/releases/tag/v0.16.2),
[upstream advisories](https://github.com/quickjs-ng/quickjs/security/advisories)

## Limits that remain visible

The skipped tests include live Codex/Claude sessions, real cloud/tracker and
keychain operations, and reference-conformance checks. Their exact eighteen
names are in `audit.json`. Separate process coverage is valid evidence for its
own scenarios, not proof that those skipped tests ran.

The recorded [Codex/Claude comparison](../2026-10-19-unified-mcp/README.md)
is a small design trial preceding final artifact hashing. It covers the planned
workflow categories but does not justify broad reliability or performance
claims. Repeat focused real-agent acceptance against the corrected candidate
before describing it as verified with current clients.

Hosted clean installation is proven on macOS 15.7.9. The Mach-O deployment
target is 11.0, which is not a runtime test of macOS 11–14.
State the tested operating-system range or add that evidence.
[Apple target documentation](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)

The binary passes strict verification of its ad-hoc signature, but has no
Developer ID/notarization evidence. The verified npm/Cargo installation paths
do not establish Gatekeeper behavior for a browser-downloaded archive.
Do not advertise a notarized direct download.
[Apple distribution guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

The first interactive npm upload has no automatic OIDC provenance. Subsequent
trusted uploads can receive it when npm's public-package/public-repository
conditions are met. Checksums and source receipts remain useful but are not
signed build attestations.
[npm trusted publishing](https://docs.npmjs.com/trusted-publishers/)

No new GUI appearance change or foreground native test was made in this audit.
Local visual evidence from earlier work remains valid for its source and states;
it does not certify the older deployed website. Website changes require new
saved and reviewed screenshots.

## Work to finish before asking for publication

The native audit task is `initial-release-audit`; reasoning step `1705`
records the verdict. Owning fixes were added as backlog work, not implemented:

| Native item | Finished result needed |
| --- | --- |
| `initial-release-dependency-security` | Acceptable native dependency baseline, reviewed conditional exceptions, affected checks, new verified artifacts |
| `initial-release-publication-recovery` | Registry/checksum reconciliation, bounded quota handling, tested partial-upload recovery |
| `initial-release-security-gates` | Required source-bound security results, reviewed immutable workflow/toolchain inputs |
| `initial-release-website-current` | Current deployed instructions, reviewed website dependencies and visual evidence |
| `public-link-verification` (updated) | Consistent release identity, public links and working private security reporting |
| `initial-release-fixture-names` | Remove four obsolete fixture directory names; preserve legal upstream attribution |

Do the engineering fixes first. Then rerun the affected named gates, current
security scans, clean extracted Cargo/npm verification and focused real-agent
checks. Save the corrected candidate's source, run ID and checksums. Old passing
artifacts are not approval inputs after source or dependency changes.

## Exact owner actions, after engineering blockers are cleared

1. **Approve how the old private v0.1.0 release/tag will be reconciled.**
   Do this before changing repository visibility. Do not approve the old
   September assets as the new CLI release.

2. **Prepare the crates.io account.** Verify its email and create a short-lived
   token that permits publishing new crates and new versions for all fourteen
   names. Enter it through a masked local prompt, never into chat:

   ```sh
   gh secret set CARGO_REGISTRY_TOKEN --repo AlrikOlson/magistr --env cli-release
   ```

   Confirm actual new-crate quota or rely on the corrected resumable publisher;
   do not expect fourteen immediate new publications from default limits.
   [Cargo account requirements](https://doc.rust-lang.org/cargo/reference/publishing.html)

3. **Prepare npm authentication.** Use an account with verified email and 2FA:

   ```sh
   npm login --registry=https://registry.npmjs.org
   npm whoami --registry=https://registry.npmjs.org
   ```

   Complete browser/2FA prompts yourself. First publication uses the exact
   verified tarball; it must not be a placeholder. Review the final run and
   checksum before authorizing the upload.

4. **Configure repository protections and public access.** Approve visibility,
   protect `main`, configure applicable release-environment protection, and
   enable a private security-reporting channel. Recheck the deployed site and
   anonymous links after those changes.

5. **Authorize the corrected publication explicitly.** State version, package
   run ID and registries. The candidate in this audit is on hold; do not use
   run 39352947283 as an approved upload merely because its installation tests
   passed. Registry availability must be rechecked immediately before upload.

6. **Configure future npm trusted publication after package creation.**
   Use repository `AlrikOlson/magistr`, workflow `cli-publish.yml`,
   environment `cli-release`, and explicit publish permission. The current
   docs require first use within two days of configuration; create/recreate it
   when the next trusted upload is ready. Current supported owner command:

   ```sh
   npm trust github magistr --file cli-publish.yml --repo AlrikOlson/magistr --env cli-release --allow-publish
   ```

   This needs npm 11.15+ and interactive 2FA. OIDC publishing needs a supported
   hosted runner, Node 22.14+ and npm 11.5.1+.
   [npm trust](https://docs.npmjs.com/cli/v11/commands/npm-trust/),
   [trusted-publisher requirements](https://docs.npmjs.com/trusted-publishers/)

After authorized uploads, fetch the real registry archives and compare hashes,
install with fresh Cargo/npm homes and caches, run `magistr --version`,
disposable-project setup, `doctor` and read-only MCP context, then check the
published release assets and live instructions. These cannot honestly be marked
passed while registry packages are unpublished.

If an incorrect version is uploaded, stop and inspect registry state.
Do not overwrite or blindly replay immutable versions; use a reviewed correction,
deprecation/yank policy and a new version when necessary.
[npm unpublish policy](https://docs.npmjs.com/policies/unpublish/),
[Cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)

Declare the public release only after those checks pass. Version management
remains consolidated in `Cargo.toml`; `0.1.0` is still appropriate for this
first public release.
