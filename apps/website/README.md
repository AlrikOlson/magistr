# Magistr website

The Astro site at https://magistr.dev.

```sh
npm ci
npm run dev
npm test
npm run test:ui
```

The public guides are in src/content/docs. The landing page's only interactive
example is BuildWalkthrough.astro and build-walkthrough.ts. It replays the
allowlisted records in public/demo/tic-tac-toe/recording.json.

terminal-playback.ts owns the read-only xterm.js terminal. Seeking rebuilds its
scrollback up to the current action; queued writes coalesce to the latest seek.
The annotation and optional project details follow the same cursor. Opening a
record pauses playback and leaves the terminal in place.

build-explanations.mjs owns the website's explanations, keyed to all 65 recorded
actions. The sidebar, timeline labels and readable transcript share this copy.
Summaries explain the event; labeled details add the reason or next action when
needed. New recording actions must supply explanations: raw tool responses are
never an annotation fallback. The terminal keeps the original output, and an
explanation must not invent a cause or claim a later result has already happened.

scripts/highlight-recording.mjs tokenizes code at build time with Shiki and
derives unified diffs from successive recorded snapshots with jsdiff. The
original recording is unchanged. Shared code-theme.mjs supplies both themes to
the transcript, file readers, terminal ANSI output and Markdown guides. The
browser loads the generated, content-versioned playback.json; it does not load
Shiki or calculate diffs. Readers clone the escaped, pre-rendered transcript.
Source and command copy actions use original text.

The interaction follows the [xterm.js terminal API](https://xtermjs.org/docs/api/terminal/classes/terminal/)
and [asciinema's marker navigation](https://docs.asciinema.org/manual/player/markers/).
Chapter controls seek recorded actions; long idle gaps are shortened during
playback. No keystrokes or private user prompts are invented.

The roadmap and reasoning maps import the actual desktop renderer modules from
crates/magistr-desktop/ui. The website adapts their surrounding layout and uses
a readable list on compact screens. Selecting a record opens its requirements
or explanation. The cursor reconstructs only records available at that action.

The recording comes from implementing examples/bevy-tic-tac-toe with Bevy
0.20.0. It includes actual roadmap changes, reasoning branches, file writes,
task transitions and command results. Routine setup is omitted. The displayed
terminal is a re-enactment of tool records, not video of the Codex application.
No private session files or verbatim user prompts are published.

Run npm run publish:game after an intended example-source or recording update.
It packages only the source allowlist, copies the reviewed game captures and
writes their hash manifest. Ordinary website builds do not read user histories,
compile Bevy, download models or run an agent.

## Verification

npm test checks generated routes, links, preview availability, privacy,
record chronology and downloads, then runs the game rules tests without
compiling Bevy. test:ui runs the actual website in headless WebKit.

To retain a visual iteration:

```sh
npm run test:ui -- --evidence-dir docs/evidence/desktop-iterations/YYYY-MM-DD-name
```

Use a fresh folder, open the images and add REVIEW.md. The browser report keeps
viewports, appearance, timestamps, renderer and source hashes. A passing browser
run is not a visual review.

The optional game browser check needs the compiled Bevy game served at port 4322:

```sh
node tests/game-browser.mjs --evidence-dir docs/evidence/desktop-iterations/YYYY-MM-DD-game
```

See the game's README for building it. The website serves the source download
and reviewed captures; its landing page does not load the large game binary.