# Automatic project upgrades — 2026-10-20

## Result

Implemented the owner's request for quiet, automatic upgrades of recognized
settings and durable state. Known conversions run at setup or owner admission,
retain recovery copies, and continue using the current format. Normal successful
startup does not prompt or print upgrade messages. Full scaffold repair remains
explicit.

The installed release binary successfully upgraded `vs-code-image-viewer`.
Repeated repair reported `Updated 0 files.`; no history was duplicated and
unrelated tracked content remained unchanged.

## Source and installation

- Branch: `feature/automatic-project-upgrades-20261020`.
- Base: `af8eaff0c4a5f25a7811cc5415d8bbd3e892fee7`.
- Release-build source digest: `16be0513f463d990a471dc83d5f3aef3197e0c3a74c7fd617d0e3d5b107167c9`
  (`magistr-source-snapshot/1`, unchanged during the check).
- Build: `cargo build --release -p magistr-cli --bin magistr`, macOS arm64.
- Installed `~/.cargo/bin/magistr` SHA-256:
  `b0f5b67072eda611492fddb7ef1443134f9a2f0330a98d399678bc68d2faaa91`.
- The installed file matches the tested release artifact. The previous binary
  was retained in the local `magistr-before-automatic-upgrades-l3wmnudm`
  temporary directory.

## Recorded verification

All commands were run through native `check_work` with focused verification.

| Check | Actual result | Receipt |
| --- | --- | --- |
| Configuration and scaffold regressions | 399 passed | `4f70ca9a-06c6-4791-8919-7afc09b2af55` |
| Journal, configuration, workspace ownership | 64 passed | `b390a593-501e-4d14-943b-12dfb6702469` |
| Configuration, workflow ownership, CLI subprocesses | 433 passed | `ab4eebd2-208d-44d8-a0ca-68adb94b106d` |
| Final changed-crate Clippy and upgrade regressions | No warnings; 13 passed | `bc486036-5611-40ea-9142-5bbce33a6f6c` |
| Formatting, diff, structure, native write paths, workflow/workspace architecture and release build | Passed, including 9 structure regression tests | `4da77319-5dad-4bdc-b6ef-8e163b7c92e2` |
| Installed repair preview on the affected project | Exit 0; all 64 observed settings/state files unchanged | `ac38a391-015d-4fcd-b0b9-a7bdba75e36c` |
| Installed repair on the affected project | Exit 0; project identity and workflow request published; stores upgraded | `bbb34551-975c-45ad-a838-cc35c044acb5` |
| Actual records, retained originals, repeated repair and binary identity | Passed | `d06f5fd3-c839-46b7-8faa-065088d136e5` |

The final focused tests include initial setup adopting an older declared identity
and refusing a live older owner before that owner saves its first document.
Other tests cover WAL data, coexisting histories, undo links, pending intents,
rollback, interrupted publication, torn lock retirement, conflicts, unknown
formats, symbolic links, custom settings and saved launch registrations.

An initial lint warning was corrected. The real-project audit initially omitted
the expected retirement of `workbench.toml` from its allowlist; the corrected
audit also verified the archived file's exact original hash before passing.
No failed result was represented as a pass.

## Actual project recovery

Before upgrade, the current workspace journal was empty. The older version-3
journal contained 94 operations. The current version-4 journal now contains:

- 94 operations, 94 file records and 94 recorded write outcomes.
- 108 compressed snapshots, byte-for-byte equal to the originals.
- One completed import marker and 94 historical ordering records.
- Passing SQLite integrity and foreign-key checks.

The older declared project identity was retained. All 22 selected workflow files
match the retained originals: one execution document containing one saved task,
and 21 check receipts. Other projects' records were outside the selected import.

`workbench.toml` was archived with its exact original hash under
`.magistr/upgrades/workspace-config/`. The original workspace journal and
workflow documents remain intact. The older workspace lock was retired on its
existing inode; no owner was terminated.

The only changed tracked content in the affected project was
`.magistr/project.json`, `.workbench/server.lock`, and the retired
`workbench.toml`. Existing staged work was preserved and not committed by this task.

## Scope and limits

See [upgrade behavior](../reference/upgrades.md) and
[ownership and recovery](../architecture/upgrades.md).
Unknown historical schemas, unrecognized custom store locations, conflicting
records and live owners stop with an actionable error while preserving data.
The implementation does not guess how to convert an unknown format.

No GUI changed; no foreground input, package publication or deployment was run.
The existing Magistr connection remained running. Only focused checks were
required; this receipt does not claim a full workspace test run or remote CI pass.
