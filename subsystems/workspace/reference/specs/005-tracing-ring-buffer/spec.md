# Feature Specification: Local Tracing Ring Buffer

**Feature Branch**: `005-tracing-ring-buffer`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "local tracing ring buffer (chunk:m6-tracing, SDD §11.2/§1.1/§14): always-on-by-default local structured tracing for the workbench server — OTel-shaped spans (tool request → engine IPC → envelope) written as JSON lines to a size-capped on-disk ring buffer under .workbench/ (the flight recorder), never leaving the machine; span attributes carry tool name, error code, recovery class, and duration so tool-misuse patterns (retry loops, error churn) are visible for skill iteration; tracing never fails or slows a request; spans never contain query text, file contents, or tool arguments; no upload path exists (telemetry upload by explicit CLI action is a later chunk); a tracing.enabled config key (default true) can turn it off."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The flight recorder is just there when something goes wrong (Priority: P1)

A user's session misbehaves — an agent hammered a tool, an engine call was slow, an edit was rejected for reasons nobody remembers. Without having configured anything in advance, the user (or a maintainer they ask) opens the local trace record and reconstructs what happened: which tools were called, in what order, how long each took, and which failed with what error and recovery class.

**Why this priority**: a flight recorder that must be switched on before the incident records nothing. "On by default locally, never leaves the machine" is the chunk's first acceptance criterion and the whole value of the feature.

**Independent Test**: run a server against a fresh workspace with zero configuration, perform a handful of tool calls (some failing), and reconstruct the call sequence — names, durations, error codes — from the on-disk record alone.

**Acceptance Scenarios**:

1. **Given** a fresh workspace and no configuration, **When** tool requests are served, **Then** each produces a trace record on local disk with its name, start time, duration, and outcome.
2. **Given** a served request that failed, **When** the trace is read, **Then** the record carries the machine-readable error code and its recovery class.
3. **Given** any amount of tracing activity, **When** the machine's network activity is examined, **Then** tracing has produced none: the record exists only under the workspace's state directory.

---

### User Story 2 - Tool-misuse patterns are visible for skill iteration (Priority: P2)

A skill author (or the team reviewing agent behavior, SDD §14) wants to know how agents actually use the tools: retry loops on the same failing call, error-code churn, tools called before their prerequisites are up. They read the trace and the patterns are extractable without guesswork.

**Why this priority**: the chunk's third acceptance criterion, and the named data source for the SDD's tool-misuse mitigation. Without attribute discipline the record is noise.

**Independent Test**: drive a deliberate misuse pattern (the same failing call repeated N times), then demonstrate the pattern — same tool, same error code, N occurrences in sequence — from the trace alone.

**Acceptance Scenarios**:

1. **Given** an agent that retries a failing call five times, **When** the trace is read, **Then** five consecutive records with the same tool name and error code are extractable.
2. **Given** a request that involved engine work, **When** the trace is read, **Then** the engine call appears as a child record tied to its parent request, with its own duration.

---

### User Story 3 - Tracing can never hurt the product (Priority: P2)

Tracing is invisible to the agent: it never fails a request, never blocks one, and its storage never grows without bound. When the disk record cannot be written, requests proceed exactly as before.

**Why this priority**: Constitution I — any cost the shell does not have is a reason to bypass the tool. An observability feature that adds a failure mode or unbounded disk growth is a regression, not a feature.

**Independent Test**: make the trace destination unwritable and serve requests (all succeed, no notes change); write enough spans to exceed the cap and verify the record stays bounded with the newest entries retained.

**Acceptance Scenarios**:

1. **Given** an unwritable trace destination, **When** requests are served, **Then** every request completes exactly as it would have, with no new error surface.
2. **Given** sustained tracing past the size cap, **When** the record is examined, **Then** its size stays at or under the cap and the most recent entries are present (oldest evicted first).
3. **Given** tracing enabled, **When** request latency is compared with tracing disabled, **Then** the difference is within measurement noise for the served path.

---

### User Story 4 - Privacy is structural (Priority: P2)

The trace records structure and symptoms, never content: no query text, no file contents, no tool arguments, no paths outside the workspace. A user can hand their trace to a maintainer (or a future doctor bundle can carry it) without reviewing it line by line for secrets.

**Why this priority**: the constitution's telemetry constraint ("metadata only") and the reason the record can ever be shared. One leaked query string poisons trust in the whole ops plane.

**Independent Test**: perform searches and edits whose arguments contain a sentinel secret; assert the sentinel appears nowhere in the trace record.

**Acceptance Scenarios**:

