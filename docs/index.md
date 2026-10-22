# Documentation index

## Product manual

The [standalone manual](https://magistr.dev/docs) is the source for current user
instructions. Each page can be entered directly.

| Need | Guide |
| --- | --- |
| Install and connect | [Installation](https://magistr.dev/docs/installation), [agents](https://magistr.dev/docs/agents) |
| Understand and start work | [Concepts](https://magistr.dev/docs/concepts), [first task](https://magistr.dev/docs/workflow) |
| Plan and deliver | [Roadmap](https://magistr.dev/docs/roadmap), [reasoning](https://magistr.dev/docs/reasoning), [chunks](https://magistr.dev/docs/chunks), [verification](https://magistr.dev/docs/verification) |
| Continue and improve | [Sessions](https://magistr.dev/docs/sessions), [feedback](https://magistr.dev/docs/feedback), [desktop](https://magistr.dev/docs/desktop) |
| Look up settings or behavior | [Commands](https://magistr.dev/docs/commands), [skills](https://magistr.dev/docs/skills), [configuration](https://magistr.dev/docs/configuration), [permissions](https://magistr.dev/docs/permissions) |
| Maintain an installation | [Storage](https://magistr.dev/docs/storage), [troubleshooting](https://magistr.dev/docs/troubleshooting), [upgrades](https://magistr.dev/docs/upgrades), [uninstall](https://magistr.dev/docs/uninstall) |

Repository user guides are entry points to that manual, not a second copy of it.

## Current contributor guidance

These files require repository access. They describe changing Magistr itself.

- [Contributing](../CONTRIBUTING.md): scope, checks and review.
- [Development](development.md): toolchain, focused verification, desktop,
  website, CI and delivery.
- [Architecture overview](architecture/overview.md): component boundaries and
  links to the current technical contracts.
- [Website](../apps/website/README.md): content ownership, routes, search,
  browser checks and evidence.

The native roadmap and work plans are stored in Magistr. Keep
`roadmap_exports: false`; do not generate `ROADMAP.md` or `ROADMAP.json`.

## Technical reference

- [Codex integration](reference/codex.md): launcher and project skill discovery.
- [Authentication](reference/authentication.md): team-identity protocol and limits.
- [Tool recovery](reference/tool-recovery.md): error and recovery contracts.
- [Upgrade contracts](reference/upgrades.md): conversion ownership and recovery.
- [Interface module index](reference/cockpit-modules.md): source module inventory.
- [Claude adapter verification](reference/claude-mod-verification.md): integration checks.

## Release maintenance

- [Release inventory](release-inventory.md): contents and qualification checklist.
- [CLI releasing](releasing-cli.md): packaging and explicit publication procedure.
- [Release security](release-security.md): security checks and receipts.

These procedures describe release preparation. They do not establish that
Magistr has been publicly released or that registry packages are available.

## Historical evidence

Dated records retain the source, environment and limits of the work they report.
They are evidence, not current setup instructions.

- [September benchmark pilot](benchmark-pilot-september-2026.md).
- [Public-release audit](public-release-audit.md).
- [Desktop foundation decision](architecture/desktop-foundation.md).
- [Desktop layout evidence](evidence/desktop-layout/README.md).
- [Native desktop evidence](evidence/desktop-current/README.md).
- [Dated interface iterations](https://github.com/AlrikOlson/magistr-evidence):
  the `magistr-evidence` repository, mounted as the submodule
  `docs/evidence/desktop-iterations` (`git submodule update --init`).
- [Recorded example sources](../examples/bevy-tic-tac-toe/README.md): the Bevy
  game behind the website walkthrough and the
  [support inbox](../examples/support-inbox/README.md) behind the recorded session.

Retain original records and earlier screenshots when new evidence is added.
