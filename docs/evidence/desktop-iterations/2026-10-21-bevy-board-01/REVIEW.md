# Bevy board, iteration 1

Task: inspect and use the real Bevy 0.20.0 board.

- `desktop-empty.png` — opened at full 720 × 700 resolution. The game title, turn and numbered cells are legible. The bottom row is clipped at the canvas boundary and keyboard instructions are out of view. **Fail.**
- The browser run stopped before its result capture: rapid key presses were sent without waiting for Bevy to process each move. The next run will wait for each actual board change.

This iteration is incomplete. No compact or completed-game visual claim is made. Headless WebKit only; no native OS or assistive-technology test. The fixed paper appearance is the only game theme.
