# Initial-release remediation and audit — 19 October 2026

**Engineering checks passed. Publication still needs the owner's actions below.** This is the follow-up to the [original 73-check audit](../2026-10-19-release-audit/README.md), which remains unchanged as historical evidence. No Cargo/npm upload, new public tag or public-release declaration occurred.

The checklist now records **56 PASS, 7 OWNER, 6 LIMITED and 4 PENDING**. PASS describes the specific tested claim. OWNER requires an account action or explicit release decision. LIMITED states what the evidence does not establish. PENDING can only be checked after publication. There are no unresolved BLOCKED or WARN findings from the original audit.

The release target is **Apple Silicon, macOS 15 or later, Cargo/npm CLI version 0.1.0**. The root Cargo workspace owns the product version; `scripts/version.py` synchronizes its generated consumers. Desktop distribution, Intel Macs, Linux and Windows remain outside initial-release acceptance.

## Exact candidate

| Item | Reviewed value |
| --- | --- |
| Source | `1e99aba7b9e3d9f76c953ddf03041166f45aa599` |
| Hosted package run | [39364916005](https://github.com/AlrikOlson/magistr/actions/runs/39364916005) |
| Artifact | `cli-distribution`, ID `12238999976`, 36,996,424 bytes |
| Artifact ZIP SHA-256 | `3641229644941f19d7351ba3a13cf4fc321dedb03a8002118150d3ad10cc2ff2` |
| npm archive | `magistr-0.1.0.tgz`, 33,077,883 bytes, 933 files |
| npm SHA-256 | `93f5e3d798389c5344f6e7973377e5f66d82c96a7153d817004b60224c11670a` |
| Native binary SHA-256 | `8c3d78ae31a27482812f88523fb268512a5cdf1471e50a99422f4926ed479317` |
| Cargo archives | 14; all reproduced byte for byte from the reviewed source |
| Native notices | 534 dependency records; 925 notice hashes independently checked |
| Artifact retention | Until `2027-01-18T03:33:25Z` |
| Security receipt | Generated `2026-10-20T03:49:02.215504+00:00`; maximum age 24 hours |

The selected source is the last product/build/package change. Later commits contain release documentation and retained evidence. The local download is `target/hosted-release-1e99aba7/`; its ZIP digest, normalized archives, npm manifest/file inventory, executable permissions, native dependencies and strict ad-hoc signature were checked independently. The hosted binary differs from the local compiled binary; no claim of binary reproducibility is made.

Review [all archive checksums](SHA256SUMS), [publication order](publication-order.txt), [Cargo inventory](cargo-inventory.json), [npm inventory](npm-inventory.json), [native notices](binary-inventory.json) and the [independent artifact review](independent-artifact-review.json). The [Cargo installation receipt](cargo-install-verification.json) and [npm installation receipt](npm-install-verification.json) are bound to these exact hashes. Their clean installations ran on macOS 15.7.9 arm64 without a source checkout; npm's test PATH contained neither Rust nor Bun. Cargo used a checksum-verified vendor registry for the still-unpublished packages. Real public-registry installation remains a post-publication check.

## What was fixed

- Replaced vulnerable YAML and JWT dependency owners, updated the affected Wasmtime/TLS/QUIC/native dependency graph, and refreshed complete native license notices. [Source](source-security.json) and [artifact-bound security](security-verification.json) scans report zero applicable Rust/npm findings and zero dependency exceptions. The 21 exact synthetic secret fixtures remain explicitly reviewed.
- Added exact-archive, dependency-ordered Cargo publication with quota handling, indexing/checksum reconciliation, uncertain-effect receipts and safe resumption. Publication now requires matching successful source checks and fresh security receipts. Actions and toolchains have canonical reviewed pins.
- Finished guided terminal onboarding, actual installed Codex setup and Claude transport/cancellation checks. Removed obsolete current fixture state names and kept workflow/project ownership inside Magistr. Upstream attribution and historical evidence remain intact.
- Fixed native permission routing to the exact unified public tool catalog, and implemented `--continue` through the existing saved-session admission owner. Real screenshot sessions exposed both defects. Fixed the Codex launcher facade and stale shared-version fixtures; no compatibility paths or architecture exceptions were added.
- Updated and deployed current installation/repair/removal instructions, patched website dependencies, added real app and client screenshots, and corrected image-loading/capture verification. [CI corrections](ci-corrections.json) retain the failed attempts and the owning fixes.
- Archived the obsolete September release as a draft under `archive/unpublished-v0.1.0-20260909`, preserving its original annotated tag object and assets. Restored main protection, prepared the release environment and kept the detached backup worktree intact.

## Verification and screenshots

All four final hosted workflows passed against the exact source:

| Check | Result |
| --- | --- |
| [Rust and website](https://github.com/AlrikOlson/magistr/actions/runs/39364882173) | All-targets check, format, Clippy and website checks; 8,069 Rust tests passed, 18 explicit skips. Main job: 16m40s. |
| [Cargo/npm qualification](https://github.com/AlrikOlson/magistr/actions/runs/39364916005) | Clean extracted Cargo install, packed npm/global/fresh-cache npx, signals, worker startup and removal; 8 unified MCP and 8 onboarding scenarios. Job: 13m56s. |
| [Current security](https://github.com/AlrikOlson/magistr/actions/runs/39364882252) | Clean native/website/script dependencies and reviewed full-history secret scan. Job: 2m. |
| [macOS desktop](https://github.com/AlrikOlson/magistr/actions/runs/39364882140) | App packaging, child ownership and native contents checks. Job: 8m03s. Separate desktop readiness job: 8m45s. |

See [CI receipts, durations and all skips](ci.json). Every Rust CI job targets macOS. Remaining Windows/Linux adapter jobs run JavaScript only and are bounded to 10 minutes. Rust/website and desktop gates have 30-minute limits; the packaging job permits 60 minutes for two independent release builds, and publication permits 150 minutes for registry quota/indexing waits. Publication does not compile Rust.

The final exact-artifact trials used Codex 0.163.0 and Claude Code 2.1.304, each with an implementation turn and a fresh-session recovery/finalization turn:

| Client | Outcome | Total time | MCP tool calls | Tool errors |
| --- | --- | --- | --- | --- |
| Codex | PASS | 82.28 s | 26 | 0 |
| Claude Code | PASS | 207.04 s | 31 | 1, recovered |

[Independent client evidence](agent-evaluations.json) confirms unchanged tests, the source fix, recorded required verification, feedback, roadmap work, factual decision records and completed tasks/objectives. Codex deliberately recorded a failing optional baseline before its passing required check. Claude's recovered error omitted the required `lane` on a read-only roadmap query; the server stopped composition with completed-operation receipts and no automatic replay. These small acceptance trials do not establish broad performance or reliability. Earlier comparisons remain historical and include worse Claude hybrid timing.

Fresh screenshots are live at [magistr.dev/#agents](https://magistr.dev/#agents):

- [Native app, light](../../desktop-iterations/2026-10-19-release-product-captures/desktop-resumed-light.png) and [dark](../../desktop-iterations/2026-10-19-release-product-captures/desktop-resumed-dark.png).
- [Actual Codex session](../../desktop-iterations/2026-10-19-release-product-captures/codex-resume.png).
- [Actual Claude Code session](../../desktop-iterations/2026-10-19-release-product-captures/claude-code-resume.png).

The [product review and provenance](../../desktop-iterations/2026-10-19-release-product-captures/REVIEW.md) state the real capture methods and source/binary revisions. Native captures are ScreenCaptureKit images of the rebuilt app. Client captures render unmodified real interactive PTY output with xterm/WebKit. The app's native conversation proposed a fix and test; the separate CLI sessions actually implemented and verified it. The screenshots predate the final packaged binary; the final-artifact trials above supply that separate acceptance evidence.

All selected originals, 12 corrected local gallery frames and [12 deployed frames](../../desktop-iterations/2026-10-19-release-session-gallery-production/REVIEW.md) were opened and inspected. Every one of the 39 current served resources matches the reviewed build using the exact content-hashed image URLs. Unversioned image URLs may retain older immutable cached bytes. Failed captures remain separately recorded. No OS keyboard/pointer/clipboard input or assistive-technology certification is claimed; the app activated itself despite a requested background launch.

## Exactly what the owner needs to do

1. **Prepare crates.io access.** Sign in to [crates.io](https://crates.io/), verify your email, and create a short-lived token permitted to create all 14 new names in [publication-order.txt](publication-order.txt), as well as their versions. Add it with this masked prompt:

   ```sh
   gh secret set CARGO_REGISTRY_TOKEN --repo AlrikOlson/magistr --env cli-release
   ```

   The environment currently contains no secrets. A token restricted to existing crates cannot create this first release. See [Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html).

2. **Sign in to npm on this Mac and enable 2FA.** Complete the browser authentication yourself:

   ```sh
   npm login --registry=https://registry.npmjs.org
   npm whoami --registry=https://registry.npmjs.org
   ```

   The current check returns `ENEEDAUTH`. Do not paste credentials or recovery codes into chat.

3. **Approve making `AlrikOlson/magistr` public.** Then the agent can make the authorized visibility change, enable and verify private vulnerability reporting, configure supported environment review restrictions, and verify anonymous repository/support URLs. Main already requires successful Rust/website and security checks, strict updates and resolved conversations; force pushes and deletions are disabled. Administrator enforcement is checked again after the final evidence push. The current private account plan rejects environment reviewers; this limitation is recorded in [administration evidence](admin.json) and [GitHub's environment documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments).

4. **Approve the exact release.** Authorize Cargo/npm version `0.1.0` from **CLI packages run `39364916005`**, source `1e99aba7b9e3d9f76c953ddf03041166f45aa599`, using the hashes above. Approval must be explicit; fixing and preparing the release has not published it. If the security receipt is over 24 hours old, refresh qualification before upload.

5. **After the first npm package exists, configure trusted publishing with your 2FA.** Use GitHub owner `AlrikOlson`, repository `magistr`, workflow `cli-publish.yml`, environment `cli-release`, and allowed action `npm publish`. With npm 11.15+:

   ```sh
   npm trust github magistr --file cli-publish.yml --repo AlrikOlson/magistr --env cli-release --allow-publish
   ```

   A first package must exist before this configuration can be created. Its next successful trusted publication must happen within two days of configuration; otherwise recreate it when the next release is ready. The first interactive upload has no automatic OIDC provenance. Later eligible public-repository trusted uploads can have provenance. See [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/) and [npm trust](https://docs.npmjs.com/cli/v11/commands/npm-trust/).

After authorization and authentication, follow [the release procedure](../../../releasing-cli.md) to upload the reviewed archives, fetch and hash real registry packages, run fresh real-registry installs, update the website's published-installation status, and create the approved public tag/release using the [draft notes](RELEASE-NOTES.md). The public-release declaration comes after those checks.

## Full 73-check audit

| ID | Area | Status | Check and observed evidence |
| --- | --- | --- | --- |
| I01 | Identity | PASS | **First-release scope is Apple Silicon macOS CLI** — First-release acceptance is the Apple Silicon CLI on macOS 15 or later; other desktop/CPU/OS distribution remains separate. |
| I02 | Identity | PASS | **One product version owner** — One owner remains workspace.package.version=0.1.0; generated manifests/lockfiles and version tests pass. CI tools also have shared canonical pins. |
| I03 | Identity | PASS | **Audited candidate is identified and tree is clean** — The selected package source is 1e99aba7b9e3d9f76c953ddf03041166f45aa599. Subsequent commits record evidence only; final tree status is retained separately. |
| I04 | Identity | PASS | **Pre-release identity is reconciled before public release** — Archived the obsolete September release as a draft under archive/unpublished-v0.1.0-20260909, preserving the original annotated tag object and both assets. v0.1.0 is available for the explicitly authorized initial release. |
| I05 | Identity | PASS | **Requested registry names are currently unused** — All 14 Cargo names and npm magistr returned 404 at 2026-10-20T03:38:58Z. Availability does not reserve names or prove publishing permission. |
| C01 | Cargo | PASS | **All internal dependencies can be published** — Fourteen normalized archives have matching 0.1.0 registry requirements and no remaining internal path dependency; package licenses retained. |
| C02 | Cargo | PASS | **Required assets are in extracted packages** — Hosted run 39364916005 installs the extracted normalized Cargo archives in release mode on macOS 15.7.9 arm64 against a checksum-verified vendored registry, with no checkout dependency; installed CLI smoke checks pass. |
| C03 | Cargo | PASS | **Cargo package source matches reviewed source** — All 14 final hosted Cargo archives reproduce byte for byte from the reviewed source with Cargo 1.99.1. Archive inventories, source metadata and SHA-256 comparisons are retained. |
| C04 | Cargo | PASS | **Advertised Rust minimum builds the CLI** — The patched graph passes cargo +1.95.0 check --locked for the CLI library and binary; this is minimum-toolchain typechecking, not its full test suite. |
| C05 | Cargo | PASS | **Internal publication order follows dependencies** — Fourteen-package publication-order.txt is derived from the dependency closure. |
| C06 | Cargo | PASS | **First publication tolerates quotas and partial failure** — Exact reviewed archives, topological order, API/index/checksum reconciliation,429 Retry-After/bounded waits, uncertain-effect receipts and safe resumption are implemented;29 tests pass. |
| C07 | Cargo | OWNER | **Account email and a suitable token are ready** — CARGO_REGISTRY_TOKEN is absent from cli-release; owner must verify account email and authorize creation of new crates. |
| N01 | npm | PASS | **Packed package contains the versioned native CLI** — magistr@0.1.0, macOS arm64 restriction, direct native bin, no runtime JavaScript dependencies or install hooks. |
| N02 | npm | PASS | **Global installation needs no Rust, Bun or checkout** — Hosted packed-artifact install passed in an isolated prefix with those tools absent from PATH. |
| N03 | npm | PASS | **npx works from a fresh cache** — Loopback registry and fresh-cache test passed, including the generated version-pinned MCP configuration. |
| N04 | npm | PASS | **Stdio, exit codes, signals and worker startup survive installation** — Actual packed binary checks passed; there is no wrapper process to lose native signals. |
| N05 | npm | PASS | **Package file inventory is intentional** — The final hosted package contains 933 regular files, no symlinks or unexpected paths, and a mode-0755 binary. Compressed size is 33,077,883 bytes; unpacked size is 115,844,287 bytes. |
| N06 | npm | PASS | **Uninstall works** — Packed npm uninstall and executable removal passed. |
| N07 | npm | OWNER | **Publishing account and interactive 2FA are ready** — npm whoami returned ENEEDAUTH. Owner must sign in and complete first-publication authentication. |
| N08 | npm | OWNER | **Subsequent trusted publisher is configured** — No package exists yet. Configure the exact repository/workflow/environment and allow-publish after first creation; verify current expiry rules. |
| M01 | macOS | PASS | **Executable is arm64 and uses expected system libraries** — The exact hosted npm binary is Mach-O arm64, links only system dylibs and passes strict ad-hoc signature verification. SHA-256: 8c3d78ae31a27482812f88523fb268512a5cdf1471e50a99422f4926ed479317. |
| M02 | macOS | PASS | **Minimum operating-system claim is tested** — The documented minimum is macOS 15. Hosted clean installation runs on macOS 15 arm64; the binary's lower Mach-O linking target is explicitly not certification of older macOS versions. |
| M03 | macOS | LIMITED | **Browser-downloaded archive passes Gatekeeper** — No Developer ID/notarization proof. Cargo/npm installation is proven; do not advertise a notarized direct download. |
| U01 | Unified MCP | PASS | **One public MCP connection owns workflow and workspace access** — magistr mcp --dir is used by current integrations; workflow/project and workspace/working-copy ownership remain distinct internally. |
| U02 | Unified MCP | PASS | **All capabilities have catalog coverage** — One typed catalog covers direct common tools plus discover/execute and advanced workspace/workflow operations; schema/handler coverage passed. |
| U03 | Unified MCP | PASS | **Composition keeps authorization and bounded execution** — Native authorization, sandbox bounds, sequential mutation and stop-on-failure receipts pass. Fresh native-session testing also fixed host routing to the exact unified public catalog; governance restrictions retain precedence. |
| U04 | Unified MCP | PASS | **Edits are verified, journaled and reject stale requests** — Source/hash checks, confirmation behavior and session-scoped undo are covered in retained tests. |
| U05 | Unified MCP | PASS | **Concurrent agents and worktrees keep correct owners** — Simultaneous clients, separate worktrees and owner restart passed in the eight process scenarios. |
| U06 | Unified MCP | PASS | **Cancellation and partial execution are recoverable** — Cancellation, stale confirmations and partial failure tests passed; completed effects remain visible without automatic replay. |
| U07 | Unified MCP | PASS | **Response failures and truncation are explicit** — Typed envelope/catalog checks passed; discovery needs no client-specific extension. |
| U08 | Unified MCP | PASS | **Current fixtures use current state names** — All four tas-data fixture references now use workflow-state. Current source ownership/architecture gates pass; historical attribution and audit evidence remain intact. |
| O01 | Onboarding | PASS | **Setup detects the project and proposes changes** — Guided interview and administrative plan service are present; configuration and verification commands are reviewed before application. |
| O02 | Onboarding | PASS | **Only selected integrations are generated** — Codex, Claude Code and portable setup are supported; selected-agent and multi-agent process cases passed. |
| O03 | Onboarding | PASS | **Repeated setup preserves custom settings and other servers** — Customized-config, repeated setup, paths with spaces, repair and removal cases passed. |
| O04 | Onboarding | PASS | **Unavailable clients produce actionable diagnostics** — Unavailable-agent diagnostics and doctor behavior pass in the eight onboarding process scenarios. |
| O05 | Onboarding | PASS | **Setup finishes with a real connection and read-only context** — Protocol handshake/context checks passed; setup suggests planning and does not start implementation. |
| O06 | Onboarding | PASS | **MCP startup does not automatically repair project files** — Repair belongs to explicit setup/repair through the administrative change service. |
| O07 | Onboarding | PASS | **Upgrade, repair and removal are documented** — Current release procedure documents ownership-preserving repair/removal and reopening installed clients. |
| O08 | Onboarding | PASS | **Interactive terminal onboarding is exercised** — Eight onboarding process cases now include real background PTY customization, proposal-before-write, acceptance and cancellation with no file changes. No foreground or human usability walkthrough is claimed. |
| E01 | Agent evaluations | PASS | **Codex and Claude Code were exercised on realistic workflows** — Codex 0.163.0 and Claude Code 2.1.304 each completed implementation and fresh-session resumption against the exact final hosted npm binary. Independent state/source checks pass for safe edits, plans, feedback, decisions, roadmap, recorded verification and completion. One Claude read-only argument error was explicitly rejected and recovered; no automatic replay occurred. |
| E02 | Agent evaluations | LIMITED | **Efficiency claims reflect measured evidence** — Small comparisons record success/calls/context; Claude unified timing was worse. No broad performance or reliability claim is justified. |
| T01 | CI | PASS | **Exact candidate passes full Rust checks** — Final source 1e99aba7 passes all-targets check, formatting, Clippy and 8,069 workspace tests in hosted run 39364882173; 18 opt-in cases are explicitly skipped. |
| T02 | CI | PASS | **Protocol and onboarding processes pass on release packaging** — Final packaging run 39364916005 passes all eight unified MCP process scenarios and all eight onboarding scenarios, including the interactive PTY interview and cancellation. |
| T03 | CI | PASS | **Release does not wait for Windows Rust builds** — Every Rust CI job now targets macOS, including the manual desktop workflow. Remaining Windows/Linux adapter checks execute JavaScript only and have a 10-minute bound; public HTTP checks use a 5-minute job. |
| T04 | CI | PASS | **CI duration is bounded and measured** — Final Rust/website job took 1,000 seconds (16m40s), within its 30-minute bound. Packaging took 836 seconds; independent desktop jobs took 483 and 525 seconds, and security took 120 seconds. Jobs ran concurrently. |
| T05 | CI | LIMITED | **Skipped live scenarios are accounted for** — The final hosted skip inventory is retained separately. Opt-in local Codex and Claude transport checks were also run; optional cloud/tracker/keychain and reference scenarios are not covered by the full-suite pass. |
| T06 | CI | PASS | **Reviewed artifacts are tied to checks and checksums** — Publication selector requires the intended successful package run/source and matching installation receipts; source archive reproduction passed. |
| T07 | CI | PASS | **Artifacts are retained long enough for review** — Verified hosted artifact 12238999976 is retained until 2027-01-18T03:33:25Z; local ZIP and unpacked copies plus tracked inventories/receipts are retained. Security approval expires after 24 hours and requires a fresh qualification if delayed. |
| S01 | Security | PASS | **Enabled native dependencies have no unresolved applicable advisories** — Final-source target-aware scanning reports zero findings across 492 enabled native CLI packages, including advisory warnings; no dependency exception is configured. |
| S02 | Security | PASS | **Sandbox runtime is on an acceptable security baseline** — Wasmtime 48.0.7 and the grammar component's C API 36.0.19 clear the current runtime findings. Affected sandbox and language-pack tests pass. |
| S03 | Security | PASS | **Website dependencies have a reviewed security baseline** — Astro 7.4.1, MDX 8.1.0 and Sharp 0.35.5 replace the vulnerable website dependency chain. Current npm audit reports zero findings. |
| S04 | Security | PASS | **Release-script JavaScript dependencies pass current audit** — The release tooling, including pinned Wrangler 4.151.2, has zero current npm audit findings. |
| S05 | Security | PASS | **Git-history secret matches are reviewed** — Full-history Gitleaks 8.30.1 reports only 21 exact reviewed synthetic-fixture fingerprints. No broad file exclusions or inline suppression is accepted. |
| S06 | Security | PASS | **Embedded JavaScript engine has provenance** — QuickJS-ng 0.16.2 WASI hash matches the official release asset; reviewed upstream advisories do not establish an unfixed matching issue. |
| S07 | Security | LIMITED | **Prebuilt embedded engine is reproducible from source** — Upstream asset/source/license provenance is retained; original toolchain inputs are insufficient for an independent byte-for-byte rebuild. |
| S08 | Security | PASS | **Security checks are enforced before publication** — Push/PR security and artifact-bound package scans are enforced. Publication requires successful matching source runs and a clean receipt bound to every archive, less than 24 hours old. |
| S09 | Supply chain | PASS | **Action and compiler inputs are immutable** — External workflow/composite actions use full commit SHAs. Rust 1.99.1, Node 24.22.0, Python 3.14.8, Nextest 0.9.130 and scanners have reviewed canonical pins; the hosted arm64 jobs use them. |
| S10 | Supply chain | OWNER | **Main and release environment have appropriate protection** — Main has strict Rust/website and security requirements bound to GitHub Actions, conversation resolution, administrator enforcement, and no force pushes/deletions. The cli-release environment accepts only main. Required environment reviewers are unavailable on the current private account plan; that remaining setting follows the owner's visibility/plan decision. Final enforcement is checked again after the evidence push. |
| S11 | Supply chain | LIMITED | **Package provenance is accurately described** — Checksums/source receipts exist. First interactive npm upload has no automatic OIDC provenance; later public-repository trusted uploads can have it. |
| L01 | Legal and privacy | PASS | **Package license notices match their inventory** — The final native notice inventory contains 534 dependency records and 925 notice hashes, including nested native-library attribution. Exact packed notice bytes are checked independently; this is a technical inventory review. |
| L02 | Legal and privacy | LIMITED | **Data and permission behavior is fully observed** — Code defaults telemetry consent off and requires an endpoint; local records/provider permissions are documented. No packet-capture audit of every optional integration was performed. |
| W01 | Public launch | PASS | **Public website is reachable** — The production homepage and installation guide respond successfully at magistr.dev. |
| W02 | Public launch | PASS | **Deployed instructions match current setup** — Deployed current selected-agent setup, read-only connection check, plan-first boundary, doctor, repair, upgrade and removal instructions; registry packages are truthfully marked unpublished. |
| W03 | Public launch | OWNER | **Repository and support links are public** — Repository/releases/issues/security URLs return anonymous 404 while repository is private. Owner visibility decision is outstanding. |
| W04 | Public launch | OWNER | **Private vulnerability reporting works** — Current API/page check could not establish a usable route. Enable and verify private reporting or document a concrete private contact. |
| W05 | Public launch | PASS | **Launch checks verify deployed content and required links** — Semantic HTTP/content checks now run the website stage on every push/PR, publication prerequisites before uploads, and registry/full-release checks on manual/tag release runs. No continue-on-error bypass. |
| W06 | Public launch | PASS | **Visual evidence describes what is actually deployed** — Production deployment 551dda39-03b8-4b26-bebe-6c4e26e046de serves the reviewed source. All 39 current versioned resources match; 12 fresh production gallery frames were opened and reviewed, alongside 12 corrected local frames and four selected real product/client images. |
| P01 | Publication | OWNER | **Owner approves exact corrected artifacts and public launch** — Owner approval is still required for version 0.1.0 from source 1e99aba7b9e3d9f76c953ddf03041166f45aa599 and selected CLI packages run 39364916005. No registry upload or new public tag has occurred. |
| P02 | Publication | PENDING | **Cargo installs successfully from the real registry** — Run after publication using an isolated Cargo home/root and --locked; unpublished names cannot be installed from crates.io now. |
| P03 | Publication | PENDING | **npm global and npx install successfully from the real registry** — After publication, verify integrity and exact version with fresh cache/prefix and a disposable project. |
| P04 | Publication | PENDING | **Published archives match reviewed hashes and metadata** — After upload, fetch registry archives and compare all 14 Cargo checksums, npm integrity/version/bin metadata and release assets. |
| P05 | Publication | PASS | **Support and correction procedure is ready for a bad upload** — Exact-artifact partial-upload recovery, uncertain-effect handling and immutable bad-version correction/yank/deprecation procedures are documented and covered by controlled-registry tests. |
| P06 | Publication | PENDING | **Public release is explicitly declared** — Declare public release only after authorized uploads, real registry install/hash checks and deployed installation/support routes pass. The archived September draft is not a public-release declaration. |

[Machine-readable audit](audit.json) and [current primary-source research](research.json) accompany this checklist. Registry availability was checked at 2026-10-20T03:38:58Z; all 15 requested names were unused, which neither reserves them nor proves publishing access. Recheck immediately before publication.
