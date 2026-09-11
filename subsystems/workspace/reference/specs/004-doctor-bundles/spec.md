# Feature Specification: Workbench Doctor + Diagnostic Bundles

**Feature Branch**: `004-doctor-bundles`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "workbench doctor + diagnostic bundles (chunk:m6-doctor, SDD §11.2/§11.3): extend the existing workbench-mcp CLI verb family with `doctor [workspace]` (a read-only, no-lock probe report: environment — filesystem type, native-vs-WASM engine selection, watcher health; index health from .workbench/index.db; five-layer config resolution with per-key provenance reusing the existing doctor config renderer; plugin inventory with trust tiers and unhealthy/breakered plugins; quarantined files from .workbench/quarantine.jsonl; recent engine crashes) and `doctor bundle [workspace]` (emit a redacted diagnostic tarball carrying structure and symptoms, never repo file contents, that a support engineer can use to resolve a seeded corrupt-index scenario without repo access — the M6 release-gate scenario)."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A corrupt index is resolved from the bundle alone (Priority: P1)

A user's workspace misbehaves (searches degrade, the index never warms). They run `doctor bundle`, inspect the produced archive, and attach it to a support ticket. A support engineer — with **no access to the repository** — reads the bundle, identifies the corrupt index store, and sends back remediation steps that fix the workspace.

**Why this priority**: SDD §11.3 makes exactly this scenario the M6 release gate: "an ops plane that hasn't been exercised is documentation, not architecture." It is also the chunk's first acceptance criterion.

**Independent Test**: Seed a workspace, corrupt `.workbench/index.db` with garbage bytes, produce a bundle, and — using only the bundle's content (no repo access, no live workspace access) — derive the remediation, apply it, and verify the workspace recovers to a healthy index.

**Acceptance Scenarios**:

1. **Given** a workspace whose index store is unreadable (seeded corruption), **When** the user produces a diagnostic bundle, **Then** the bundle names the corrupt store, its data class (rebuildable cache vs durable user data), and the remediation for that class — without requiring any other input.
2. **Given** only the bundle content, **When** a support engineer follows the remediation it names, **Then** the workspace returns to a healthy state (the index rebuilds; no user data is lost).
3. **Given** a healthy workspace, **When** a bundle is produced, **Then** it reports every probed subsystem as healthy so the absence of findings is a claim, not silence.

---

### User Story 2 - Local health report on demand (Priority: P2)

A user (or an agent driving the CLI on their behalf) runs `doctor` against a workspace and gets a single readable report: environment facts, index health, effective configuration with per-key provenance, plugin inventory with trust tiers, unhealthy (breakered) plugins, quarantined files, and recent engine crashes — each section stating where the fact came from.

**Why this priority**: The report is the substrate the bundle is built from; it also satisfies the chunk's other two acceptance criteria (per-key layer attribution, quarantined files and unhealthy plugins visible).

**Independent Test**: Run `doctor` on a workspace seeded with a quarantined file, a breakered plugin, and a session config override; assert all three appear in the report with their provenance.

**Acceptance Scenarios**:

1. **Given** a workspace with config decided across multiple layers, **When** the user runs `doctor`, **Then** every effective key is shown with the layer that decided it (the existing `doctor config` surface, embedded in the full report).
2. **Given** a workspace where an input file was quarantined and a plugin was breakered, **When** the user runs `doctor`, **Then** both appear: the quarantined path with its strike history, and the plugin with the reason and the recovery rule (changed component clears the breaker).
3. **Given** a workspace with no index yet, **When** the user runs `doctor`, **Then** the index section reports "no index" as a normal finding with the expected consequence (Tier-0 answers only) rather than an error.

---

### User Story 3 - The bundle is safe to share (Priority: P2)

Before attaching the bundle to a ticket, the user can open it and see everything a recipient would see. Nothing in it reveals repository file contents; the worst it can leak is structure and symptoms.

**Why this priority**: The constitution's supply-chain section is explicit — "diagnostic bundles carry structure and symptoms, never file contents." A bundle that cannot be trusted will not be shared, and then the ops plane fails at its purpose.

**Independent Test**: Seed workspace files with a sentinel secret string, produce a bundle, and assert the sentinel appears nowhere in any bundle member; assert every member is human-inspectable text.

**Acceptance Scenarios**:

1. **Given** a workspace whose files contain a known sentinel string, **When** a bundle is produced, **Then** no bundle member contains the sentinel (no repo file bytes are ever copied in).
2. **Given** a produced bundle, **When** the user unpacks it with standard tooling, **Then** every member is readable text (report sections in machine-readable form plus a manifest), so informed consent to share is possible.
3. **Given** state records that reference absolute paths outside the workspace root, **When** the bundle is produced, **Then** those paths are reduced to placeholders while workspace-relative paths are preserved.

---

### User Story 4 - Diagnosis never makes things worse (Priority: P3)

`doctor` and `doctor bundle` are read-only: they take no lock, mutate nothing under the workspace or its state directory, and run safely beside a live server.

**Why this priority**: A diagnostic that can contend with or corrupt the thing it diagnoses is a new outage class. The existing `doctor config` verb already established the no-lock precedent.

**Independent Test**: Run `doctor` and `doctor bundle` while a server holds the workspace lock; both succeed, the server is undisturbed, and a before/after snapshot of the state directory is byte-identical (except the bundle output file itself, written outside the state directory by default).

**Acceptance Scenarios**:

