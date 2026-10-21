# Contributing to Magistr

Magistr is a source preview and has not been publicly released. Contributions
require repository access. For using Magistr on another project, read the
[product manual](https://magistr.dev/docs).

## Start here

1. Read [project instructions](AGENTS.md), the [development guide](docs/development.md),
   and the [architecture overview](docs/architecture/overview.md).
2. Inspect existing code and native roadmap records before proposing work.
3. Identify which component owns the behavior and fix the problem there.
4. Keep the change coherent, preserve unrelated work, and describe the observable
   result and acceptance criteria.

The repository keeps roadmaps and work plans in Magistr. Do not generate
`ROADMAP.md` or `ROADMAP.json`. Planning and reviews do not authorize
implementation of unrelated roadmap items.

## Verification

Choose focused verification for bounded changes, including code. Run formatting
and checks for the affected behavior, dependencies and acceptance criteria.
Record actual results through named Magistr gates or server-executed
`check_work`. Use the full configured suite only for an explicit request or
a concrete cross-cutting risk; state the reason first.

For website changes, run `npm test` in `apps/website` and the affected browser
scenarios. `npm run test:docs` isolates documentation; `npm run test:ui`
includes documentation and the landing page. See the
[website guide](apps/website/README.md).

Every interface iteration needs reviewed screenshots saved directly under a new
dated folder in `docs/evidence/desktop-iterations/`. Record viewport, appearance,
renderer, source hashes, time, observations and verdicts. Passing tests do not
replace visual review. Native OS-input journeys require explicit authorization
for that run; routine layout checks run in the background.

Read the relevant desktop or terminal contract before changing an interface.
Preserve contrast, reduced motion, icon, screenshot and architecture-index checks.
Run release packaging checks when packaging is affected, not for every change.

## Documentation

Public user instructions belong in `apps/website/src/content/docs/`.
Repository entry points link to that manual. Technical contracts, development
commands and dated evidence stay in their contributor sections. The
[documentation index](docs/index.md) identifies those audiences.

## Delivery

Explain the problem, resulting behavior, checks, and material limitations.
Include reviewed evidence for visual changes. Use one coherent conventional
commit and stage only the intended files.

For authorized direct delivery to protected `main`, follow the
[verification-branch process](docs/development.md#ci-and-direct-pushes).
A request to edit files does not independently authorize publication.

By participating, you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
For security issues, use [private reporting](SECURITY.md).
