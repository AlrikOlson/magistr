# Workspace subsystem component notices

These notes describe the files imported from Workbench revision
`2a2430bbcd5dcbed00920f92c6f3e6928f387d95` and license texts obtained on
2026-09-10. Complete texts are retained under [LICENSES/](LICENSES/).
[PROVENANCE.md](PROVENANCE.md) records the source boundary and artifact hashes.

## Workbench declarations

[reference/Cargo.toml](reference/Cargo.toml) declares
`workspace.package.license = "Apache-2.0"`. Its `workbench-core` and
`componentize` members inherit that field. The pinned repository contains no
top-level LICENSE or NOTICE file. No such upstream file or authorship notice
has been invented by this import.

The four standalone `plugins/*/guest/Cargo.toml` files omit a license field
and each declares its own workspace. They do not inherit the root Cargo
license field. This document does not assign them a license or imply that
missing upstream notices have been resolved. The root Cargo declaration is
the available evidence for the native source; it is not a substitute for
missing license evidence for unrelated reference files.

## Vendored interpreter and grammar

| Component | Imported artifact | Evidence | Complete license text |
|---|---|---|---|
| QuickJS-ng 0.16.2 | `reference/vendor/qjs/qjs-wasi.wasm` | The adjacent README names the official release and MIT license; [tagged upstream LICENSE](https://raw.githubusercontent.com/quickjs-ng/quickjs/v0.16.2/LICENSE) | [QuickJS-ng MIT](LICENSES/quickjs-ng-0.16.2-MIT.txt) |
| tree-sitter-gleam 1.0.0 generation | `reference/plugins/lang-pack-gleam/grammar/gleam.wasm`, also embedded in the Gleam guest | The grammar README names this generation; [v1.0.0 upstream LICENSE](https://raw.githubusercontent.com/gleam-lang/tree-sitter-gleam/v1.0.0/LICENSE) declares Apache-2.0 | [Gleam Apache-2.0](LICENSES/tree-sitter-gleam-1.0.0-Apache-2.0.txt) |

The QuickJS license text retains the upstream copyright notices for Fabrice
Bellard, Charlie Gordon, Ben Noordhuis, and Saúl Ibarra Corretgé. The copied
texts are unmodified. Their SHA-256 values are:

| License file | SHA-256 |
|---|---|
| `quickjs-ng-0.16.2-MIT.txt` | `96f73f9d2a16c21a36b418f06073be26e7d6d5e7c1bc99756b21a4f2c74ef171` |
| `tree-sitter-gleam-1.0.0-Apache-2.0.txt` | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |

The Gleam license is associated with the source generation documented by
upstream. The import does not independently attest the source commit used
for the grammar binary. Neither vendored binary has a complete original
build-input inventory in the pinned repository.

## Preserved guest dependency notices

The following crates occur in the preserved guest lockfiles and provide
runtime or generated binding support used by the guest source. MIT license
texts were extracted from their exact registry archives without alteration.
Each archive's SHA-256 was checked against the checksum in the preserved
`reference/plugins/*/guest/Cargo.lock` before extraction. The original Cargo
license expressions are recorded below; retaining the MIT text does not
remove the upstream alternative grants.

| Package | Cargo license expression | Exact archive evidence | Complete MIT text |
|---|---|---|---|
| wit-bindgen 0.46.0 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [Registry archive](https://static.crates.io/crates/wit-bindgen/wit-bindgen-0.46.0.crate) | [wit-bindgen](LICENSES/wit-bindgen-0.46.0-MIT.txt) |
| dlmalloc 0.2.14 | `MIT/Apache-2.0` | [Registry archive](https://static.crates.io/crates/dlmalloc/dlmalloc-0.2.14.crate) | [dlmalloc](LICENSES/dlmalloc-0.2.14-MIT.txt) |
| cfg-if 1.0.4 | `MIT OR Apache-2.0` | [Registry archive](https://static.crates.io/crates/cfg-if/cfg-if-1.0.4.crate) | [cfg-if](LICENSES/cfg-if-1.0.4-MIT.txt) |
| serde 1.0.229 | `MIT OR Apache-2.0` | [Registry archive](https://static.crates.io/crates/serde/serde-1.0.229.crate) | [serde](LICENSES/serde-1.0.229-MIT.txt) |
| serde_core 1.0.229 | `MIT OR Apache-2.0` | [Registry archive](https://static.crates.io/crates/serde_core/serde_core-1.0.229.crate) | [serde_core](LICENSES/serde_core-1.0.229-MIT.txt) |
| serde_json 1.0.151 | `MIT OR Apache-2.0` | [Registry archive](https://static.crates.io/crates/serde_json/serde_json-1.0.151.crate) | [serde_json](LICENSES/serde_json-1.0.151-MIT.txt) |
| itoa 1.0.18 | `MIT OR Apache-2.0` | [Registry archive](https://static.crates.io/crates/itoa/itoa-1.0.18.crate) | [itoa](LICENSES/itoa-1.0.18-MIT.txt) |
| memchr 2.8.3 | `Unlicense OR MIT` | [Registry archive](https://static.crates.io/crates/memchr/memchr-2.8.3.crate) | [memchr](LICENSES/memchr-2.8.3-MIT.txt) |
| zmij 1.0.23 | `MIT` | [Registry archive](https://static.crates.io/crates/zmij/zmij-1.0.23.crate) | [zmij](LICENSES/zmij-1.0.23-MIT.txt) |

Archive checksums are preserved in the guest lockfiles. The license-file
checksums provide an additional check on these extracted notices:

| License file | SHA-256 |
|---|---|
| `wit-bindgen-0.46.0-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `dlmalloc-0.2.14-MIT.txt` | `378f5840b258e2779c39418f3f2d7b2ba96f1c7917dd6be0713f88305dbda397` |
| `cfg-if-1.0.4-MIT.txt` | `378f5840b258e2779c39418f3f2d7b2ba96f1c7917dd6be0713f88305dbda397` |
| `serde-1.0.229-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `serde_core-1.0.229-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `serde_json-1.0.151-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `itoa-1.0.18-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `memchr-2.8.3-MIT.txt` | `0f96a83840e146e43c0ec96a22ec1f392e0680e6c1226e6f3ba87e0740af850f` |
| `zmij-1.0.23-MIT.txt` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |

This table records source-visible dependency evidence. It is not a complete
binary composition attestation: the pinned artifacts have not been traced
back to every compiler, standard-library, WASI SDK, or code-generation input.
The native library's registry dependencies remain governed by their own
package licenses and Cargo lockfile; this import does not vendor their source
or replace those declarations.
