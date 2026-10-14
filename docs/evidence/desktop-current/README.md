# Recorded native desktop evidence

These are the last reviewed native integration captures, with their actual
source versions retained in each receipt. Background UI verification now lives
in [desktop-layout](../desktop-layout/README.md). Native journeys control the
foreground desktop and require explicit authorization and `--allow-foreground`;
do not use them as the default check for an ordinary UI edit.

The real Tauri window is exercised with disposable Git projects, working copies,
a signed app bundle, a controlled ACP peer and a local streaming API fixture. Application state uses temporary XDG and Magistr directories; the real macOS home is preserved for WebKit's renderer. The screenshots contain the live
window; no annotations or simulated terminal rendering are added.

The 14 October 2026 captures exercise the current debug executable in a
disposable signed macOS app bundle. All three native journeys use the same source
hashes. These 32 reviewed captures verify the conversation-first composition,
conversation ownership and bounded work controls. Persistent navigation stays
within the window while its collections scroll. At 800 × 680, Details and the
conversation share one working surface; opening Details and pressing Escape
preserve the draft and files and restore keyboard focus. Latest reply occupies
its own row without covering transcript text. Installer and Launch Services
checks are separate packaging evidence.

The receipt records keyboard and pointer actions, accessible controls, Unicode
and multiline editing, clipboard round trips, permissions with requested action
details, cancellation, retained drafts, separate sessions and working copies,
and ordered native Quit. Background supervision exercises a waiting request beside
another working agent, exact request focus, independent Stop/Reconnect/Close,
cancellation reservations and retained drafts. Catalog observation launches no
provider. Reconnect restores one provider identity and creates only one replacement
process; the same logical conversation remains. Agent selection explicitly previews advertised models without sending a prompt. Confirmed model changes survive a failed turn and reconnect. Accepted follow-ups clear only their matching draft; Stop discards the queued turn and preserves newer unsent text. Conversation names follow their owner through close and exact saved resume. It closes the last session and reopens its project before
the main conversation journey. The chooser opens without a project or provider; native folder selection, refused and partial setup, cancellation, and recovery all use the actual application owners. Saved history distinguishes resumable conversations from summaries whose agent cannot resume. A provider-refused load stays in the history dialog for retry and never creates a replacement conversation. The journey also starts an OpenAI conversation through the local fixture. Close and Quit must save all seven admitted histories,
release the project owner and leave no provider process. Reopening the source endpoint proves
its owner can be admitted again.

The rich-conversation journey keeps native multilingual selection fixed while
its original reply, an earlier tool and a separate reply update. It verifies exact
source bytes for reply, tool and conversation copying, clears held selection with
Latest, follows new replies and keeps a scrolled reading anchor stable. A controlled
web link opens by keyboard in the default browser; the observer verifies the exact
local destination and closes only that test tab. Authored HTML stays literal,
unsafe links stay text and remote Markdown images make no request. The rich
accessibility and observation records retain those measurements alongside the
light, dark and narrow captures.

The attachment journey selects real project files in the native picker, cancels
selection, pastes an image from the native clipboard and drops a native file URL.
It checks exact PNG bytes and complete CRLF/Unicode text at the ACP peer,
revalidates changed and missing files, refuses oversized or unsupported content,
and preserves pending files through switching, Stop and negotiated image refusal.
The file list exposes previews and removal in light, dark and narrow windows.
Selection IDs distinguish an identical file chosen again from an earlier request;
source preparation participates in the addressed session's cancellation.

The draft recovery journey reopens four named conversations across two working
copies and two providers. It reads and edits saved text without starting an
agent, recovers source and clipboard files, and confirms changed or missing
source files refuse without losing the draft. It exercises a final edit followed
immediately by native Quit, interrupted exit after a saved rapid revision,
database contention with continued editing and Retry save, a refused Quit that
keeps the unsaved message editable, explicit discard, and saved appearance and
reduced motion. Normal Quit also checks that every logical
conversation retains exactly one spending record. Its separate receipt records exact source,
binary and capture hashes and verifies that recovery sends no unintended prompt.

The work-control journey starts a bounded run, requests pause during an item,
steps one item from a paused boundary, resumes and finishes the current item
before stopping. A competing launch from another resident is refused; returning
to the owner retains its run and controls. Next completes one item, stopping a
reply remains separate from stopping work, and a new conversation retains
completed work. Native Quit leaves no provider process. `work-receipt.json`
records 25 checks and the light and dark captures reviewed in `review.json`.

The attention capture shows concurrent waiting and working agents with the
selected draft. Light, dark and narrow conversation screenshots, chooser appearances, setup review, partial results, unavailable-agent guidance, agent/model selection, named conversation controls, saved summaries and refused resume are reviewed
against their hashes in `review.json`. The receipt names interface and shared session-owner and observer source hashes.
The screenshot gate validates these recorded images against their reviews and
validates current UI layout through the separate headless WebKit receipt. It
reports when native integration sources have changed; `--native-current` on
`desktop-visual-gate.py` requires exact native freshness explicitly. The gate
also checks text/control contrast, the icon, reduced motion and module index. This evidence does not establish
pixel equality across operating systems.

Runs have observed Japanese Kana `ミニくらみ` committed as an unsent draft and
have also reported Japanese while delivering Latin preedit. The current `ime.json`
and receipt state which result was observed. `ime-observed-receipt.json` retains
an earlier positive run with its exact source hashes; `ime-unavailable.json`
preserves a failed activation attempt. The observer tests Return commit only after observing
native preedit; a reported source alone is not proof of conversion. Reliable
activation, candidate panels, Romanization, Chinese and Korean coverage remain
in the native IME observer roadmap item. Production composition tests cover both
conversion-event orders separately.

The response gate measures production batch application and checks the native
keyboard-to-accessible-reply observation with a two-second bound. The receipt
identifies the measured build; it is not a whole-application release benchmark.

Regenerate from the repository:

```sh
cargo build --manifest-path crates/magistr-desktop/Cargo.toml --locked
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
python3 crates/magistr-desktop/tests/draft_persistence_journey.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
python3 crates/magistr-desktop/tests/work_journey.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
```

Review the resulting images in `.magistr/desktop-native`, `.magistr/desktop-native-drafts`
and `.magistr/desktop-work`, then copy them and their receipts here and update the review hashes. `scripts/website-captures.sh` uses
these reviewed native images for the local website.
