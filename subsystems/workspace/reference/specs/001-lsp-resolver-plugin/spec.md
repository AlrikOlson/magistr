# Feature Specification: LSP Resolver Plugin on the Public SDK

**Feature Branch**: `001-lsp-resolver-plugin`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "Rebuild the LSP resolver as a plugin on the public SDK, lazily booted and circuit-breakered. The resolver is built entirely on public SDK interfaces, with no private hooks. If the resolver never becomes healthy, every tool still works at tree-sitter fidelity. References carry confidence provenance so the envelope never overclaims." (chunk:m5-lsp-resolver-plugin; SDD §8.3 Resolvers, §1.1 no-LSP-requirement refusal, §7.3.1 Tier-2)

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly
> one roadmap chunk. This spec covers `chunk:m5-lsp-resolver-plugin` only. The other
> M5 surfaces (lang pack, runner adapter, ranking shadow) are separate future features.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Precise references through a plugin that could have been written by a third party (Priority: P1)

An agent asks for references to a symbol. When the resolver plugin is healthy, the answer
is upgraded to Tier-2 precision (real language-server resolution) instead of Tier-1
symbolic matching. The plugin achieves this using only interfaces a third party could
use: the public resolver surface, the declared subprocess capability, and nothing else.
The core team feels every rough edge of the SDK before any third party does.

**Why this priority**: This is the forcing function for SDK quality and the reason the
chunk exists. Without it, the "public SDK" label is a claim, not a fact.

**Independent Test**: Install the resolver plugin in a workspace with a detectable
language server; ask for references; verify the answer is resolver-upgraded. Audit the
plugin's imports: every interface it consumes is declared in the public WIT vocabulary.

**Acceptance Scenarios**:

1. **Given** a workspace whose project has a detectable language server and a healthy
   resolver plugin, **When** the agent requests references for a symbol, **Then** the
   result reflects language-server resolution and is labeled as such.
2. **Given** the resolver plugin's manifest and imports, **When** they are audited
   against the public WIT surface, **Then** no import, hook, or host call exists that a
   third-party plugin could not also declare.
3. **Given** the plugin conformance kit (TCK), **When** it runs over the resolver plugin
   unmodified, **Then** every check passes.

---

### User Story 2 - Nothing ever waits on the resolver, and nothing dies with it (Priority: P2)

The resolver boots lazily after Tier-1 indexing completes and is circuit-breakered. A
user whose language server is missing, broken, or slow gets exactly the product they had
before this feature existed: every tool answers at tree-sitter fidelity, immediately.

**Why this priority**: The SDD's standing refusal — "No LSP requirement" — is what makes
the upgrade safe to ship. Violating it turns an optional upgrade into a new outage class.

**Independent Test**: Run the full tool surface with the resolver absent, then with it
crashing on every request, then with it timing out. All three runs answer every request
successfully at tree-sitter fidelity.

**Acceptance Scenarios**:

1. **Given** a workspace where no language server is detectable, **When** any tool is
   used, **Then** every answer arrives at tree-sitter fidelity with no added latency and
   no error surfaced to the agent.
2. **Given** a resolver plugin that crashes or exceeds its timeout, **When** the failure
   occurs, **Then** the plugin is marked unhealthy, the in-flight request is answered at
   tree-sitter fidelity, and subsequent requests do not wait on the unhealthy plugin.
3. **Given** an unhealthy resolver, **When** the system retries it, **Then** retries are
   bounded and lazy — recovery attempts never block or delay an agent-facing answer.
4. **Given** Tier-1 indexing in progress, **When** the resolver would boot, **Then**
   boot waits for Tier-1 completion and no query is delayed by resolver startup.

---

### User Story 3 - The envelope never overclaims (Priority: P3)

Every reference answer carries confidence provenance naming how it was resolved. An
agent reading the envelope can tell a language-server-verified reference from a symbolic
tree-sitter match, and no degraded answer is ever presented as an upgraded one.

**Why this priority**: Honest labeling is what makes graceful degradation usable —
an agent that cannot tell Tier-1 from Tier-2 answers must assume the worst everywhere.

**Independent Test**: Request references with the resolver healthy and again with it
unhealthy; assert the provenance labels differ accordingly and that no result produced
symbolically carries a resolver-verified label.

**Acceptance Scenarios**:

1. **Given** a healthy resolver, **When** references are returned, **Then** each edge
   carries provenance identifying resolver-verified precision.
2. **Given** an unhealthy or absent resolver, **When** references are returned, **Then**
   provenance identifies symbolic (tree-sitter) resolution — never a resolver claim.
3. **Given** a mixed answer (some edges upgraded, some not), **When** it is enveloped,
   **Then** provenance is per-edge, not per-response.

---