1. **Given** tool calls whose arguments contain a sentinel string, **When** the trace record is examined, **Then** the sentinel appears nowhere in it.
2. **Given** the feature as shipped, **When** its code is examined for outbound-network capability, **Then** none exists in the tracing path — upload is impossible by construction, not by configuration (explicit-CLI-action upload is a later chunk's scope).

---

### Edge Cases

- Trace destination directory does not exist yet (workspace never opened): tracing starts when the server creates its state dir; before that there is nothing to trace.
- Record file torn mid-line (crash during append): readers tolerate malformed lines (the quarantine-log precedent); the writer never attempts repair.
- Cap breach mid-burst: eviction must not lose the entries of the request currently being written beyond cap mechanics; the newest entries always survive.
- `tracing.enabled` set to false at any config layer: no records are written, nothing else changes; flipping back on requires no migration.
- Two concurrent requests: records interleave; parent/child linkage keeps each request's tree reconstructable.
- Engine child down: the failed engine call still gets a record (with the failure outcome) from the server side.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The server MUST record one trace record per served tool request — core and plugin alike — carrying: a request-scoped trace identity, name, start time, duration, outcome (ok or the taxonomy error code plus its recovery class), and bounded descriptive attributes.
- **FR-002**: Engine calls made while serving a request MUST be recorded as child records linked to their parent request record, each with its own duration and outcome.
- **FR-003**: Records MUST use a widely mappable span vocabulary (trace/span/parent identifiers, name, timestamps, status, attributes) so an exporter could translate them 1:1 later — without this feature depending on any external telemetry library.
- **FR-004**: Records are stored as an append-oriented file under the workspace state directory, size-capped with oldest-first eviction; the cap is fixed and documented, and the newest records always survive eviction.
- **FR-005**: Tracing MUST be on by default with zero configuration, controllable by a `tracing.enabled` configuration key (default true) resolvable at any layer; disabling stops writes and changes nothing else.
- **FR-006**: Tracing MUST never fail, block, or alter a request: all trace IO is swallowed; an unwritable destination leaves the served surface byte-identical.
- **FR-007**: Records MUST NOT contain: tool arguments, query text, file contents, environment variable values, or absolute paths outside the workspace. Attribute keys are an allowlist; nothing free-form from the request may enter.
- **FR-008**: Nothing in the feature may open an outbound network connection or provide an upload mechanism; the trace never leaves the machine on its own. Upload-by-explicit-CLI-action is out of scope (a later chunk) and MUST NOT be partially implemented here.
- **FR-009**: The record format itself MUST be versioned from its first release (a format version stamped per file or per line), with additive-only evolution within the major version.
- **FR-010**: A documented read path (test-level is sufficient for this feature) MUST demonstrate reconstructing a request tree and extracting a misuse pattern from the record alone.

### Key Entities

- **Span record**: one unit of recorded work — a served tool request (root) or an engine call (child). Identity (trace, span, optional parent), name, start, duration, status (ok / error code + recovery), allowlisted attributes.
- **Trace**: the set of span records sharing one request's trace identity; reconstructable into a tree.
- **Ring buffer file**: the size-capped on-disk record under the workspace state directory; cache-class data (discardable and rebuildable by time — losing it loses history, never correctness).
- **`tracing.enabled`**: the configuration key governing writes; default true; not a security-lattice key (the privacy boundary is what records may contain, which is not configurable).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a fresh, unconfigured workspace, 100% of served tool requests produce a local trace record, and a request tree with durations and outcomes is reconstructed from the record alone.
- **SC-002**: A deliberate 5x retry loop on a failing call is extractable from the record as exactly five same-tool, same-error records in sequence.
- **SC-003**: A sentinel string planted in tool arguments appears zero times in the trace record across a full suite of traced operations.
- **SC-004**: With the trace destination made unwritable, 100% of requests succeed unchanged; with sustained writes, the record never exceeds its documented cap.
- **SC-005**: The tracing feature introduces no measurable served-path regression: traced vs untraced request latency differs within measurement noise in the recorded comparison.

## Assumptions

- "Ring buffer" is realized as a size-capped append file with oldest-first eviction (the workspace already uses this shape for another bounded log); a fixed-slot binary ring is not required by any consumer and is out of scope.
- Spans inside the engine child process (per-query database timing) are out of scope for this feature: the server-side engine-call record bounds that time. Cross-process trace propagation is a recorded follow-up, not silent omission.
- The existing `telemetry.consent` key is untouched; it governs upload consent for a later chunk. Local tracing is not telemetry and is governed only by `tracing.enabled`.
- Trace records may later be carried by doctor bundles; the privacy requirements here (FR-007) are what make that safe, but bundle integration is not part of this feature.
- The trace covers the served tool surface and engine calls; internal background lanes (index build progress, plugin boots) may add spans later — additive under FR-009's versioning rule.