1. **Given** a live server holding the workspace lock, **When** `doctor` runs, **Then** it completes without waiting on or breaking the lock.
2. **Given** any doctor invocation, **When** it finishes, **Then** the workspace and its state directory are unmodified.

---

### Edge Cases

- Index store is unreadable/garbage: reported as a finding with data class and remediation — never an abort.
- Index store is missing entirely: "no index yet" is a normal state (fresh workspace), distinct from corruption.
- Quarantine log contains malformed lines: skipped with a per-line note; valid lines still reported (append-only logs can tear).
- Plugin inventory references a component file that no longer exists on disk: reported as a finding.
- State directory absent entirely (never-opened workspace): doctor reports that and still emits environment + config sections.
- A state file is newer than the binary understands (version from the future): fail-closed reporting — the store is named as "newer than this binary" rather than misread (mirrors the data-lifecycle rules).
- Workspace root does not exist: usage error with the existing exit code convention.
- Bundle output path already exists: refused rather than overwritten (a diagnostic must not destroy a prior diagnostic).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CLI MUST provide a `doctor` verb over a workspace that emits a single health report covering, at minimum: environment (filesystem type and network-fs detection outcome, engine selection native-vs-fallback, watcher status), index health (presence, version, integrity, tier/generation, last build error if recorded), effective configuration with per-key deciding layer, plugin inventory with trust tiers, unhealthy/breakered plugins with reasons, quarantined files with strike history, and recent engine crash evidence.
- **FR-002**: Every report section MUST name the on-disk source it was read from, so a reader can audit any claim.
- **FR-003**: The CLI MUST provide a `doctor bundle` form that writes a single archive file whose members carry the report's sections in machine-readable form plus a manifest (bundle format version, tool version, creation time, member list). The archive MUST be openable with standard tooling.
- **FR-004**: The bundle MUST NOT contain repository file contents. Workspace-relative paths are permitted; absolute paths outside the workspace root MUST be reduced to placeholders. Raw query text, secrets, and environment variable values MUST NOT appear.
- **FR-005**: Both forms MUST be read-only: no workspace lock taken, no file under the workspace or its state directory created, modified, or deleted. The bundle file itself is written to an explicit or default output path outside the state directory.
- **FR-006**: A missing, unreadable, or from-the-future state store MUST be reported as a finding (store name, data class, symptom, remediation) and MUST NOT abort the report or the bundle.
- **FR-007**: Every unhealthy finding MUST carry a remediation statement specific to its data class: rebuildable cache stores name the discard-and-rebuild path; durable stores name the quarantine-intact recovery; version-from-the-future names the upgrade path.
- **FR-008**: The process exit code MUST distinguish "report produced, all healthy" from "report produced, findings present" and from usage/environment errors, so scripts can gate on doctor.
- **FR-009**: The `doctor config` verb MUST keep working unchanged; the full report embeds the same per-key provenance rendering rather than forking it.
- **FR-010**: The seeded corrupt-index scenario MUST exist as an automated test that produces a bundle from a corrupted workspace and derives + applies the remediation using bundle content alone.

### Key Entities

- **Probe**: one read-only inspection of an on-disk artifact or environment fact; yields a section of the report. Probes are independent — one probe's failure is another probe's finding.
- **Finding**: an unhealthy condition: store/subsystem name, data class (cache vs durable), symptom, remediation text.
- **Report**: the ordered composition of all probe sections; human-readable on stdout.
- **Bundle**: an archive of the report's sections as machine-readable members plus a **Manifest** (bundle format version, tool version, timestamp, member list). The bundle is itself a versioned contract.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A support engineer (simulated by the automated gate test) resolves the seeded corrupt-index scenario using only bundle content: the remediation derived from the bundle restores a healthy workspace with zero user-data loss.
- **SC-002**: Zero bytes of repository file content appear in any bundle: a seeded sentinel string planted in workspace files is absent from every bundle member in the privacy test.
- **SC-003**: `doctor` on a cold workspace (no server running, no index) completes in under 5 seconds and reports every probed subsystem.
- **SC-004**: `doctor` and `doctor bundle` run to completion beside a live server holding the workspace lock, and the state directory is byte-identical before and after.
- **SC-005**: All three chunk acceptance criteria are demonstrated by tests: corrupt-index-from-bundle-alone, per-key layer attribution in the report, quarantined files + unhealthy plugins in the bundle.

## Assumptions

- No watcher subsystem exists yet; the environment section reports watcher status honestly as "no watcher subsystem in this build" rather than fabricating a probe. When a watcher ships, the section gains a real probe.
- Crash-report upload, consent, and anything network-facing is out of scope (that is chunk m6-crash-reports; the server/CLI never phones home here). "Recent engine crashes" is read from local state only.
- Doctor is diagnosis-only: no repair verb in this feature. Remediation is text a human (or agent) follows; a mutating repair flow would need the confirm-gate machinery and is a separate decision.
- The existing CLI exit-code convention (64 usage, 66 missing root) is kept; a new code distinguishes "findings present" from "healthy".
- The bundle's archive format is a widely-unpackable tarball (gzip-compressed); introducing no new runtime dependency is a constraint inherited from the stack commitments.
- The quarantine record home (`.workbench/quarantine.jsonl`), plugin health (`.workbench/plugin-health.json`), plugin inventory (`.workbench/plugins.json`), and index store (`.workbench/index.db`) are the shipped artifacts this feature reads; it introduces no new stores of its own.
