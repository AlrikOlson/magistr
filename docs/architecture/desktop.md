# Desktop architecture

The desktop is `magistr-desktop`: a Rust Tauri 2.12 host with the operating
system's WebView and semantic HTML/CSS. Tauri owns its app bundle and installer;
`Launch Magistr.command` and `Launch Magistr.cmd` call `scripts/desktop.mjs` to
build the current checkout and open that same executable. Installed users open
Magistr from Applications/Dock on macOS or the Start menu on Windows.

```sh
node scripts/desktop.mjs open
node scripts/desktop.mjs open --dir /path/to/project
node scripts/desktop.mjs package
```

The helper selects the host Rust target and reads Cargo metadata for the output
directory. Tauri bundles the canonical icon and embedded frontend; macOS emits
an app and disk image, Windows a per-user NSIS installer with WebView2 setup.
Windows uses the GUI subsystem while worker, ACP and MCP roles keep their inherited
pipes. Installed child roles continue to use the current executable, with no
checkout-relative runtime assets or separate CLI installation. Node and Cargo
are build tools only. Direct `cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked`
remains a development command. See [development](../development.md) for prerequisites
and artifact locations.

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
| `InteractiveHost` and `Resources` | Provider, workflow attachment, source workspace and their child lifetimes |
| `SessionObservation` | Retained conversation identity/name, messages, complete configuration, status/usage, requests and drafts |
| `ProjectFolder`, `ProjectSetup`, `ScaffoldPipeline` and `AdminService` | Read-only readiness, detected setup plan, exact approved writes, ownership and per-path receipts |
| Desktop commands | Typed translation from window actions to the application client; ephemeral single-use setup review handles |
| Desktop streams | Independent conversation and catalog subscriptions, batching and acknowledgement credit |
| WebView | Native editing, accessible controls, focus, layout and bounded conversation presentation |

The UI never launches a provider or writes a second session registry. Every
frame carries project, canonical workspace and resident session identity. Every
conversation mutation also carries the logical conversation ID in a
`ConversationTarget`; resident navigation and Close address the resident itself. Switching views replaces the subscription; resident work continues.
Close session settles that session. Quit awaits the registry and its project
resources before the native event loop accepts exit. Window visibility preserves
the current view; replacing a selection or dropping the window stream ends its
subscription without closing resident sessions.

The supervisor activates a conversation only after its history, draft writer and
observation are bound. Conversation metadata cannot activate a context. Planning
and ordinary conversations retain separate provider identities, names, messages,
permission requests and drafts when parked. Purpose is required in the existing
history record and restores the planning policy on reopen. No duplicate history
or per-view persistence owns these facts.

Draft and attachment edits resolve the named retained conversation even while it
is parked. Provider commands and Send additionally require that conversation to
be active when dispatched. Preparation and queued commands retain the same target,
so a switch cannot redirect their work. Late provider events and save results
update only their own context. Close flushes every retained draft writer before
releasing its history claims. The catalog projects all retained conversation IDs,
including parked and closing contexts; saved history uses those claims to prevent
concurrent editing or duplicate resume.

A Tauri Channel delivers ordered batches. One batch may be in flight; the next
waits for acknowledgement of the exact sequence after DOM application. Slow views
cannot accumulate token payloads. Navigation intentions are monotonic at both
ends. Late subscriptions and frames cannot replace the selected session.
Each conversation snapshot retains at most 512 source records within a one MiB
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
configuration, requests and drafts survive transcript pruning. Workflow reads
remain in the application's existing project seed, outside this message budget.
Each family retains its latest successful records and any subsequent read failure;
refreshing those records does not reset conversation identity or draft state.
See [projections](projections.md) for the read and retry contract.

Incremental frames replace changed records by identity and carry the owner's
complete retained-id order. A cursor outside retention causes an owner refresh
and explicit reseed; surviving source and DOM identities remain stable. There is
one canonical source store in the application. Activation preserves a monotonically
increasing resident cursor, while each new conversation presentation reseeds
messages and attachment previews. Editor and send state are keyed by the full
conversation target, including pending writes when a conversation is parked. The WebView retains only its
current presentation and removes records when the owner removes them. A successful
draft RPC retains its acknowledged text and revision until an equally current
owner frame arrives, so returning to a parked conversation cannot restore older
text while the frame catches up.

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

