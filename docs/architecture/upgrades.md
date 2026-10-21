# Project upgrades

The owner requested automatic upgrades on 2026-10-20, superseding the previous
blanket prohibition on migrations. Each component retains one current runtime
path and converts recognized older inputs at its admission boundary.

## Configuration

`ProjectSettingsUpgrade` normalizes known settings before typed parsing.
Read-only callers see the intended configuration without filesystem effects.
`ProjectUpgrade` plans narrow changes to project settings and saved MCP launch
arguments. Init, repair, agent launch and hosted workspace startup publish those
changes through `AdminService`; the service saves content-addressed original
bytes before replacement. Custom fields and unrelated registrations survive.

`ProjectIdentityUpgrade` preserves an older declared identity only when the
current configuration does not declare a different identity. A separate current
namespace containing work is an explicit conflict. The workflow relocation
request is written before the identity changes; the administrative writer also
checks that dependency before publishing the identity.

## Workflow admission

After taking the current project lease, `ProjectAuthority` completes a requested
workflow relocation. It takes older project and document locks, selects only that
project's workflow records, converts recognized older envelopes through each
family's persistence owner, validates the resulting current documents, checks
every destination, and publishes complete files atomically. Existing different bytes
are a conflict. A completion receipt lists copied paths and hashes; an
interrupted batch accepts only identical prior copies. Subsequent admissions
never overwrite newer work with the retained originals. Read views report a
pending relocation instead of presenting an empty project.

Version-1 saved work receives the same stable run and task identities the older
writer assigned, preserving receipt references. Project-wide work and receipts
move into the primary working copy's default execution. Version-1 reasoning
histories consolidate into one project document, retaining session names, step
numbers and existing identities. Missing step identities are derived deterministically;
conflicting identities or numbers are refused. Roadmap and signal owners fill
omitted item origins from the validated enclosing project, preserving all authored
fields and rejecting explicit foreign origins. Current readers remain strict.
Conversion is deterministic, so an interrupted copy can resume without minting
new identities or replacing newer work. The older files remain byte-for-byte intact.

The older JSON fixtures under each workflow family's `fixtures` directory
use the envelope and record shapes captured from an existing installation on
2026-10-21, with synthetic content replacing private project information.
Admission and CLI repair tests exercise those fixtures, interruption, repeat
admission, invalid input and live document owners.

The default older user-data location is recognized. Explicit custom storage,
unsupported historical schemas and conflicting identities are not guessed.
Other projects, global credentials and unrelated application state are outside
this project-scoped conversion.

## Workspace admission

`WorkspaceService` acquires the current workspace lock and then the older
workspace lock before opening the journal or changing workspace settings.
Recognized native locks use OS ownership, never a guessed stale PID.
Unknown lock protocols and live owners stop admission.

The journal owner reads the older database through a SQLite read transaction,
including committed WAL rows. SQLite read transactions retain a consistent
snapshot; copying the main database file alone would omit uncheckpointed
commits. See [SQLite isolation](https://sqlite.org/isolation.html).

One transaction in the current database imports operation rows, ordered file
records, compressed blobs, undo links and native write outcomes, and records
completion. Current operation IDs remain stable. Imported IDs are offset when
necessary and all internal references are translated. Schema 4 separates imported
historical positions from numeric operation IDs, so recent-history pagination
and default undo selection keep current work ahead of imported history. Raw
append-order queries retain numeric sequence semantics.

After commit, admission retires the older lock on its existing inode. If the
process stops between the transaction and retirement, or during the marker write,
the committed import permits finishing retirement without duplicating rows.
Original journal files remain recovery evidence; keys and derived/process state
are not copied. Settings move separately through `WorkspaceConfigUpgrade`,
which preserves comments and archives the original before removing the retired
configuration path.

## Verification

Companion tests cover committed WAL data, coexisting histories, stable references,
undo eligibility, pending intents, pagination, transaction rollback, a torn
retirement marker, live owners, symbolic links, retained originals, custom
settings, read-only previews and repeated admission. The journal schema fixture
was captured from an existing version-3 store; its test records are synthetic.
CLI subprocess tests exercise saved launch commands and explicit repair.
