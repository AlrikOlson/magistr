// Explanations belong to the website. Recorded actions remain the source of truth.
// Every published action needs an entry; do not fall back to a raw tool response.
export const explanations = {
  "action-1": {
    "title": "Start with a complete little game",
    "summary": "Build tic-tac-toe with an opponent, a playable Bevy board, and another round when the game ends.",
    "details": [
      [
        "The plan",
        "Keep the rules separate from the screen so the same game can be tested and played."
      ]
    ]
  },
  "action-2": {
    "title": "Plan the rules first",
    "summary": "The game needs to know whose turn it is, which moves are allowed, and when someone has won.",
    "details": []
  },
  "action-3": {
    "title": "Give the opponent a clear goal",
    "summary": "The computer should never lose, whether it plays X or O.",
    "details": [
      [
        "How to check it",
        "Try every legal human reply against the opponent."
      ]
    ]
  },
  "action-4": {
    "title": "Plan a board you can play",
    "summary": "Build nine squares in Bevy, with clicks and number keys for moves and text showing whose turn it is.",
    "details": []
  },
  "action-5": {
    "title": "Plan what happens after a round",
    "summary": "Keep the score, clear the board, and alternate who starts. This part needs both the board and the opponent.",
    "details": []
  },
  "action-6": {
    "title": "Start with legal moves and turns",
    "summary": "The agent starts the rules first. Both the opponent and the Bevy board will use them.",
    "details": [
      [
        "Done when",
        "Invalid moves leave the game alone, and wins and draws are recognized."
      ]
    ]
  },
  "action-7": {
    "title": "Make the rules work concrete",
    "summary": "The agent saves a task for turns, allowed moves, and game endings.",
    "details": [
      [
        "Why save it",
        "The task gives the code changes and their checks a shared place in the project."
      ]
    ]
  },
  "action-8": {
    "title": "Begin writing the rules",
    "summary": "The rules task is now in progress. The next step is to create the Rust project and its board logic.",
    "details": []
  },
  "change-58": {
    "title": "Create the Rust project",
    "summary": "The agent writes `Cargo.toml`, which describes the game package and how Rust should build it.",
    "details": [
      [
        "Starting small",
        "The rules can run without opening a Bevy window."
      ]
    ]
  },
  "change-59": {
    "title": "Put the game rules in place",
    "summary": "The new `src/lib.rs` handles turns, legal moves, wins, and draws.",
    "details": [
      [
        "Checks included",
        "Tests cover occupied squares, all eight winning lines, a full board, and moves after a game ends."
      ]
    ]
  },
  "action-9": {
    "title": "All five rules tests pass",
    "summary": "The tests confirm that legal moves work and invalid moves leave the board and turn unchanged.",
    "details": [
      [
        "Also checked",
        "Wins, draws, and refusing moves after the game ends."
      ]
    ]
  },
  "action-10": {
    "title": "The rules code is formatted consistently",
    "summary": "Rust's formatter checks the source and reports no changes needed.",
    "details": []
  },
  "action-11": {
    "title": "Run the rules tests once more",
    "summary": "All five tests passed, but that first run also created `Cargo.lock`, the file that saves the game's dependency versions.",
    "details": [
      [
        "Why Magistr stopped",
        "The project changed during the check, so Magistr won't mark the roadmap item finished yet."
      ],
      [
        "Next",
        "Run the tests again with the saved versions in place."
      ]
    ]
  },
  "action-12": {
    "title": "Save the rules work",
    "summary": "The implementation task is marked finished. The roadmap item still needs the rules check to run again.",
    "details": [
      [
        "The distinction",
        "Finishing a task does not automatically finish its roadmap item."
      ]
    ]
  },
  "action-13": {
    "title": "Plan how to choose the opponent",
    "summary": "The next task compares a short set of move rules with an opponent that looks ahead.",
    "details": [
      [
        "The goal",
        "Choose an approach that can handle every legal human reply."
      ]
    ]
  },
  "action-14": {
    "title": "Start the opponent task",
    "summary": "The agent begins the comparison before committing to an approach.",
    "details": []
  },
  "action-15": {
    "title": "Would a few simple rules be enough?",
    "summary": "Try winning immediately, blocking an immediate loss, then taking the center or a corner.",
    "details": [
      [
        "The question",
        "Can this miss a move that creates two winning threats? An experiment will check."
      ]
    ]
  },
  "action-16": {
    "title": "The rules pass with the saved versions",
    "summary": "The agent reruns the five tests with `Cargo.lock` in place. All five pass.",
    "details": [
      [
        "What changed",
        "The result now belongs to the saved project, so the rules can be marked complete."
      ]
    ]
  },
  "action-17": {
    "title": "The rules are finished",
    "summary": "The roadmap now marks rules and turns complete. The board and opponent can build on this shared code.",
    "details": []
  },
  "action-18": {
    "title": "Start building the opponent",
    "summary": "The rules are ready, so the agent starts the next roadmap item: an opponent that cannot lose.",
    "details": []
  },
  "change-60": {
    "title": "Write an experiment before choosing",
    "summary": "The agent adds a program that tries every legal human reply against the simple opponent.",
    "details": [
      [
        "What it records",
        "If the human wins, keep the sequence of moves that caused it."
      ]
    ]
  },
  "action-19": {
    "title": "The experiment's own assumption fails",
    "summary": "The experiment expected to find a losing game for each side. It stops when it finds none for X.",
    "details": [
      [
        "What this means",
        "The experiment needs to report what it finds, without assuming both sides must lose."
      ],
      [
        "Next",
        "Correct that assertion and run the comparison again."
      ]
    ]
  },
  "action-20": {
    "title": "The simple opponent loses as O",
    "summary": "The corrected experiment finds no loss as X, but finds a winning sequence for the human when the computer plays O.",
    "details": [
      [
        "The moves",
        "X: 1, O: 5, X: 8, O: 3, X: 7, O: 9, X: 4. X wins."
      ]
    ]
  },
  "action-21": {
    "title": "One blocked threat still leaves another",
    "summary": "After X takes square 7, X can win through either square 4 or square 9. O can only block one.",
    "details": [
      [
        "Decision",
        "Use minimax: look ahead through the remaining games before choosing a move."
      ],
      [
        "Why it fits",
        "Tic-tac-toe has only nine squares, so checking those possibilities is practical."
      ]
    ]
  },
  "change-61": {
    "title": "Add an opponent that looks ahead",
    "summary": "The agent adds minimax to `src/lib.rs`. It considers the remaining games before choosing its move.",
    "details": [
      [
        "Check the promise",
        "New tests try every legal human reply with the computer playing either side."
      ]
    ]
  },
  "action-22": {
    "title": "No losses against any legal human reply",
    "summary": "All eight tests pass. The opponent search finds only wins and draws.",
    "details": [
      [
        "Computer plays X",
        "82 wins, 10 draws, 0 losses."
      ],
      [
        "Computer plays O",
        "368 wins, 153 draws, 0 losses."
      ]
    ]
  },
  "action-23": {
    "title": "Save the choice and its reason",
    "summary": "The agent records the decision to use minimax, supported by the losing example and the passing tests.",
    "details": [
      [
        "Keep one set of rules",
        "The Bevy board will call the same Rust code these tests checked."
      ]
    ]
  },
  "action-24": {
    "title": "Close the opponent investigation",
    "summary": "The question, experiment, and losing example now lead to the saved decision: use minimax for either side.",
    "details": [
      [
        "Why keep the trail",
        "A later session can see why the simpler opponent was rejected."
      ]
    ]
  },
  "action-25": {
    "title": "Confirm the original comparison still holds",
    "summary": "The simple opponent still loses as O and has no losing game as X. The experiment keeps that contrast available alongside the chosen approach.",
    "details": []
  },
  "action-26": {
    "title": "The opponent is finished",
    "summary": "The roadmap marks the computer opponent complete. Its moves are legal and it has been checked against every legal human reply.",
    "details": []
  },
  "action-27": {
    "title": "Finish the opponent task",
    "summary": "The comparison, implementation, and checks stay together in the completed task.",
    "details": []
  },
  "action-28": {
    "title": "Move on to the Bevy board",
    "summary": "The agent starts the playable board. The rules are ready; now a person needs a way to make moves and see the result.",
    "details": []
  },
  "action-29": {
    "title": "Plan the screen and controls",
    "summary": "The board task covers nine visible squares, turn and result text, and input through clicks and number keys.",
    "details": []
  },
  "action-30": {
    "title": "Start building the screen",
    "summary": "The agent begins connecting Bevy's display and controls to the existing game rules.",
    "details": []
  },
  "change-62": {
    "title": "Add Bevy to the project",
    "summary": "The project file now includes Bevy 0.20.0 and the pieces needed to build the game for a browser.",
    "details": [
      [
        "Keep rules easy to check",
        "The rules tests still run without building the visual game."
      ]
    ]
  },
  "change-63": {
    "title": "Draw the board and connect input",
    "summary": "The agent writes `src/main.rs`: the Bevy window, nine squares, labels, and controls.",
    "details": [
      [
        "Shared behavior",
        "Clicks and number keys send moves to the same rules code."
      ]
    ]
  },
  "action-31": {
    "title": "The board doesn't all fit yet",
    "summary": "The first Bevy screen cuts off the bottom row and the keyboard instructions.",
    "details": [
      [
        "What needs fixing",
        "The layout hasn't reserved enough height for the square board."
      ],
      [
        "Next",
        "Give the board an explicit size, then check the screen again."
      ]
    ]
  },
  "action-32": {
    "title": "Check off the rules requirements",
    "summary": "The roadmap's checklist now records the tested behavior: valid turns, wins and draws, and no moves after the game ends.",
    "details": []
  },
  "action-33": {
    "title": "Check off the opponent requirements",
    "summary": "The roadmap records that the opponent plays either side, survives every legal human reply, and has a saved explanation for its approach.",
    "details": []
  },
  "action-34": {
    "title": "The board passes a browser playthrough",
    "summary": "The game accepts keyboard moves and stops accepting moves when the round ends.",
    "details": [
      [
        "Sizes checked",
        "Both the desktop and narrow board pass."
      ]
    ]
  },
  "action-35": {
    "title": "Clicking an occupied square changes nothing",
    "summary": "The pointer test places X in a square, clicks that square again, and confirms the board stays the same.",
    "details": []
  },
  "action-36": {
    "title": "Review the screen as well as the rules",
    "summary": "The recorded visual review finds the full board, result, winning row, and keyboard instructions readable at both sizes.",
    "details": [
      [
        "Checked separately",
        "A passing rules test alone would not show whether the screen was clipped."
      ]
    ]
  },
  "action-37": {
    "title": "The whole board now fits",
    "summary": "The agent keeps a fixed board size after reviewing the corrected desktop and narrow screens.",
    "details": [
      [
        "What was fixed",
        "The bottom row and instructions fit, and the keyboard label uses characters the font can display."
      ]
    ]
  },
  "action-38": {
    "title": "Keep the explanation for the layout fix",
    "summary": "The board-size investigation is closed and linked to the saved conclusion.",
    "details": [
      [
        "For the next session",
        "The record explains why this board uses a definite size."
      ]
    ]
  },
  "action-39": {
    "title": "Check off the playable board",
    "summary": "The board's checklist now records working input, correct game text, and a browser build.",
    "details": []
  },
  "action-40": {
    "title": "The playable board is finished",
    "summary": "The roadmap marks the Bevy board complete. It has passed input checks and a separate visual review.",
    "details": []
  },
  "action-41": {
    "title": "Finish the board task",
    "summary": "The agent saves the completed board work with its browser checks and visual review.",
    "details": []
  },
  "action-42": {
    "title": "Make it a game you can play again",
    "summary": "The agent starts the final roadmap item: connect the computer, keep score, and begin another round.",
    "details": [
      [
        "Ready to build on",
        "Both the opponent and the playable board are complete."
      ]
    ]
  },
  "action-43": {
    "title": "Plan the full round of play",
    "summary": "The task covers computer replies, scoring a finished round once, and letting the other player start next time.",
    "details": []
  },
  "action-44": {
    "title": "Start connecting the full game",
    "summary": "The agent begins adding scores and rematches to the board and opponent.",
    "details": []
  },
  "change-64": {
    "title": "Keep track of the match",
    "summary": "The rules file now tracks the human's side and the score across rounds.",
    "details": [
      [
        "Prevent mistakes",
        "A finished round adds its score once, and a new round alternates who starts."
      ]
    ]
  },
  "change-65": {
    "title": "Add scores and a Play again button",
    "summary": "The Bevy screen now shows who you are, lets the computer reply, and offers another round when the game ends.",
    "details": [
      [
        "Same game underneath",
        "The screen uses the match logic in `src/lib.rs`."
      ]
    ]
  },
  "action-45": {
    "title": "The full game passes ten tests",
    "summary": "Rules, opponent moves, scoring, and rematches pass their checks.",
    "details": [
      [
        "Still no losses",
        "The opponent also passes every legal human reply as X and O."
      ]
    ]
  },
  "action-46": {
    "title": "The browser playthrough stops waiting",
    "summary": "The test waits 30 seconds for the expected game state and times out.",
    "details": [
      [
        "What we know",
        "This run did not finish. A timeout alone doesn't tell us whether the game or the test is wrong."
      ],
      [
        "Next",
        "Check which move or screen update the test was waiting for, then try again."
      ]
    ]
  },
  "action-47": {
    "title": "The playthrough now finishes",
    "summary": "The next browser run passes with computer replies, scoring, and alternating rematches.",
    "details": [
      [
        "Also checked",
        "A finished game ignores further moves. The checks pass at 720px and 320px wide."
      ]
    ]
  },
  "action-48": {
    "title": "The game builds for the browser",
    "summary": "Rust successfully builds the Bevy game for the browser. The build uses the saved dependency versions.",
    "details": []
  },
  "action-49": {
    "title": "Rust flags a small code cleanup",
    "summary": "Clippy, Rust's code checker, finds two nested `if` statements that can be combined.",
    "details": [
      [
        "Why this check fails",
        "This project requires those warnings to be fixed before completion."
      ],
      [
        "Next",
        "Combine the conditions and rerun the code checks."
      ]
    ]
  },
  "action-50": {
    "title": "The game still passes after cleanup",
    "summary": "All ten rules, opponent, scoring, and rematch tests pass again.",
    "details": []
  },
  "action-51": {
    "title": "The cleanup passes",
    "summary": "The formatter and Rust code checker now report no problems.",
    "details": []
  },
  "action-52": {
    "title": "Rebuild the checked game",
    "summary": "The browser build passes again after the code cleanup.",
    "details": []
  },
  "action-53": {
    "title": "Play through the checked build again",
    "summary": "The final browser run passes at desktop and narrow sizes, including computer replies, one score per round, and alternating starts.",
    "details": []
  },
  "action-54": {
    "title": "Review the final screens",
    "summary": "The recorded review covers the start, a computer win, and a rematch at desktop and narrow sizes.",
    "details": [
      [
        "What was checked",
        "The board, scores, player labels, result, and controls are readable and fit on screen."
      ]
    ]
  },
  "action-55": {
    "title": "Check off the last requirements",
    "summary": "The final checklist records computer replies, scoring once, and a new round that keeps the score and alternates who starts.",
    "details": []
  },
  "action-56": {
    "title": "All four pieces are finished",
    "summary": "The roadmap marks the last item complete. Rules, opponent, board, and rematches now form a checked, playable game.",
    "details": []
  },
  "action-57": {
    "title": "Leave the finished work ready to revisit",
    "summary": "The final task is complete. The project keeps its plan, decisions, changes, failed attempts, and passing checks.",
    "details": [
      [
        "What another session can use",
        "The next agent can read what was built and why before making a change."
      ]
    ]
  }
};

