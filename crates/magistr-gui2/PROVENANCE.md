# Desktop source provenance

The maintained desktop source is `crates/magistr-gui2` on `main`.

- Source repository: https://github.com/AlrikOlson/magistr
- Reviewed branch: `gui2`
- Source commit: `f0d6370ac32877160e6d40852b8051736e4ca6ec`
- Imported scope: the complete `crates/magistr-gui2` subtree, including fonts, notices, reference material, fixtures and golden images.
- Source license: MIT; the repository's root LICENSE applies. Font licenses and attribution remain in `fonts/`.
- Integration: shared crates come from the current checkout. The old branch's shared cockpit, application, engine, root manifest and lockfile were not imported.

The adoption changes the desktop's dependency paths, typed fixture inputs, accessibility role mapping and layout feedback to use the current shared contracts. The renderer and its assets retain their original history at the source commit. Subsequent fixes are maintained here.

## Renderer dependency

`qs-gpu`, `qs-platform` and `qs-text` come from the public repository
https://github.com/AlrikOlson/qs-framework at
`a788b7be2d1035c64f4387259f4c8b3fc670e9ec`.
Retrieval of that commit, its Cargo manifest and LICENSE was verified on 17 September 2026. The repository became public on 19 September 2026. Its workspace declares Apache-2.0 and the pinned LICENSE contains the Apache License 2.0 terms. The dependency is fetched by Cargo; its source is not copied into Magistr. Any desktop distribution must retain its required license and notices.

No GitHub login is needed to fetch the renderer dependency. This package is excluded from the root workspace and owns its own workspace and Cargo.lock.

## Reproducible build

Rust 1.95 is the supported floor for this checkout. The desktop lockfile pins the complete graph, including wgpu 30, winit 0.30, AccessKit 0.24 and accesskit_winit 0.33. Keep these versions compatible with the pinned renderer. Use `--locked` for builds and tests, and review lockfile changes deliberately.

See [the desktop guide](../../docs/architecture/desktop.md) for commands and the delivered interface boundary.

