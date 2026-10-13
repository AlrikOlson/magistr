# Current native desktop evidence

The real Tauri window is exercised with disposable Git projects, working copies,
a signed app bundle and a controlled ACP peer. Application state uses temporary XDG and Magistr directories; the real macOS home is preserved for WebKit's renderer. The screenshots contain the live
window; no annotations or simulated terminal rendering are added.

The receipt records keyboard and pointer actions, accessible controls, Unicode
and multiline editing, clipboard round trips, permissions with requested action
details, cancellation, retained drafts, separate sessions and working copies,
and ordered native Quit. It closes the last session and reopens its project before
the main conversation journey. The chooser opens without a project or provider; native folder selection, refused and partial setup, cancellation, and recovery all use the actual application owners. Close and Quit must save all six admitted histories,
release the project owner and leave no provider process. Reopening the source endpoint proves
its owner can be admitted again.

Light, dark and narrow conversation screenshots, chooser appearances, setup review, partial results, unavailable-agent guidance and accessibility trees are reviewed
against their hashes in `review.json`. The receipt names interface source hashes.
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
