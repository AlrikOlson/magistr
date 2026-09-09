# Desktop interface

`magistr-gui2` renders the shared cockpit model in a native window. It uses
winit for input, wgpu for GPU rendering, and a CPU renderer for screenshot tests.
It is independent of the terminal renderer.

## Build and run

Desktop builds require access to the `qs-framework` dependency pinned in
`crates/magistr-gui2/Cargo.toml`. The CLI on `main` has no such requirement.

```sh
cargo run -p magistr-gui2
cargo run -p magistr-gui2 -- 05
cargo run -p magistr-gui2 -- sink
```

The standalone binary opens interactive fixtures: the default busy view,
reference view `05`, or the component catalogue (`sink`). Live CLI sessions are
not connected to this binary.

## Rendering and input

The cockpit owns state transitions and emits a layout tree. The desktop crate
maps that tree to native elements, draws it with shared theme values, and sends
input back as cockpit messages. Side effects use the cockpit's command ports.
Motion follows model ticks and respects reduced motion.

Fonts, reference HTML, and image fixtures live under `crates/magistr-gui2`.
The font notices describe their origins and licenses.

## Tests

```sh
cargo nextest run -p magistr-gui2
./scripts/screenshot-gate.sh
```

Tests cover input routing, accessibility data, layout, and CPU-rendered images.
Review intended visual changes before regenerating goldens with
`MAGISTR_GOLDEN_UPDATE=1`.

`./scripts/mock-parity-gate.sh` compares native fixtures with the reference
HTML captures. It is an optional development check; some views are unfinished.
Capture scripts under `scripts/` require Node 22+ and Chrome. Design token
changes can be generated with `python3 scripts/almagest-tokens.py`.