Object.assign(explanations, {
  "message-1": {
    "title": "Ask for a Bevy game",
    "summary": "You change course: a small Bevy game instead of the earlier plan. This is where the project starts.",
    "details": []
  },
  "message-2": {
    "title": "Agree on what to build",
    "summary": "Codex proposes a playable board, a computer opponent, and rematches. The walkthrough it mentions is this page.",
    "details": []
  },
  "message-3": {
    "title": "Make the work concrete",
    "summary": "The agent identifies the game behavior it will build and check.",
    "details": []
  },
  "message-4": {
    "title": "Report what the experiment found",
    "summary": "The first opponent looks promising when it starts. Playing second still needs checking.",
    "details": []
  },
  "message-5": {
    "title": "Specify the Bevy version",
    "summary": "You add a requirement while the agent is working: use Bevy 0.20.0.",
    "details": []
  },
  "message-6": {
    "title": "Apply your follow-up",
    "summary": "Codex acknowledges the version and moves on to the interface.",
    "details": []
  },
  "message-7": {
    "title": "Report the first working browser build",
    "summary": "The game compiles for the browser, and the opponent has survived every legal human reply.",
    "details": []
  },
  "message-8": {
    "title": "Explain a visual problem",
    "summary": "Playing the game works, but its instructions are clipped. The agent reports the problem before fixing it.",
    "details": []
  },
  "message-9": {
    "title": "Report the finished game features",
    "summary": "Scores and rematches are working. The agent is reviewing the walkthrough and its published data.",
    "details": []
  },
  "message-10": {
    "title": "Prepare the final review",
    "summary": "The agent checks that the recording includes the decisions, failures, fixes, and completed work.",
    "details": []
  },
  "message-11": {
    "title": "Check the publication",
    "summary": "The game’s four pieces are complete. The agent checks the public files for private material.",
    "details": []
  },
  "message-12": {
    "title": "Fix a keyboard issue",
    "summary": "Browser testing catches a menu that does not close with Escape. Codex fixes it and reports what changed.",
    "details": []
  },
  "message-13": {
    "title": "Finish the browser checks",
    "summary": "Playback, details, search, and recovery pass at desktop and phone sizes in both themes.",
    "details": []
  },
  "message-14": {
    "title": "Explain the opponent decision",
    "summary": "The agent sums up the losing fork and the tested replacement while reviewing the phone layout.",
    "details": []
  },
  "message-15": {
    "title": "Catch a narrow-screen overflow",
    "summary": "One tab extends beyond the player at 320 pixels. The agent fixes it before shipping.",
    "details": []
  },
  "message-16": {
    "title": "Prepare the commit",
    "summary": "Visual fixes are complete. The same commit now needs to pass the repository’s required checks.",
    "details": []
  },
  "message-17": {
    "title": "Report the commit",
    "summary": "The changes are committed. Codex explains which checks must finish before pushing to main.",
    "details": []
  },
  "message-18": {
    "title": "Report the first CI results",
    "summary": "Security and screenshot checks pass. The remaining build jobs are still running.",
    "details": []
  },
  "message-19": {
    "title": "Confirm the website checks passed",
    "summary": "The website passes its CI checks; the wider repository checks are still running.",
    "details": []
  },
  "message-20": {
    "title": "Keep you updated while CI runs",
    "summary": "Rust and desktop builds are still in progress. The agent reports that no failure has appeared.",
    "details": []
  },
  "message-21": {
    "title": "Share the local preview",
    "summary": "You can inspect the walkthrough while the build checks continue.",
    "details": []
  },
  "message-22": {
    "title": "Report successful builds",
    "summary": "Rust compilation and macOS packaging pass. Tests and desktop verification continue.",
    "details": []
  },
  "message-23": {
    "title": "Report test progress",
    "summary": "The compiled tests are running. The agent is waiting for the results before pushing.",
    "details": []
  },
  "message-24": {
    "title": "Narrow down what remains",
    "summary": "Three test workers have passed. The final worker is saving its cache.",
    "details": []
  },
  "message-25": {
    "title": "Finish the Rust tests",
    "summary": "All four Rust test groups pass. Desktop compilation is the remaining prerequisite.",
    "details": []
  },
  "message-26": {
    "title": "Build the installer",
    "summary": "Desktop checks move on to packaging the app and installer.",
    "details": []
  },
  "message-27": {
    "title": "Wait for the installer",
    "summary": "The installer is still building; the completed checks have passed.",
    "details": []
  },
  "message-28": {
    "title": "Check the packaged app",
    "summary": "The installer build passes. GitHub checks the packaged targets next.",
    "details": []
  },
  "message-29": {
    "title": "Run the last package tests",
    "summary": "The packaged targets pass their checks. The final job tests the app and its process ownership.",
    "details": []
  },
  "message-30": {
    "title": "Wait for GitHub to finish",
    "summary": "The tests and packaging pass. GitHub is saving the final cache.",
    "details": []
  },
  "message-31": {
    "title": "Push and publish",
    "summary": "The required checks pass. The agent pushes directly to main and publishes the site.",
    "details": []
  },
  "message-32": {
    "title": "Check the live site",
    "summary": "The published walkthrough, guides, search, controls, and mobile layouts pass their browser checks.",
    "details": []
  },
  "message-33": {
    "title": "Hand over the finished work",
    "summary": "Codex provides the live site, commit, downloadable game, and verification results.",
    "details": []
  }
});

export function explanationFor(id) {
  if (!Object.hasOwn(explanations, id)) throw new Error("Missing recording explanation: " + id);
  const explanation = explanations[id];
  return explanation;
}
const escape = text => String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const inline = text => escape(text).replace(/`([^`]+)`/g, "<code>$1</code>");
export function explanationHTML(explanation) {
  return '<div class="action-explanation"><p>' + inline(explanation.summary) + '</p>' +
    (explanation.details.length ? '<dl>' + explanation.details.map(([label, text]) =>
      '<div><dt>' + escape(label) + '</dt><dd>' + inline(text) + '</dd></div>'
    ).join("") + '</dl>' : "") + '</div>';
}
