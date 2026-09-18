# Shared permissions

`arch-05-shared-permissions` moves the permission decision before presentation.
Every received ACP permission request is evaluated by the same project policy.
Headless, plain, cockpit, next/loop and pool paths select context explicitly;
the presence of a screen supplies no additional authority.

## Owners and precedence

The domain `PermissionPolicyService` combines the normalized request, local
policy context, remembered charter and governance. Its resolution carries
Allow, Deny or Ask, risk, scope, reason and a deterministic policy version.
The service, its context, mode, outcome, source and resolution are one
[cohesive module](domain.md#cohesive-modules) behind the domain's `policy`
facade.
The CLI `SessionPermissionPolicy` supplies request classification, project path
resolution, configuration, charter persistence and audit effects.
`CockpitPolicy` currently aliases that implementation. The alias may be removed
with its callers under the [pre-release policy](../../AGENTS.md#before-public-release).

The application owns `sessions::SessionPolicy`, its request and decision types,
and the meaning of offered permission choices. The CLI converts ACP requests to
that contract, retaining tool identity, title, kind and the full encoded input.
Its policy adapter decodes and normalizes the input; malformed data is refused
and cannot create a remembered rule. ACP reply channels and pending offered IDs
remain with the host supervisor.

The decision order is:

1. A mutation of a protected target is denied, including edit, delete and move.
2. The selected profile, explicit auto-allow rules, configured gate commands and
   workflow allowances determine the local result.
3. Remembered choices and written constraints apply to that local result.
   A refusal still applies when a workflow shortcut would otherwise allow.
   Remembered permission for one path does not authorize additional targets.
4. Governance tightens the result. A protected or governed Deny cannot be
   changed into an Ask or an Allow by remembered permission or automatic response settings.

Workflow is explicit context. The existing development-tool and project-MCP
allowances remain available in workflow mode and are capped by protected paths,
remembered constraints and governance. They are not inferred from headless mode.
Project rules, gates, protected paths, charter and governance resolve from the
selected project directory.

Scope and policy version identify what was evaluated. They are diagnostic
correlation values, not signed capabilities, native authority revisions, durable
approval receipts or an OS security boundary. Changing relevant policy or
remembered rules changes the policy version; choosing a different presentation
surface does not.

## Interactive and noninteractive Ask

Cockpit presents Ask and forwards an offered answer. Remembered answers belong
to the shared policy; the session no longer maintains an independent allow set
keyed only by tool title. Invalid option IDs are refused. Cancelling a turn clears
its pending requests and responds with Cancelled, so a stale card cannot later
grant permission. Late arrivals remain cancelled until a fresh turn is admitted.
The shared supervisor cancels pending and late permissions for both surfaces
when cancellation is requested. `cancel_turn` retains the session but admits no
new turn until the provider's terminal response; permission handling resumes only for an admitted
fresh turn. Legacy `AgentSession::cancel` ends the usable session. See the
[separate lifecycle operations](sessions.md#shared-session-supervision).

Sessions without a permission UI refuse Ask by default. This includes plain
terminal sessions whose text input is for conversation, not a permission picker.
The current implementation has an explicit automatic response setting for Ask:

```toml
# <project>/.claude/permissions.toml
noninteractive = "legacy-automatic-approval"
```

Omitting the field, or setting `noninteractive = "refuse"`, selects the default.
The automatic response option does not change the core decision. Deny remains Deny,
and an absent compatible response option is Cancelled. Automatic responses select
only the matching once option; standing Always
choices require an explicit user answer. A missing once option is Cancelled,
even if the provider offers a standing choice. The historical first-option
fallback is retired rather than generalized.

The earlier requirement to preserve unconditional headless approval is withdrawn.
This setting describes existing behavior and may be removed or redesigned with
its callers and tests; older behavior supplies no reason to retain it. Selecting
a permissive profile is a separate current policy choice and remains subject to
protected targets and governance.

## Evidence and limits

The policy companion suites cover profile, workflow, protection, charter and
governance precedence, exact scope and policy-version behavior. AgentSession
tests cover noninteractive handling, automatic response settings, receiver ownership
and cancellation. The cockpit integration suite exercises actual ACP transport
and terminal response handling, including remembered workflow refusal and a
protected delete. Session tests cover stale and invalid answers and cancellation
while awaiting a decision. These use controlled providers; they do not claim
live-provider or native desktop verification.

The protocol allows automatic decisions according to user settings and requires
the cancelled outcome for a cancelled prompt turn.
See [ACP permission requests](https://agentclientprotocol.com/protocol/v1/tool-calls#requesting-permission).

The guarantee applies to permission requests delivered to Magistr and, through
`arch-06-effect-capabilities`, to [ACP filesystem and terminal effects](effects.md).
The configured local adapter resolves the actual target before constructing a
canonical `Read`, `Write` or `Bash` request for the same policy. It rechecks the
target after authorization before admitting the operation. Path resolution follows
existing symlink ancestors before handling `..`, including writes to missing files.

Provider preapprovals and provider-owned tool execution remain separate enforcement
boundaries. The same supervisor enforces session lifecycle across cockpit and
headless paths. This implementation does not provide OS sandboxing or atomic
directory-handle confinement. An authorized
program can perform its own effects, and other filesystem writers can race path
checks; the [effect guide](effects.md#limits) states the resulting boundary.
Configuration is loaded when constructing the policy, with existing load-error
fallbacks retained; this is not a live governance-reload mechanism. Audit and
charter persistence retain their existing best-effort behavior; an in-memory
decision does not prove that durable evidence was written.

Run the required formatter and every gate in `.magistr/project.json`, as well as
focused permission/session tests. No golden regeneration or live-provider opt-in
is required by this change.
