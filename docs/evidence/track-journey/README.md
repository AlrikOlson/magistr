# Terminal browse and read acceptance

Verified on October 3, 2026 (Pacific time) with the production terminal runtime
and real Unix PTYs. The fixture contains 1,000 work items in 82 tracks, exceeding
the native roadmap's 915 stored items at verification. Long work and reasoning
records contain 80 numbered source checkpoints plus prose, lists, code, named
links and a table. Motion is reduced; themes are Catppuccin Mocha and Latte.

The combined cell journey uses actual keyboard bytes and SGR mouse reports:
collapse an unrelated track, hide selected work, search, reveal, copy the named
guide address, open its prerequisite in another track, inspect Dependencies,
read and scroll, open completed history, resize from 120×40 to 80×24 and back,
submit a planning request, hold its reply, browse recorded reasoning while
output arrives, and return. The original source checkpoint, selected item,
unrelated collapse, request history and unsent draft are preserved. Completing
the prerequisite changes Delivery from Ready 1 / Waiting 1 to Ready 2 / Waiting 0.
Deleting the selected item retains its identity and displays Item unavailable.

The graphics PTY negotiates Kitty with a 10×20 pixel cell, uses keyboard and
mouse input, follows the same named cross-track link, resizes, retains the draft
and opens deleted-item details at both sizes. The flat PTY retains the complete
authored document and planning reply, submitted history and draft at both sizes.
Flat mode provides a linear transcript; it has no graphical track list or mouse
interface. Agent replies and native records are scripted at their runtime input
boundaries. These checks exercise the terminal; they do not certify a desktop
window, a particular terminal emulator, or a live screen reader.

Capture provenance:

- `dark-*.txt` and `light-*.txt` are observed cell screens at each journey step.
- `graphics-*.png` reconstruct images decoded from the actual PTY Kitty output.
- `journey-*.png` render the returned runtime model through the production
  cell/Kitty screenshot path at both sizes and themes. Narrow views follow
  the retained pane focus.
- [Wide flat transcript](flat-120x40.txt) and [narrow flat transcript](flat-80x24.txt)
  retain the actual output with terminal controls removed.

Reviewed examples: [selected work](graphics-dark-selected.png),
[cross-track reading](graphics-light-cross-track.png),
[narrow reading](graphics-dark-narrow.png),
[return with draft](graphics-light-returned.png),
[narrow unavailable item](graphics-dark-deleted-narrow.png),
[wide unavailable item](graphics-light-deleted.png),
[held request](dark-held-request.txt), and
[updated readiness](light-updated-readiness.txt).

Measurements used a release build on an Apple M5 Max (arm64), 48 GiB RAM,
macOS 26.5.2. During the request, 200 lines arrive at nominal 5 ms intervals
while input and navigation continue. Each theme has 22 fresh visible input
observations; 1 ms polling includes PTY delivery, decoding and painting.
Already-visible strings are excluded. At the final implementation run:

| Theme | Input p90 | Input maximum | Narrow resize | Wide resize |
| --- | ---: | ---: | ---: | ---: |
| Dark | 19.09 ms | 19.31 ms | 23.72 ms | 52.28 ms |
| Light | 17.18 ms | 17.86 ms | 22.90 ms | 51.56 ms |

Input satisfies the existing `turn_harness` release limits of p90 below 20 ms
and maximum below 40 ms. Resize includes the input thread's 50 ms winsize
polling interval; the existing release resize/coalescing checks remain the
comparison, with no new resize target.

Reproduce from the repository root:

```sh
MAGISTR_JOURNEY_EVIDENCE="$PWD/.magistr/screenshots/track-journey" \
  cargo test --release -p magistr-tui --test track_journey \
  -- --nocapture --test-threads=1
```

The three PTY scenarios run serially for a combined capture; nextest isolates
individual tests in separate processes. Capture output is optional and otherwise
uses temporary files. Live elapsed/activity state is evidence rather than a
deterministic golden. Existing screenshot goldens are unchanged.
