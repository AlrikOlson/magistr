# Bevy board, iteration 3

Task: play the board and inspect the complete frame.

Every image was opened at original resolution.

| Capture | Observation | Verdict |
| --- | --- | --- |
| desktop-empty.png | Nine numbered cells and turn text are clear, but the keyboard instruction remains outside the canvas. | Fail |
| desktop-win.png | The winning row is highlighted and X wins is readable. The instruction is still clipped. | Fail |
| compact-empty.png | Board, title, turn and instruction fit the 320-pixel width. The en dash renders as a missing glyph. | Fail |
| compact-win.png | Winning row and result fit; the same instruction glyph is missing. | Fail |

The browser playthrough passed at both sizes: each observed move advanced the board and further input could not change a finished game. Visual review remains failed. Replace the viewport-unit/max-size combination with the board's definite dimensions and use the font's supported keyboard-label characters. Headless WebKit only; no native OS or assistive-technology claim.
