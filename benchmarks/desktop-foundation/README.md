# Desktop foundation proof

The [12 October 2026 decision](../../docs/architecture/desktop-foundation.md)
selects Tauri: a Rust host and the OS WebView. This separate, bounded workspace
contains the selected fixture and its shared Rust workload. It opens no project
store, provider, workflow writer or Magistr service. It is not a second
production desktop or a workflow authority.

Use the explicitly measured installed compiler rather than the default:

```sh
cargo +1.99.0 build --manifest-path benchmarks/desktop-foundation/Cargo.toml --release --locked
python3 benchmarks/desktop-foundation/measure.py prepare
python3 benchmarks/desktop-foundation/journeys.py --output /private/tmp/magistr-foundation-journeys/tauri
python3 benchmarks/desktop-foundation/measure.py performance --output /private/tmp/magistr-foundation-selected-performance
```

Native observation requires macOS, Xcode's Swift compiler, Accessibility and
Screen Recording access. The observer rejects input to other applications.
It restores the front application, clipboard and temporarily enabled/selected
input sources. Performance requires VoiceOver to be off. The manual reader
exercise uses real VoiceOver keyboard navigation and its caption panel; protected
VoiceOver AppleScript settings are not changed. Linux/Windows native acceptance
requires their own runners and readers.

The [predeclared protocol](protocol.md) describes the corpus, release profile,
budgets and sampling limits. The comparison uses five alternating fresh process
launches and three alternating active runs per candidate, with warm filesystem
caches. Display-time pixel observations are separate from DOM/draw acknowledgements.
CPU ticks are calibrated against getrusage and converted with mach_timebase_info.
RSS sums every resource-coalition process, including WebKit helpers; it is not
unique physical memory. Streaming CPU endpoints reject an exited starting PID
and count newly born helpers. Earlier comparison CPU has explicit conservative
[bounds](../../docs/evidence/desktop-foundation/cpu-bounds.json).

Record a new selected validation after the native commands finish:

```sh
python3 benchmarks/desktop-foundation/evidence.py selected --performance /private/tmp/magistr-foundation-selected-performance --journeys /private/tmp/magistr-foundation-journeys/tauri
python3 benchmarks/desktop-foundation/verify.py
```

The verifier audits actual artifacts and accepted boundaries. It reports missed
idle/frame targets as recorded delivery work; it does not convert them into
passing performance budgets. The native journey exits nonzero on a failed core
check. Its 15 checks exclude unreliable automatic IME activation, which is
recorded as unavailable. Separate successful native conversion and failed
activation observations are retained in the evidence index.

Focused developer checks:

```sh
cargo +1.99.0 fmt --manifest-path benchmarks/desktop-foundation/Cargo.toml --all --check
cargo +1.99.0 check --manifest-path benchmarks/desktop-foundation/Cargo.toml --workspace --all-targets --locked
cargo +1.99.0 nextest run --manifest-path benchmarks/desktop-foundation/Cargo.toml --workspace --locked
cargo +1.99.0 clippy --manifest-path benchmarks/desktop-foundation/Cargo.toml --workspace --all-targets --locked -- -D warnings
node --test benchmarks/desktop-foundation/tauri/tests/composer.mjs
bunx prettier@3.6.2 --no-config --check benchmarks/desktop-foundation/tauri/ui benchmarks/desktop-foundation/tauri/tests
```

Node, Bun and Prettier are development tools; the application has no Node runtime
or frontend package dependencies.

The losing GPUI source and the exact matched harness are retained solely in the
[comparison archive](../../docs/evidence/desktop-foundation/comparison-source.tar.gz),
with hashes, licenses and reproduction notes in the
[evidence index](../../docs/evidence/desktop-foundation/README.md).
No active manifest, lockfile, command or fixture selects GPUI.
