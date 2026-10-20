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

On macOS, ordinary desktop startup reads only PATH from the user's interactive
login shell before starting threads, with a three-second timeout and a 64 KiB
output limit. Explicit inherited command directories keep precedence. The
shared process-group owner settles the shell and its descendants; a failed read
leaves the inherited path intact and logs the failure. Protocol children skip
this bootstrap and inherit the same resolved path as discovery. Automatic
agent selection considers authenticated Claude, installed Codex, Copilot and
implicit Claude in that order; an explicit provider remains authoritative.
Coding-agent availability checks and automatic startup both include Codex.
See [Tauri's macOS packaging guidance](https://tauri.app/distribute/macos-application-bundle/).

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

Project-opening failures retain the selected folder and identify the failed
operation. Short reasons stay visible; multiline or lengthy diagnostics are
selectable under a bounded native disclosure. Retry repeats the normal
readiness/admission path, and pending admission disables duplicate submission.
The ordinary project dialog sizes its form from its content; only the explicitly
sized setup review fills its height. A percentage-height form inside an auto-height
dialog collapses in macOS WebKit, clipping the controls while its backdrop blocks
the window. The background `tests/project_dialog.swift` scenario exercises the
installed system renderer without showing a window or sending OS input.

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
suspends following. The view retains at most 64 recent conversation bookmarks: semantic block/offset anchors, selected-text coordinates and fingerprints, open tool IDs and editor revision/caret/scroll. It retains no message or draft bytes and no extra subscriptions. Returning first renders the current owner snapshot; missing or changed selected source is not restored. Editor coordinates apply only to the same current revision and text fingerprint. Catalog claims remove closed-owner bookmarks, and resize or a temporarily hidden conversation uses the same saved reading anchor. Latest clears selection, flushes held revisions and scrolls
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

Roadmap detail reads resolve prerequisites and dependents from the same complete native graph. Shared workflow policy supplies readiness, including missing and cross-track requirements. Local navigation retains at most 64 item identities and reading positions, then returns to the originating track; it stores no second roadmap.

## Interface

The desktop has an authored visual system: ink and ivory materials, optical typography, distinct document and instrument compositions, and dimensional graphics grounded in current data. This direction follows the owner’s request for substantial design work, including shaders. Use hierarchy, alignment and space to connect each surface to its purpose; borders, accent marks and depth distinguish actual selection, requests, editable documents and measured instruments. Color always accompanies readable text or shape. Bundle Manrope for controls and body text, Newsreader for display headings and quotations, and use system monospace for source and paths. The Latin variable fonts and their SIL licenses are local assets; other scripts use the system fallback. Font loading swaps into the same native text layout without a network request. Inspect rendered dark, light and compact views whenever the system changes.

The desktop visual system uses recessed ink dark surfaces, warm ivory light surfaces, layered document sheets, and violet for selection, primary actions and keyboard focus. The compact rail and open conversation canvas share flat edges. Compact project entries, ruled source folios, quiet document margins and restrained control depth distinguish the working surfaces. Amber denotes waiting requests, mint denotes active or successful work, and rose denotes failure. These meanings always accompany text or shape; unknown usage remains explicit. Compact SVG masks are decorative and control names stay in accessible text. Typography, alignment and spacing distinguish replies, tool activity and requests. Long replies use a bounded 744-pixel document measure (800 pixels in windows at least 1600 pixels wide), larger section headings and separated quotations; authored messages occupy a narrower inset surface. Source blocks remain independently scrollable and selectable. Each code block is a semantic figure with its declared language, a line count, nonselectable gutter and exact-source Copy code action. The Rust document projector retains bounded fenced-language names. The desktop applies lexical color for explicitly named JavaScript/TypeScript, Rust, Python, shell and JSON sources through text nodes, preserving literal markup and exact copying. Coloring scans forward and stops above 64 KiB or 12,000 spans; unknown languages and larger sources remain complete plain text. Transcript still owns selection freezing and reading anchors. A decorative source overview samples at most 120 line-length/indent marks for 24–1,000 lines within 64 KiB; it never contributes selectable source. Large code remains complete while gutters are omitted above 1,000 lines to bound presentation work; selection freezing and reading anchors remain with Transcript. Muted surfaces contain authored messages, code and editable drafts. Work displays the current attempt from its owner as a compact current position and numbered itinerary for runs of at most twenty attempts, never a fabricated completion percentage. Only the current position is emphasized; longer runs use a sparse sequence with explicit gaps and bounded landmarks; previous positions do not claim successful delivery. Ended runs label their last attempt explicitly. Settings presents three appearance previews and the native appearance selector alongside a keyboard reference and motion controls; wide windows put the reference in its own column. All appearance controls read and edit the existing Preferences owner. The agent inspector uses a compact native capacity meter with a reported percentage and exact token count; spending has its own aligned row. The existing clamped usage fraction supplies the fill, and the semantic meter remains available in forced colors. Unknown and reported zero values remain distinct. The contrast gate covers every semantic foreground in both appearances.


Code documents offer horizontal reading or wrapping while preserving original bytes for copying. Document tables retain authored column alignment, semantic column headers, anchored first columns, source-derived dimensions and an independently scrolling region. Unified patches share a bounded line parser with setup review; additions, removals and old/new line positions come from the actual patch. Side-by-side patch reading pairs contiguous changed lines within their hunk and leaves unmatched positions hatched and blank. Paired changes emphasize the differing middle between common Unicode-aware prefixes and suffixes without changing their source text; original unified source remains available. Structured JSON in tool results and permission requests uses the same bounded source-detail renderer, with exact original-source copying. Bounded arrays of primitive-valued records become semantic tables with actual record/field counts and numeric alignment; nested fields remain disclosures. Consecutive tool calls form a connected execution timeline showing reported status, duration and command or location; Finished means the tool ended, not that a verification gate passed. Long replies derive a bounded contents list from their own headings. Wide windows place the outline beside the reply; narrower windows expose it through a compact disclosure that collapses after moving focus to the chosen heading. Authored task-list checks remain noninteractive document content.

The desktop frame gives the conversation a broad reading surface and a compact composer. The left rail groups conversations under their projects, with project opening, new conversations, saved history and Settings outside its scrolling list. RecentProject exposes the same NativeConfig-resolved identity used by session admission; grouping never infers identity from a display name or working-copy path. Stable row nodes keep focus, disclosure state and scroll through catalog updates. Each row has one actions disclosure; unavailable Stop and Reconnect controls do not clutter the list. Project marks derive from their actual names, and branch lines connect their conversations. Filled rows with a violet edge identify selection; light and dark appearances use the same semantic roles. Details starts closed to give reading and editing the available width. Attachment images open an ephemeral full-size inspection dialog from their existing preview URL. Fit, actual-size and bounded 25–400% zoom views preserve image proportions. A filename-labeled vertical contact sheet (horizontal in compact windows) and arrow navigation resolve the current images from the existing AttachmentList; they do not retain another file collection. The inspection background can be checked, light or dark; metadata uses actual file and decoded-image facts, and removal closes the matching inspection. Sent files form a contact sheet with image dimensions and text excerpts derived from the retained previews. No file read or new attachment registry is introduced.

The integrated composer includes file attachment and an agent-settings shortcut. Expand turns the same textarea into a larger writing surface with a centered manuscript measure, a document margin, anchored actions and source-derived Unicode character and line counts, preserving selection, caret and the current draft owner. Escape returns to the compact composer; hidden conversation content is inert while writing is expanded. Application appearance and motion live in a separate Settings dialog using the existing preferences owner. A labeled Details control shows requests and planned work before agent settings and the working copy. One persistent project bar owns the project name, working-copy access and Conversation, Work, Roadmap, Reasoning, Feedback and Sources destinations. Conversation title and current controls occupy a single row below it. Compact windows retain the same destinations in one project row. A persistent toolbar exposes the current work state and waiting requests while Details is closed; WorkPanel supplies the same presentation to both surfaces. Conversation actions live beside the title. Rename keeps its captured ConversationTarget, prevents duplicate pending saves, preserves refused text and waits for the existing command acknowledgement; the title still comes from owner observations. Above 1100 CSS pixels it controls an adjacent pane; at narrower widths it switches between the same conversation and details nodes. No subscription or session is restarted by changing layout. Escape returns from Details to its toggle, and resizing preserves focus in the active surface. Latest reply has its own row so it cannot cover conversation text.

At 900 CSS pixels and below, ProjectRailLayout turns the rail's application actions into a compact top bar. Projects switches between the same navigation and main nodes; it does not create an overlay, another project registry or a new session. The native minimum is read from the window configuration and verified in dark and light captures. Escape, F6 and committed conversation selection reveal the intended surface. A failed selection save reveals the retained draft and its error. Background requests keep a visible count in Projects and the existing polite announcement. Above the threshold the usual rail and conversation are both visible. Short-window spacing keeps source, draft and action controls usable. At the 760-by-550 native minimum, the conversation retains at least 200 pixels of reading height with an ordinary title and draft; long titles clamp to two lines and expose their full text through the native title tooltip. Compact project browsing uses two columns where they fit. At 1600 pixels and above the rail widens to 264 pixels, the reading measure to 800 pixels and an open details pane to 356 pixels.

The opening activity overview derives mutually exclusive working, waiting, idle and disconnected counts from the accepted catalog, excluding closed residents. Compact labeled counts and a proportional strip show the same observations without another activity registry.

The directory and rail reuse projectGroups over the recent-project overview and live session catalog. Keyed rows show actual working folders, open conversations and pending answers. Counts update on accepted catalog observations; closing a conversation removes it from the open count. The alternate project map projects at most eight projects and six conversations per project into the shared WorkSurface. Readable project and conversation pieces use orthogonal membership lines without dependency arrows. Project pieces open the existing resident workspace, or use normal admission when no resident exists; conversation pieces select through the same draft-flush and subscription owner. View controls, keyboard navigation and local arrangement are shared with roadmap and reasoning. Explicit overflow pieces and the complete list keep omitted catalog entries reachable. Saved history uses a compact catalog beside a reading document. Permission sheets retain the exact request and choices. Opening, setup review and appearance dialogs retain their existing writers. The [workspace interaction contract](../design/desktop-workspace.md) and [current iteration archive](../evidence/desktop-iterations/2026-10-15-restart/README.md) describe the current composition and verification scope.

Find a conversation filters the existing catalog by name, project and working-copy path and selects through the same draft-flush/navigation path. Results group by canonical project identity, showing a shared working folder once or explicit row paths for multiple copies. Its ordinary search field and result buttons support arrow-key browsing, preserve recoverable save errors and return focus to a surviving trigger. Platform shortcut labels are shown in the rail and Settings. F6 and Shift F6 visit projects, messages, composer and Details; compact layout changes reveal the intended region. Global navigation shortcuts ignore composition and open modal dialogs.

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
New conversation names the destination project and working copy, then pairs detected-agent panels with native model radio choices without opening them. Optional model discovery and admission have distinct pending states. Late reads are fenced by the current dialog and provider choice; an admitted address is retained until selection finishes, so retrying a navigation failure cannot start another agent. Choose model
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
summary, and distinguishes reading from the explicit Resume action. The desktop projects that same summary through the reply parser into a reading sheet; search excerpts derive plain text from those document nodes, and Copy saved reply retains the original summary bytes. Missing agents,
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
stronger request. SurfaceMotion observes dialog opening and expansion of the existing composer. Each entrance lasts 160 milliseconds, uses only opacity and transform, and runs once. It starts only when the effective reduced-motion state is explicitly false, cancels active effects when that state changes, and releases its observer and animations on page disposal. Closing a surface cancels its effect immediately. Source content, scroll positions and native editing do not animate. CSS contains no independent animation or transition path; the GPU activity sculpture also has no animation loop.

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
edit a draft without starting a provider. Its desktop browser searches the returned records by name, project, agent and summary; filtering never replaces the selected draft. Fixed heading/search and Resume controls surround independently scrolling results and reading surfaces. SavedDraftEditor retains all write authority and cancels obsolete reads when the browser closes; refused saves preserve the current editable message, and pending resume cannot be redirected or dismissed mid-admission. Text and file selections share one
revision. Files retain immutable bytes, their selection identity and any source
path; source files are checked again before Send, so changed or missing files
can be removed without losing the message. Explicit discard writes an empty
revision. Old saves cannot resurrect it.

Native Quit first freezes the current editors, including open modal dialogs,
and awaits pending file preparation, text and preference writes. Only then does
it settle resident resources. A failed flush restores editing and reports the
failure. Unexpected process exit restores the latest committed revision; it
does not claim durability for a write that never committed.

Open project accepts a native folder selection or an entered path. Its chooser keeps the destination and actions visible while a file list and selected change scroll independently. Reading a file is separate from approving it; the count reflects exactly the selected indexes in the retained review. Applying and opening cannot be duplicated or dismissed; cancelling an unfinished read revokes its result. Partial results show each affected path and require a new review. Prepared
projects use the existing session admission authority. Other folders show the
actual setup plan: each file's action, ownership, refusal and exact contents or
diff. Changes, Current file and After setup switch between representations of the same retained review, independently of approval checkboxes. The source view numbers full-file lines and unified-diff hunks from their real coordinates, with syntax color selected from the actual filename; gutters do not alter the selectable source text. Approval applies the retained plan through the administrative writer;
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

Project Work, Roadmap and Reasoning open as full workspace destinations from the
project rail and directory through Open workspace, then a persistent project tab
bar shared with Conversation, Work, Roadmap, Reasoning, Feedback and Sources.
They reuse the selected
project's resident session without admitting another provider. Returning restores
the same conversation and draft. The project rail remains available, marks the
active work destination, and keyboard region navigation includes the work reader.
The reader retains its selection when returning to the conversation and rejects
late reads after closing or changing the resident address.

The project work reader asks the resident session for its complete validated
workflow snapshot on demand. SessionObservation increments workflow_revision
only when records or their provenance change; streaming frames carry this
revision rather than the full project history. ProjectSeed shares the immutable
NativeSnapshot through Arc, so streamed replies and subscription clones do not
copy complete work histories. Only changed workflow reads or actual family
invalidation replace the shared value; duplicate and phase events retain it.
Held observations remain unchanged when a newer snapshot is published. Conversation switches preserve
that owner revision. Saved work uses each execution's stable run identity;
selecting current work follows new objectives, while selecting a saved run stays
on that run. A removed selection remains explicitly unavailable. The reader
keeps drafts in place, rejects late responses
from another address and presents stale, unavailable and missing states from
ReadState. Check output distinguishes commands actually executed from reported
claims and preserves failed attempts beside later passes. Recorded check results
do not certify that current source is unchanged. Complete commands and output
use the shared safe source renderer, with exact copying and retained disclosures.
Task activity preserves owner action order, complete outcomes and produced
artifact references. Files and tools mentioned by an agent remain authored
claims. Web references use the existing native link policy; local references
are selectable and copyable without inventing a filesystem-opening command.
The Roadmap view uses workstream_catalog and workstream_frontier from the shared
workflow contracts against the same complete roadmap. Counts and readiness
arrive with preserved ReadState. The desktop lays out prerequisites beside their
consumers, marks the owner's next-ready item and retains authored checklist
progress. Each primary page contains at most 50 items; the visible neighborhood
is bounded at 100 context pieces and 160 edges. Items with more than six prerequisites use one explicit bundle for their complete prerequisite set; additional omitted context also uses counted bundles. Counts and completed totals come from actual items, and each bundle opens the complete item reader. Canvas geometry contains no second readiness
policy. A persistent inspector retains complete records while the graph stays
visible. Returning restores the originating item and focus. Track selection
keeps closed, ungrouped and missing tracks distinct. Keyboard tabs retain focus
outside refreshed content. Selected item reads map the same owner ReadState to
an optional complete Chunk plus disposable Document trees. Removed records stay
distinct from read failures. The existing Markdown renderer supplies semantic
text, safe links and complete-source fallback when presentation limits are hit.
The reader retains item identity, text selection, track return position and
focus across updates, and ignores late reads after navigation.
The reader never computes a second persisted plan.

Execution search spans current and retained objectives, requirements, task actions,
check output and artifact references. An exact run selection stays on its stable
identity when new work starts. Task, check-attempt and activity views page at 50
matching entries and retain original task positions and receipt identities; a
failed attempt is not removed by a later pass. The check and activity indexes
reuse the same renderers as task details.

Roadmap search includes open, completed and retired records across tracks. Details
show prerequisites, dependents and proposed values beside actual saved values;
viewing a suggestion never applies it. Reasoning reads complete default and named
histories, investigation membership, dependencies and revisions, with project-wide
step references and exact source copying. Branch copies are not counted twice.
The reasoning overview groups the current page by investigation and separates
main reasoning, branch origins and returns. Branch endpoints come from complete
history, so a partial page never invents an origin or conclusion. Only recorded
relations produce edges. Both maps use the same Canvas 2D geometry owner with
semantic DOM pieces and relationship buttons. Distinct attachment points and
orthogonal obstacle-aware paths keep connections clear of items and headings. Same-column relations use top and bottom ports; direction-aware arrowheads follow the final segment. Large prerequisite sets use one bundle instead of overlapping fan-in lines.
The desktop owns pan, zoom and manual arrangement; these never change workflow
order or status. Keyboard pan, zoom, item navigation and arrangement controls
provide alternatives to dragging. Cancellation restores the original position.
Each persistent surface has one resize observer and no animation loop. Default
framing keeps text readable; Fit all explicitly offers an overview for large maps. The clipped drawing area ends above a separate view-control footer, so panned items cannot cover controls. Actual size restores readable framing around the current item.
Selected decisions remain beside the map with complete selectable evidence and
exact copying. Investigation origin/conclusion links preserve the filtered map
and exact focus on return.
Feedback includes full authored context, append-only research and sources, kind
and status filters, and exact references to roadmap, reasoning and other feedback.
An ambiguous unqualified execution reference remains explicitly unresolved.

Record status presents each owner's independent observation, complete counts and
public read diagnostics. A file timestamp means saved records last changed, never
an inferred read time. A failed desktop refresh stays explicit across tabs until
an accepted successful read or project change. Native family read states remain
owned by their existing stores.

Manual tab changes retain selected records and at most five transient reading
bookmarks: scroll, disclosure identities and stamped text-selection coordinates.
No authored text is stored in those bookmarks. The roadmap reader suspends pending
application while hidden, retains its own selected identity and return trail,
and rereads on return. Address and navigation generations reject late responses
and restores. Changed selected text fails its stamp check rather than selecting
substitute text. Roadmap and reasoning filters remain available beside contextual
inspection. At compact sizes the inspector overlays part of the canvas; Back
restores the exact originating control. Unavailable histories clear stale detail
content. Short windows retain a useful reading area.

The [thirty-chunk delivery index](../evidence/desktop-layout/desktop-roadmap-delivery.md)
records the delivered scope, commits and verification limits.

The desktop opens conversations, reads current objectives and tasks, starts planned work and prepares projects. The CLI retains
headless runs, initialization, validation and administrative commands; MCP
roles serve workflow and source tools. Roadmap editing and browsing arbitrary working-copy changes remain later desktop work.

## Verification

The separate Cargo workspace has explicit fmt, check, build, Clippy and Nextest
CI jobs on macOS. The first release qualifies Apple Silicon macOS; Linux and
Windows native builds are outside its required CI. Node tests cover composition, addressed sequence
handling, stale navigation and bounded retention. `tests/process.rs` exercises
the actual executable's worker, ACP and unified MCP endpoint and rejects retired
renderer dependencies.

Storybook owns the named desktop review states in `stories/*.stories.mjs`.
Its HTML renderer embeds the actual production document in a fresh iframe:
the app currently owns document-scoped controllers, so switching stories must
dispose their entire document and native bridge simulation. The shared
`tests/browser/observations.mjs` supplies isolated native observations to both
Storybook and interaction tests. Stories arrange and assert visible state;
`storybook.spec.mjs` consumes Storybook's generated index, verifies identity
and state, and captures the supported viewport/appearance matrix. This catalog
does not implement a second application or claim to exercise Rust commands.

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

macOS has been exercised locally. The separate desktop-app delivery workflow
builds the Mac app and installer, checks the packaged protocol roles and runs
the process owner's tests on macOS. Native Linux and Windows jobs are excluded
from first-release qualification. Recorded passing runs remain the evidence;
a workflow definition alone does not establish native behavior. Complete native IME observation remains
separate desktop work. Automated Japanese input-source observation must be reported as
observed; composition unit tests and Unicode paste alone do not prove conversion.