The desktop frame gives the conversation a broad reading surface and a compact composer. The left rail groups conversations under their projects, with project opening, new conversations, saved history and Settings outside its scrolling list. RecentProject exposes the same NativeConfig-resolved identity used by session admission; grouping never infers identity from a display name or working-copy path. Stable row nodes keep focus, disclosure state and scroll through catalog updates. Each row has one actions disclosure; unavailable Stop and Reconnect controls do not clutter the list. Violet identifies selection and primary actions; light and dark appearances use the same semantic roles. Details starts closed to give reading and editing the available width. The integrated composer includes file attachment and an agent-settings shortcut. Application appearance and motion live in a separate Settings dialog using the existing preferences owner. A labeled Details control shows requests and planned work before agent settings, working copy and conversation actions. Above 1100 CSS pixels it controls an adjacent pane; at narrower widths it switches between the same conversation and details nodes. No subscription or session is restarted by changing layout. Escape returns from Details to its toggle, and resizing preserves focus in the active surface. Latest reply has its own row so it cannot cover conversation text.

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

Pending files belong to the addressed conversation alongside its text draft. The
native picker and native file drops stage project files through the admitted
workspace's exact `read_source` observation. Clipboard files supply bytes only;
they grant no filesystem access. Staging classifies content from bytes and keeps
its SHA-256 revision. Source files are checked again before send; changed,
missing, oversized or outside-project files refuse without discarding the draft.
Reads run away from the registry loop and settle before project resources close.
Each session admits only one preparation at a time. Stop invalidates that
preparation before it can dispatch, so a slow file check cannot start work after
cancellation. A newly selected attachment has a new identity even when its bytes
match a previous selection; an earlier acknowledgement cannot clear it.

`PromptInput` carries immutable text/image content through the shared command,
session, supervisor queue and provider adapter. Its bounds are one MiB per file,
eight files and eight MiB of attachments, plus one MiB of message text. Text files
become separate labeled text blocks with their exact UTF-8 content; PNG, JPEG and
WebP become image blocks only when the provider negotiated image support.
Other binary files are refused. The adapter encodes already prepared content;
it does not read files mentioned in message text. Once queued, a request's
bytes cannot change with the filesystem.

Each selected stream sends image and text preview data once per new attachment
identity. Subsequent frames carry metadata; the desktop retains only that
presentation data until removal or navigation. A new stream seeds its previews
again, including when the logical conversation changes inside one resident.
Native file drops grant one use to the full selected conversation and stream;
JavaScript receives no path-reading capability.

The current submission outcome is an explicitly sequenced owner fact, separate
from bounded conversation history. It records actual acceptance or refusal with
ordered file identities, names, hashes, kinds and lengths, so attachment payloads do not fill
the transcript. Acceptance clears only the matching text and attachments;
late outcomes leave newer edits intact. Rejection can restore exact bytes into
an empty pending draft. The desktop allows one outstanding send per addressed
conversation and never infers acceptance from an empty editor.

Appearance selects System, Light or Dark and saves through
`DesktopPreferencesService` to the captured user settings path. System follows
the operating system; an explicit Light or Dark overrides that appearance.
Reduced motion is enabled when any of the saved preference, operating-system
preference or `--reduce-motion` requests it. The user setting cannot disable a
stronger request. There are no animations or transitions.

The core `UserConfigDocument` is the shared settings writer for CLI and desktop.
A revision is the exact original file state, including absence. The writer holds
a sibling lock, rereads and compares the file, edits supported TOML keys without
dropping comments or unrelated settings, and durably replaces it. Unreadable,
invalid or concurrently changed settings produce a recoverable error; only an
absent file uses defaults. Loading and saving run away from the async event loop.

Unsent text and attachment selections belong to the existing conversation
history record. `SessionHistory` checkpoints a live record before its first turn
and stores each draft as a child row with a durable revision. Live records have
an update time and no fabricated end time or outcome. A watch-based writer
coalesces edits while one SQLite transaction compares the prior saved revision
and commits the next draft. Views observe editable and saved revisions
separately; a failure retains the newest text and exposes Retry save. Each
completed write also publishes its monotonically increasing attempt, including
same-text retries, so a delayed observation or RPC cannot restore an older
failure. Saved-only edits use the durable revision while no resident writer exists.

