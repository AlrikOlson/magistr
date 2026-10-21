# Rematch iteration 02

Task: finish a game, read its result, and start another round without losing the score. Every frame below was opened at original resolution and inspected in full. The game has one fixed paper appearance. Capture times, viewport sizes, WebKit version and source hashes are in `browser-checks.json`.

| Frame | Observations | Verdict |
| --- | --- | --- |
| desktop-start.png | At 720 × 700, assignment and zero scores precede the empty nine-cell board. Controls and instructions fit; restart is visibly disabled. | Pass |
| desktop-result.png | Computer O's diagonal win is visible, the result and score agree, and Play again is available below the board. No overlap or clipping. | Pass |
| desktop-rematch.png | The human is now O, computer X occupies the center, and the earlier score remains visible. The same stable geometry makes the change easy to see. | Pass |
| compact-start.png | At 320 × 700, all nine cells and the instructions fit with readable type and comfortable targets. | Pass |
| compact-result.png | The winning line, result, score and restart control remain readable without horizontal scrolling. | Pass |
| compact-rematch.png | The changed role, opening X and retained score all fit. Keyboard instructions remain visible below the button. | Pass |

Layout, hierarchy, spacing, type, contrast and control placement pass across the six states. There are no graph edges in the game. The playthrough verifies real Rust/Bevy state, pointer input, keyboard moves, terminal-input protection and rematches. Native OS input and assistive technology remain untested. The later lint-only let-chain change does not alter rendering; the rebuilt game passed the same browser playthrough.
