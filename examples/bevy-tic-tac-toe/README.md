# Bevy tic-tac-toe

A complete small game built for the Magistr website walkthrough. It uses
**Bevy 0.20.0**, with the same Rust rules in the interface and the tests.

Play against the computer. X starts; your mark alternates after each completed
round. The score stays until you close the game. Number keys 1 through 9 select
cells left to right, top to bottom. Press R or click Play again after a result.

## Run

Use Rust 1.97.1 or newer:

```sh
cargo run --locked --features game --bin tic-tac-toe
```

The example has its own Cargo workspace and lockfile. It does not add Bevy to
Magistr's build.

## Check

```sh
cargo test --locked --lib -- --nocapture
cargo run --locked --example compare_opponents
cargo fmt --check
```

The opponent test tries every legal human reply to the deterministic minimax
policy. It records 82 computer wins and 10 draws as X, and 368 wins and 153
draws as O. There are no computer losses. These are games against this policy,
not all possible tic-tac-toe games.

The comparison program evaluates the earlier win/block/center/corner candidate.
That policy survives as X but loses as O. Its counterexample is retained so the
decision can be reproduced.

## Browser build

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
node build-web.mjs
python3 -m http.server 4322 --directory target/web
```

Open http://localhost:4322. Set WASM_BINDGEN to an executable path if needed.
The CLI version must match wasm-bindgen in Cargo.lock. The browser runs the
compiled Bevy game; there is no JavaScript implementation of the rules.

## Structure

- src/lib.rs owns legal moves, outcomes, minimax and match scoring.
- src/main.rs connects Bevy's buttons and keyboard input to that owner.
- web/index.html hosts the canvas and its read-only status description.
- examples/compare_opponents.rs preserves the tactical-policy experiment.

## Recorded build

The website publishes selected actual tool actions, recorded reasoning,
roadmap changes, source changes and check results. Its terminal is an interactive
re-enactment of those records, not a screen recording. No private session files
or verbatim user prompts are included.

The versioned [Bevy button example](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/ui/widgets/button.rs)
and [text example](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/2d/text2d.rs)
were used to verify the 0.20.0 APIs. The game uses one fixed paper appearance.
Browser checks do not establish native OS or assistive-technology support.