Admission restores by canonical project, working copy and logical conversation
identity, then maps to a new resident address. Saved conversations can read and
edit a draft without starting a provider. Text and file selections share one
revision. Files retain immutable bytes, their selection identity and any source
path; source files are checked again before Send, so changed or missing files
can be removed without losing the message. Explicit discard writes an empty
revision. Old saves cannot resurrect it.

Native Quit first freezes the current editors, including open modal dialogs,
and awaits pending file preparation, text and preference writes. Only then does
it settle resident resources. A failed flush restores editing and reports the
failure. Unexpected process exit restores the latest committed revision; it
does not claim durability for a write that never committed.

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

Planned work starts one item or a bounded run in a resident session. HostedWork
acquires the same stable LoopLease for either mode before reading recovery,
writing progress or clearing temporary work. The admitted selection is frozen
from the resolved conversation and retained in the run checkpoint. LoopDriver
owns one control state and joins provider, gate and checkpoint work before
publishing its end; each worker retains the lease if its coordinator is dropped.

WorkRun snapshots and launch refusals are retained outside the bounded message
history. Controls use the resident SessionAddress and exact RunId, so a fresh
provider conversation does not invalidate the run's controls. Pause, step and
stop take effect at item boundaries; Stop in the composer cancels the current
reply. A queued launch has its own pending state until the owner admits or
refuses it. Navigation and reseeding project these facts without inventing a
second execution state.

The desktop opens conversations, starts planned work and prepares projects. The CLI retains
headless runs, initialization, validation and administrative commands; MCP
roles serve workflow and source tools. Roadmap editing and detailed file review remain later desktop work.

## Verification

The separate Cargo workspace has explicit fmt, check, build, Clippy and Nextest
CI jobs on macOS and Linux. Node tests cover composition, addressed sequence
handling, stale navigation and bounded retention. `tests/process.rs` exercises
the actual executable's worker, ACP and both MCP roles and rejects retired
renderer dependencies.

Routine UI checks run selected scenarios under `tests/browser` in headless WebKit.
They load the actual HTML/CSS/JavaScript and isolated observation fixtures; they
never activate applications or post OS input. Current reviewed layout evidence
lives in `docs/evidence/desktop-layout`. These fixtures verify rendering and DOM
interaction, not Rust commands, native editing, OS dialogs or assistive technology.

Native journeys require explicit authorization for foreground control and the
`--allow-foreground` argument. The guard runs before compiling observers,
creating evidence or launching apps. Run the affected journey only when native
integration is the behavior being changed; retain its actual source-bound
receipt without forcing unrelated journeys for ordinary UI changes.

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
When foreground testing is authorized, rebuild and run it:

```sh
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
```

The separate `tests/draft_persistence_journey.py` uses the same native observer
to reopen multiple conversations and working copies, recover files and text,
verify immediate edit-to-Quit, interrupt a process after a durable rapid edit,
force a SQLite write failure and retry, and confirm explicit discard and saved
appearance. It records its own source-bound receipt and reviewed captures.

The separate `tests/work_journey.py` starts actual hosted work with a controlled
provider. It verifies lease refusal in another resident, navigation during
pending pause, one-item stepping, resume, stop after the item, single-item
completion, reply cancellation and orderly Quit in both appearances.

See [recorded native evidence](../evidence/desktop-current/README.md). Screenshot
checks verify its integrity, current background layout captures, contrast, the
selected icon and UI source freshness. Exact freshness of all native integration
sources is a separate explicit `--native-current` check.
Performance checks exercise the production conversation reducer and the native
event-to-accessible-reply receipt. This is not a whole-application release benchmark.
The [foundation decision](desktop-foundation.md) retains the framework comparison.

macOS has been exercised locally. Linux retains its existing CI checks. The
separate desktop-app delivery workflow builds on macOS and Windows, checks the
actual packaged protocol roles and installs/opens/closes the Windows app. It
also runs the process owner's tests on both platforms; a workflow definition
alone is not passing native evidence. Complete native IME observation remains
separate desktop work. Automated Japanese input-source observation must be reported as
observed; composition unit tests and Unicode paste alone do not prove conversion.
