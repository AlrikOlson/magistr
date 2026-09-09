# Shared permissions

`arch-05-shared-permissions` moves the permission decision before presentation.
Every received ACP permission request is evaluated by the same project policy.
Headless, plain, cockpit, next/loop and pool paths select context explicitly;
the presence of a screen supplies no additional authority.

## Owners and precedence

The domain `PermissionPolicyService` combines the normalized request, local
policy context, remembered charter and governance. Its resolution carries
Allow, Deny or Ask, risk, scope, reason and a deterministic policy version.
The CLI `SessionPermissionPolicy` supplies request classification, project path
resolution, configuration, charter persistence and audit effects.
`CockpitPolicy` remains a compatibility name for that implementation.

The decision order is:

1. A mutation of a protected target is denied, including edit, delete and move.
2. The selected profile, explicit auto-allow rules, configured gate commands and
   workflow allowances determine the local result.
3. Remembered choices and written constraints apply to that local result.
   A refusal still applies when a workflow shortcut would otherwise allow.
   Remembered permission for one path does not authorize additional targets.
4. Governance tightens the result. A protected or governed Deny cannot be
   changed into an Ask or an Allow by remembered permission or compatibility.

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
grant permission. Late arrivals remain cancelled until a fresh cockpit prompt.
AgentSession cancellation ends its usable stream; follow-ups require a new
session, matching that backend's existing terminal cancellation behavior.

Sessions without a permission UI refuse Ask by default. This includes plain
terminal sessions whose text input is for conversation, not a permission picker.
An explicitly opted-in compatibility mode permits automatic approval of Ask:

```toml
# <project>/.claude/permissions.toml
noninteractive = "legacy-automatic-approval"
```

Omitting the field, or setting `noninteractive = "refuse"`, selects the default.
The compatibility option does not change the core decision. Deny remains Deny,
and an absent compatible response option is Cancelled. Automatic responses select
only the matching once option; standing Always
choices require an explicit user answer. A missing once option is Cancelled,
even if the provider offers a standing choice. The historical first-option
fallback is retired rather than generalized.

This is an intentional migration from unconditional headless approval. Existing
unattended users can select the named compatibility mode; new sessions default
to policy refusal for unresolved Ask. Selecting a permissive profile is a
separate policy choice and remains subject to protected targets and governance.

## Evidence and limits

The policy companion suites cover profile, workflow, protection, charter and
governance precedence, exact scope and policy-version behavior. AgentSession
tests cover noninteractive handling, explicit compatibility, receiver ownership
and cancellation. The cockpit integration suite exercises actual ACP transport
and terminal response handling, including remembered workflow refusal and a
protected delete. Session tests cover stale and invalid answers and cancellation
while awaiting a decision. These use controlled providers; they do not claim
live-provider or native desktop verification.

The protocol allows automatic decisions according to user settings and requires
the cancelled outcome for a cancelled prompt turn.
See [ACP permission requests](https://agentclientprotocol.com/protocol/v1/tool-calls#requesting-permission).

The guarantee applies to permission requests delivered to Magistr. Provider
preapprovals, provider-owned tool execution and ACP filesystem/terminal effects
are separate enforcement boundaries. Governed host capabilities are the scope of
`arch-06-effect-capabilities`; shared provider/session supervision follows later.
This implementation does not provide OS sandboxing. Existing lexical-first path
resolution has limits around symlink ancestors and nonexistent targets; actual
effect authorization must bind the filesystem target in the following chunk.
Configuration is loaded when constructing the policy, with existing load-error
fallbacks retained; this is not a live governance-reload mechanism. Audit and
charter persistence retain their existing best-effort behavior; an in-memory
decision does not prove that durable evidence was written.

Run the required formatter and every gate in `.magistr/project.json`, as well as
focused permission/session tests. No golden regeneration or live-provider opt-in
is required by this change.
