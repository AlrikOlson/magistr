# Configuration ownership

Native workflow configuration is captured by
[NativeConfig](../../crates/magistr-engine/src/native/native_config.rs)
before a reader, writer or attached client selects its project.
`from_env` is the production capture boundary; `from_lookup` accepts injected
inputs for tests and explicit callers. Each requested environment key is read
at most once during resolution. No environment map is retained after parsing.

The captured object supplies the store, socket policy, sanitized project-name
override and typed think settings. Reader/writer admission and attachment use
that same identity input. Opening engines materializes think settings from the
captured object, the admitted project ID and native persistence. It does not
call the workflow compatibility loader again. The native publisher owns the
socket, so the think engine receives no broadcast path.

## Compatibility and precedence

Existing names and on-disk locations remain compatible. These are input
compatibility rules inside Magistr, not separate configuration authorities.

| Setting | Resolution |
| --- | --- |
| Project root | Explicit host input. Hosts should supply an absolute root. |
| Project identity | Captured, sanitized `THINK_AND_SHIP_PROJECT_NAME`, then the nearest `.think-and-ship/project.json`, then the canonical path-derived ID. An explicitly admitted project ID remains authoritative. |
| Store root | First nonblank value: `THINK_AND_SHIP_DATA_DIR`; `XDG_DATA_HOME/think-and-ship`; `HOME/.local/share/think-and-ship`; `APPDATA/think-and-ship`; `USERPROFILE/AppData/Roaming/think-and-ship`; then the captured platform temporary directory plus `think-and-ship`. |
| Native persistence | On by default. Trimmed `THINK_AND_SHIP_PERSIST=false` (case insensitive) or `0` disables it. |
| Publisher socket | Nonblank, trimmed `THINK_AND_SHIP_BROADCAST_PATH`, otherwise the existing project socket default when requested. |
| Short-socket directory | Trimmed `XDG_RUNTIME_DIR`, then `TMPDIR`, then `/tmp`. Existing Unix socket length limits and final short-path fallback remain. |
| Strict validation, no color, sessions | Existing switches require exact `true`. `THINK_AND_SHIP_AUTO_SESSION` remains a compatibility alias enabling sessions. |
| History, branch, timeout and rollup limits | Existing integer variables accept trimmed values of at least one. Invalid, zero and out-of-range values keep defaults. Rollup pinned-floor enforcement remains in its owning engine. |
| Think output format | `THINK_AND_SHIP_OUTPUT_FORMAT` accepts console, JSON or Markdown case insensitively. |
| Default think session | A valid explicit `THINK_AND_SHIP_DEFAULT_SESSION_ID` wins and enables sessions. Otherwise enabled sessions use the admitted project ID. Session namespacing remains the workflow engine's responsibility. |

The injected resolver never asks the host for a temporary directory. Its last
fallback is an injected `TMPDIR` on Unix (or `TMP`, `TEMP`, `USERPROFILE` on
Windows), then `/tmp/think-and-ship`. Production hosts use `from_env` so the
platform fallback, including macOS per-user temporary directories, is preserved.

## Provenance

`configuration_sources()` reports field paths mapped to `ConfigSource`:
built-in/platform default, named environment variable, explicit host override
or admitted project identity. It exposes source names without environment
values, credentials or rejected payloads. Warnings for rejected think settings
also omit the supplied values. Builder overrides update their field's source;
configuration fields are read through accessors outside the native composition
module.

The shared
[ThinkConfigResolution](../../crates/magistr-workflow/src/think/config_resolution.rs)
parser uses explicit persistence and injected input lookup. Its
`ThinkConfig::with_persistence` initializer is independent of the environment.
The legacy `load_config()` and `ThinkConfig::default()` remain compatibility
entry points for existing library callers; native engine composition uses the
explicit path.

## Guarantees and remaining boundaries

Capture is per key, not an atomic snapshot of the entire process environment.
Project identity files are read at admission; they are not frozen source
snapshots. After admission, the owner/reader retains the selected identity.
Relative paths retain filesystem-relative semantics, so explicit integrations
should pass absolute roots. Engine trace metadata remains its own concern.

This delivers the frozen native workflow portion of the project-configuration
refactor. Project/preset/provider/plugin/workspace resolution, administrative
setup and removal receipts, and effective configuration/policy digests in
verification applicability remain separate dependent roadmap work. This
configuration provenance is not a source digest, workflow revision, owner
generation or receipt. Protected workspace edits and human-owned command
admission remain enforced by their existing owners.

Tests cover lookup isolation, exact compatibility parsing, default/builder
provenance, rejected diagnostic payloads, admitted session identity, real engine
opens and reader/writer identity precedence. They use injected inputs,
temporary projects/stores and child-process environments rather than mutating
the test runner's process environment. The
[Rust 2024 environment guidance](https://doc.rust-lang.org/stable/edition-guide/rust-2024/newly-unsafe-functions.html)
explains why process-wide mutation is unsuitable for concurrent tests.
