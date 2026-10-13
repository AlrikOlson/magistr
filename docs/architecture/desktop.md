# Desktop architecture

The desktop is `magistr-desktop`: a Rust Tauri 2.12 host with the operating
system's WebView and semantic HTML/CSS. Its only launch route is:

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked -- --dir /path/to/project
```

The directory must be a configured Magistr project. The initial conversation
uses the selected provider, model and permission profile. `--resume` and
`--continue` use application-owned session history. Later conversations open
through the same admission path.

## Ownership

| Component | Owns |
| --- | --- |
| `application::SessionRegistry` | Resident sessions, admission, addressed controls, bounded observation and ordered shutdown |
| `InteractiveHost` and `ProjectResources` | Provider, workflow attachment, source workspace and their child lifetimes |
| `SessionObservation` | Retained messages, current status/model/usage, requests and drafts |
| Desktop commands | Typed translation from window actions to the application client |
| Desktop streams | One window subscription, batching and acknowledgement credit |
| WebView | Native editing, accessible controls, focus, layout and bounded conversation presentation |

The UI never launches a provider or writes a second session registry. Every
command and frame carries project, canonical workspace and logical session
identity. Switching views replaces the subscription; resident work continues.
Close session settles that session. Quit awaits the registry and its project
resources before the native event loop accepts exit. Window visibility preserves
the current view; replacing a selection or dropping the window stream ends its
subscription without closing resident sessions.

A Tauri Channel delivers ordered batches. One batch may be in flight; the next
waits for acknowledgement of the exact sequence after DOM application. Slow views
cannot accumulate token payloads. Navigation intentions are monotonic at both
ends. Late subscriptions and frames cannot replace the selected session.
The application snapshot is bounded to 512 messages and one MiB of text.
Sequence gaps cause an owner refresh and an explicit reseed. The conversation
shows when older messages are unavailable. Application-owned status, model,
usage, requests and drafts survive pruning.

## Interface

The left rail lists known projects and resident sessions. The conversation uses
native editable text and selectable messages; the contextual panel shows the
working copy, model, usage and permission requests. Open project, New session,
Send, Stop, Allow/Deny, Copy conversation and Close session call real owners.
Appearance selects System, Light or Dark. Motion is absent; native reduced-motion
preferences and `--reduce-motion` are honored.

Roadmap editing, supervision, detailed file review, project setup and settings
are later desktop work. These remain available through the CLI and agent tools;
the current window does not offer unfinished controls.

## Verification

The separate Cargo workspace has explicit fmt, check, build, Clippy and Nextest
CI jobs on macOS and Linux. Node tests cover composition, addressed sequence
handling, stale navigation and bounded retention. `tests/process.rs` exercises
the actual executable's worker, ACP and both MCP roles and rejects retired
renderer dependencies.

`tests/native_window.py` builds a disposable signed macOS app around the real
binary. It uses native keyboard, pointer and accessibility input with a controlled
ACP peer, isolated projects and state. It exercises independent sessions and
working copies, streamed replies, permissions, cancellation, drafts, native
clipboard, light/dark/narrow views and orderly Quit. It retains screenshots,
an accessibility tree and a receipt tied to source hashes.
Run it after rebuilding the desktop:

```sh
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop
```

See [current evidence](../evidence/desktop-current/README.md). Screenshot checks
verify current capture hashes, contrast, the selected icon and source freshness.
Performance checks exercise the production conversation reducer and the native
event-to-accessible-reply receipt. This is not a whole-application release benchmark.
The [foundation decision](desktop-foundation.md) retains the framework comparison.

macOS has been exercised locally. Linux is configured in CI; Windows build,
process-tree settlement and complete native IME observation have their own
roadmap items. Automated Japanese input-source observation must be reported as
observed; composition unit tests and Unicode paste alone do not prove conversion.