### Edge Cases

- Language server detected but fails to start (bad install, wrong version): plugin
  becomes unhealthy without consuming its crash budget on a hopeless target; tools
  unaffected.
- Language server starts, then dies mid-request: in-flight request falls back to
  symbolic resolution; circuit opens.
- A plugin other than the blessed resolver declares the subprocess capability: denied
  before instantiation — the capability is reserved to blessed plugins.
- Resolver returns malformed or out-of-workspace locations: answers are validated and
  jailed like any other path data; malformed data marks the plugin unhealthy, never
  corrupts an envelope.
- Workspace with multiple detectable language servers: detection picks per-language;
  a failure in one language's server must not open the circuit for another's.
- Resolver healthy but slower than its per-request budget: the symbolic answer ships
  and the envelope says so; slowness is a health signal, not a wait.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The resolver MUST run as a plugin consuming only public SDK interfaces
  (the public resolver WIT surface and declared capabilities); it MUST NOT use any
  private hook, side channel, or host interface unavailable to third parties.
- **FR-002**: The resolver plugin MUST pass the plugin conformance kit (TCK) unmodified.
- **FR-003**: The resolver MUST boot lazily, only after Tier-1 indexing completes, and
  booting MUST never delay any agent-facing answer.
- **FR-004**: The resolver MUST speak to project-detected language servers over stdio
  via a declared subprocess capability, and that capability MUST be grantable only to
  blessed plugins — default-deny for everything else, refused before instantiation.
- **FR-005**: The resolver MUST be circuit-breakered: any crash, timeout, or malformed
  answer marks it unhealthy; while unhealthy, every tool answers at tree-sitter
  fidelity; recovery attempts are bounded and never block a request.
- **FR-006**: Reference results MUST carry per-edge confidence provenance
  distinguishing resolver-verified from symbolic resolution, and a symbolically
  resolved edge MUST never carry a resolver-verified label.
- **FR-007**: Every resolver failure surfaced to the protocol MUST be a typed,
  taxonomy-coded condition (never message-sniffed), and resolver unavailability MUST
  present as degraded fidelity, not as an error envelope, for tools that can still
  answer.
- **FR-008**: Fidelity upgrades to the resolution contract (e.g. position-based
  resolution) MUST be additive for existing consumers of symbolic resolution — no
  removal, rename, or signature change within a major version.
- **FR-009**: Resolver-provided locations MUST pass the same workspace-jail validation
  as any other path data before entering an envelope.

### Key Entities

- **Resolver plugin**: the first first-party consumer of the public SDK; bundled, but
  installed and sandboxed exactly as a third-party plugin would be.
- **Language server**: an external per-language process, project-detected, spoken to
  over stdio; may be absent, broken, or slow at any time.
- **Health state / circuit breaker**: per-plugin (and per-language-server) record of
  healthy/unhealthy with bounded, lazy recovery; the switch between Tier-2 and Tier-1
  answers.
- **Confidence provenance**: per-edge label on reference results naming the resolution
  method, carried in the envelope.
- **Subprocess capability**: the one grant reserved to blessed plugins; named in the
  manifest, granted default-deny before instantiation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the resolver absent, crashing, or timing out, 100% of tool requests
  in the release suite still answer successfully at tree-sitter fidelity (kill-test
  measured — the pre-feature product is fully preserved).
- **SC-002**: The TCK passes over the resolver plugin unmodified, as part of the
  release-blocking suite.
- **SC-003**: Zero envelopes in the release suite label a symbolically resolved edge as
  resolver-verified (provenance honesty, pinned by test).
- **SC-004**: Resolver boot adds zero measurable latency to any agent-facing answer
  (lazy boot measured against the pre-feature baseline).
- **SC-005**: An import audit of the resolver plugin finds zero interfaces outside the
  public WIT vocabulary.
- **SC-006**: References latency stays inside the existing release gate (p95 under
  50ms at 1M LOC) with the resolver healthy.

## Assumptions

- Constitution rule 8 scopes this feature to `chunk:m5-lsp-resolver-plugin` alone; the
  other three M5 surfaces are separate future features with their own specs.
- The public WIT surfaces, the capability-manifest grant model (default-deny before
  instantiation), and the self-hosted TCK all exist and are green as of the M4 exit
  gate; this feature consumes them rather than building them.
- "Blessed" status for the bundled resolver is a build-time fact of the first-party
  distribution; the signed-registry admission path for third parties is a later
  milestone (M6) and out of scope here.
- Language-server detection heuristics cover the mainstream per-language servers;
  breadth of detection is tunable later and not an acceptance criterion here.
- The existing symbolic (tree-sitter) resolution path remains the permanent floor and
  is not modified by this feature beyond carrying provenance labels.
