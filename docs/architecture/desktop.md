# Desktop architecture

The desktop is `magistr-desktop`: a Rust Tauri 2.12 host with the operating
system's WebView and semantic HTML/CSS. Its only launch route is:

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked
# Optionally open a particular folder after the window is ready:
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked -- --dir /path/to/project
```

The window opens before project or provider admission. With no directory it
shows a project chooser and reads existing session history without creating a
project, database or provider. A requested directory is inspected after the
window is ready; invalid settings and unavailable agents remain visible there.
The initial conversation uses the selected provider, model and permission profile. `--resume` and
`--continue` use application-owned session history. Later conversations open
through typed launch, new and saved-opening intents in the same admission path.
A saved conversation has one resident claim until it settles; repeated opens select
its existing view or receive a refusal.

## Ownership

| Component | Owns |
| --- | --- |
| `application::SessionRegistry` | Resident sessions, admission, addressed controls, bounded observation and ordered shutdown |
| `InteractiveHost` and `ProjectResources` | Provider, workflow attachment, source workspace and their child lifetimes |
| `SessionObservation` | Retained conversation identity/name, messages, complete configuration, status/usage, requests and drafts |
| `ProjectFolder`, `ProjectSetup`, `ScaffoldPipeline` and `AdminService` | Read-only readiness, detected setup plan, exact approved writes, ownership and per-path receipts |
| Desktop commands | Typed translation from window actions to the application client; ephemeral single-use setup review handles |
| Desktop streams | Independent conversation and catalog subscriptions, batching and acknowledgement credit |
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
The application snapshot retains at most 512 source records within a one MiB
payload budget. Each record has its first observation identity and latest
revision. Text chunks amend their original record; tool updates merge by call
identity into complete tool facts, including the original input and later
results. Revising an existing tool or receiving ancillary observations does not
split a streamed reply; a new tool call separates subsequent content. Whole-record
pruning reports a dropped count, including an oversized record that cannot fit.
An evicted active stream keeps only its identity and kind; its remaining chunks
are suppressed until a real role/content boundary, so no arbitrary source suffix
is presented as a complete reply.
Tool input/output/text also retain the application's existing 64 KiB tool-record
cap and its original-size notice. Current status, model, usage, names,
configuration, requests and drafts survive transcript pruning.

Incremental frames replace changed records by identity and carry the owner's
complete retained-id order. A cursor outside retention causes an owner refresh
and explicit reseed; surviving source and DOM identities remain stable. There is
one canonical source store in the application. The WebView retains only its
current presentation and removes records when the owner removes them.

Replies are parsed by pulldown-cmark into typed, allowlisted document nodes.
Headings, lists, code, emphasis and web links are semantic elements. Authored
HTML is literal text, and Markdown images expose alt text without fetching.
Parsing is bounded to 4096 events and 64 nesting levels per reply; more extensive
formatting is shown as complete selectable plain text with an explanation.
Copy reply and Copy conversation use canonical source strings, preserving
Markdown, Unicode and line endings; native selection copies the selected
visible text. Tool disclosures show the original input, output, text and edits,
with explicit failure and truncation information.

The document view keeps article, disclosure and unchanged block nodes stable.
A native selection freezes intersecting message subtrees while other records
continue updating. Clearing selection applies the latest retained revision.
The first visible block anchors scrolling; selection or reading above the end
suspends following. Latest clears selection, flushes held revisions and scrolls
to the current end; scrolling to the end also resumes following. If an owner
bound removes selected source, the selection clears and the gap stays visible.

Web addresses open through one HTTP(S)-only native command after URL validation.
Credentials, controls and other schemes are rejected. The privileged WebView
accepts only its actual platform/configured bundled origin and index document,
including Tauri's empty root path, and denies popup creation. No opener plugin
commands receive capabilities, and its automatic JavaScript link handling is
disabled; no filesystem or arbitrary program opener is exposed. The CSP remains
local-only.

Dependency audit: pulldown-cmark 0.13.4 has default features disabled, including
HTML output and SIMD; this configuration forbids unsafe Rust in the parser.
The maintained Tauri opener 2.7.1 uses the platform's default browser. Its native
platform implementation includes FFI, while the application's entry point accepts
only a validated URL and never an application or path. The lockfile adds 21
packages: parser/unicase, opener/open and platform detection, plus Linux DBus
and supporting packages. Existing locked versions are unchanged.

## Interface

The left rail lists known projects and resident sessions. A summary-only catalog
updates every resident agent's status, queued messages and waiting requests.
Only changed supervision facts publish a catalog update; transcript tokens and
drafts stay on the selected conversation's stream. Each stream allows one
unacknowledged frame, and catalog delivery coalesces at 100 ms. Rail nodes stay
stable as statuses change, preserving keyboard focus; expired row labels are
removed with their session. Attention stays visible as the rail scrolls. One polite announcement
reports waiting requests and disconnected agents without announcing tokens.

Needs your attention opens the exact project, working copy, session and request.
Answers use the shared permission owner; accepted reservations, expired and
cancelled requests cannot be reapplied. Stop, Reconnect and Close in the rail
address the owning session while preserving the selected draft and other agents.
Reconnect restores that owner's provider identity when the provider supports it;
it never sends a prompt or opens a new conversation. Closing remains visible
until the addressed owner settles. Current queue, cancellation and negotiated
image/resume support come from the supervisor. Unknown usage and spending stay
explicit, and reported zero remains distinct from missing information. The conversation uses
native editable text and selectable messages; the contextual panel shows the
working copy, model, usage and permission requests. Open project, New session,
Send, Stop, Allow/Deny, Copy conversation and Close session call real owners.
New session lists installed or configured agents without opening them. Choose model
explicitly invokes the existing zero-turn ACP probe in the selected folder; it
sends no prompt and awaits cleanup, without writing Magistr history. Model values
and labels come from the provider and are filtered by the project policy. Normal
admission validates the selection again. Live configuration and permission requests
share the same policy authority; confirmed model changes update subsequent decisions.
Configuration controls show the accepted state and disable during owner-reported
configuration, work or cancellation.

Conversation names use the session owner's history record and settlement writer.
Reconnect restores the confirmed model from that same context. Saved conversations
reads the existing database without creating it, presents its bounded last-reply
summary, and distinguishes reading from the explicit Resume action. Missing agents,
folders or resume support have specific explanations. A refused provider load keeps
the saved summary open for retry and never starts a fresh context.

Follow-ups reach the resident supervisor's bounded FIFO. Send clears only the exact
matching draft after an addressed acceptance with its real observation sequence;
a rejected send or a newer draft is retained. Cancellation clears the owner's queue
and reports unsent queued messages. Navigation saves drafts only to live catalog
owners, so closing the selected conversation can switch to another resident.

Appearance selects System, Light or Dark. Motion is absent; native reduced-motion
preferences and `--reduce-motion` are honored.

Open project accepts a native folder selection or an entered path. Prepared
projects use the existing session admission authority. Other folders show the
actual setup plan: each file's action, ownership, refusal and exact contents or
diff. Approval applies the retained plan through the administrative writer;
stale bytes or ownership refuse before writes, and subsequent failures retain
per-path receipts. Declined, refused and partial setup never opens an agent.
Review again creates a fresh plan; cancel discards only the review. Setup in a
linked working copy uses its shared project owner. Project identity, settings,
customized files and unrelated shared-file entries remain with their owners.
Folders without Git open as the project's sole source directory; Git-only working-copy creation explicitly requires a repository. Recent histories from linked copies resolve to their shared project. After the last session closes, the same window returns to its chooser.

The desktop opens conversations and prepares projects. The CLI retains
headless runs, initialization, validation and administrative commands; MCP
roles serve workflow and source tools. Roadmap editing, detailed file review and settings remain later desktop work.

## Verification

The separate Cargo workspace has explicit fmt, check, build, Clippy and Nextest
CI jobs on macOS and Linux. Node tests cover composition, addressed sequence
handling, stale navigation and bounded retention. `tests/process.rs` exercises
the actual executable's worker, ACP and both MCP roles and rejects retired
renderer dependencies.

`tests/native_window.py` builds a disposable signed macOS app around the real
binary. It uses native keyboard, pointer and accessibility input with a controlled
ACP peer, isolated projects and state. The fixture preserves the real macOS home for WebKit and isolates application state with XDG and Magistr paths. It exercises independent sessions and
working copies, clean and explicit launch, native folder selection,
reviewed setup (declined, refused and partial), invalid paths/settings,
unavailable agents, streamed replies, concurrent background attention, exact-request
navigation, owner reconnect/stop/close, permissions, cancellation, drafts, native
clipboard, two provider choices, model preview/selection/reconnect, accepted and
cancelled queues, durable names, summary-only history, refused and retried resume,
light/dark/narrow views and orderly Quit. It retains screenshots,
an accessibility tree and a receipt tied to source hashes.

The rich-conversation fixture and native Swift observer add a long multilingual
Markdown/code corpus, literal hostile markup, safe links and failed tool details.
The journey holds native selection while the selected reply, earlier tool and a
separate reply change; it compares exact clipboard source, selected text and
reading position, then exercises Latest and disclosure stability. Its dark,
light and narrow captures and rich observation/accessibility receipts accompany
the ordinary journey evidence.
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
