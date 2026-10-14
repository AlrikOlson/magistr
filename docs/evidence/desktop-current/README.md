# Current native desktop evidence

The real Tauri window is exercised with disposable Git projects, working copies,
a signed app bundle, a controlled ACP peer and a local streaming API fixture. Application state uses temporary XDG and Magistr directories; the real macOS home is preserved for WebKit's renderer. The screenshots contain the live
window; no annotations or simulated terminal rendering are added.

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

The attention capture shows concurrent waiting and working agents with the
selected draft. Light, dark and narrow conversation screenshots, chooser appearances, setup review, partial results, unavailable-agent guidance, agent/model selection, named conversation controls, saved summaries and refused resume are reviewed
against their hashes in `review.json`. The receipt names interface and shared session-owner and observer source hashes.
The screenshot gate rejects changed source or images until the native journey
and review are renewed. It also checks text/control contrast, the selected icon,
reduced motion and the current module index. This evidence does not establish
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
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop
```

Review the resulting `.magistr/desktop-native` images, then copy them and their
receipt here and update the review hashes. `scripts/website-captures.sh` uses
these reviewed native images for the local website.
