# Host effect capabilities

`arch-06-effect-capabilities` puts ACP filesystem and terminal requests behind
the `magistr-application::effects` contract. `EffectPort` names the supported
operations; `EffectAuthorizer` supplies the session's permission decision path.
The application crate remains `no_std` with `alloc`, independent of ACP, Tokio,
the filesystem, process APIs and presentation.

## Ownership and wiring

The transport translates ACP requests and responses. It has no implicit local
filesystem or process authority: a standalone transport advertises no host
effects unless its builder supplies a capability adapter. The CLI's shared
`connect_transport` path opts into `.local_effects()`, which installs
`LocalEffectPort`. A host can instead supply its own `EffectPort`.

The local adapter owns filesystem reads, process creation and live terminal
resources. It never writes a source file itself: the host composes it with a
`SourceRoute` (`AcpTransport::local_effects(route)`, threaded from
`ResolvedSessionSpec::with_source_route`) — the admitted workspace's
`SourceWriter`, under which authorized writes go through the
[workspace pipeline](#routing-writes-through-the-workspace-pipeline), or a
`SourceRefusal`, under which the write capability is withheld and every write
is refused. It reports its read, write and terminal support through
`EffectCapabilities`; initialization advertises those values, so the write
capability is advertised only when a writer is admitted. Unsupported calls
are refused even if an agent sends them despite the advertised capability.
File targets and terminal working directories must resolve inside the configured
workspace. Absolute paths outside it and symlinks escaping it are denied before
policy can grant access. Command executables may live outside the workspace;
their resolved paths, working directories and complete inputs are part of the
authorization request. The requested command and quoted arguments retain existing
command-policy matching, while a separate resolved executable is what the adapter
launches. Working directory and executable paths are also governance targets.
The protocol's [filesystem](https://agentclientprotocol.com/protocol/v1/file-system)
and [terminal](https://agentclientprotocol.com/protocol/v1/terminals) methods make
capability negotiation explicit; Magistr additionally authorizes each admitted
read, write and process creation locally.

Terminals belong to their creating session. Output, wait, kill and release
check that ownership rather than asking for an unrelated second creation grant.
Cleanup remains possible after turn cancellation. Closing the connection releases
its adapter resources. The cockpit consumes existing terminal events; terminal
protocol types do not enter the application or shared model boundary.

## Admission

For a read, write or terminal creation the local path is:

1. Validate the live session and requested capability.
2. Resolve the filesystem target or process working directory, preserving actual
   symlink traversal. A missing write target is resolved through its existing
   ancestors before any directories or files are created. Resolve the executable
   using the request's working directory and effective `PATH`. Windows accepts
   the omitted `.exe` suffix; a program outside that search path needs an explicit
   path rather than an implicit platform-specific search location.
3. Send a canonical `Read`, `Write` or `Bash` request through the existing permission
   queue. The request describes the resolved target and complete operation input.
4. Apply `SessionPermissionPolicy` with the session's profile, protected paths,
   charter, governance and explicit workflow context. Interactive Ask uses the
   existing permission UI; noninteractive Ask uses the documented
   [refusal or automatic response setting](permissions.md#interactive-and-noninteractive-ask).
5. Recheck cancellation and target resolution after authorization, then perform
   the admitted operation. A changed target is refused; approval does not follow
   a symlink that was retargeted while the decision was pending.

Policy Deny cannot become an override prompt. Cancellation, a missing permission
consumer, a dropped answer, timeout or an invalid answer cannot authorize an
effect. Permission waits and terminal waits run outside the connection's dispatch
loop, so updates and cancellation continue to be processed.

`EffectError` distinguishes denied, unavailable, cancelled, invalid-request and
execution failures. The ACP bridge converts them to protocol errors instead of
fabricating successful empty responses. An error after admission does not imply
rollback; cancellation prevents pending admission and does not undo a completed
file write or program side effect.

## Routing writes through the workspace pipeline

`arch-14f2` routes an authorized `fs/write_text_file` through the admitted
workspace when the host is its owner (`magistr::application::HostedWorkspace`,
composed by `magistr run` and the cockpit loops). The order is unchanged:
resolution, the session's permission decision, the post-decision recheck — and
only then the route. A denied or cancelled write reaches neither the pipeline
nor the file. Through the route the write is the workspace's `write` verb:
anchored on the current bytes, parse-verified, atomic, journaled as an intent,
formatting off so the bytes that land are the bytes the agent wrote, and never
confirm-gated because the permission decision is the confirmation. The port's
`fs_write` event carries `route: "workspace"`, the journal intent and the
`MutationReceipt`'s revision. The pipeline's refusals — a protected path, a
syntax error into a valid source, a stale anchor — are typed effect errors in
the pipeline's own words; nothing lands.

Since `audit-260912-04` there is no direct route. A host that did not admit the
workspace — another owner holds it, its marker needs migration, its
configuration is refused, its durable data is from a newer binary — composes
the port with a `SourceRefusal` instead of a writer (`SourceRoute::Refused`;
`magistr::application::WorkspaceAdmission` is the one place every host gets
one). The port then advertises `fs.writeTextFile: false`, so a cooperating
agent must not call `fs/write_text_file` and runs its own workspace tools
instead; a write it sends anyway is answered before resolution and
authorization with the refusal (`unavailable` for a missing owner, `denied`
for a session without authority), the `fs_write` event says
`route: "refused"` with the refusal's kind, and nothing is written, journaled
or receipted. The judge's sessions and any session resolved without a route
carry `NoAuthority`. The agent's own runtime writes, like every external
write (an editor, a provider CLI, git), are outside this cooperative
contract: not routed, not journaled, observed by `SourceSnapshot` validation
as changes. `crates/magistr-cli/tests/source_pipeline_routing.rs` proves the
routed and the refused paths against real owners and the real port;
`tests/effect_capabilities.rs` proves both through the real transport and
session supervisor; the `native-write-path` gate's rule 12 forbids any direct
file write in `magistr-acp` production sources.

## Limits

This is a host application authorization boundary, not an OS sandbox. The local
adapter uses path resolution and a check after the permission wait; it does not
hold directory handles and atomically constrain every filesystem lookup. Another
process can race the final check and I/O. Hard links, executable replacement and
effects performed inside an authorized program are not confined by this layer.
Policy configuration and audit/charter persistence retain the limitations in the
[permission guide](permissions.md#evidence-and-limits).

The port uses provider session identities for live resource correlation. It does
not introduce durable effect deduplication, native project-owner admission or a
shared provider supervisor. A custom port is trusted host code and must uphold
the authorization contract; a trait implementation is not a security proof.
Provider-owned tool execution outside ACP host callbacks is not mediated here.

## Verification

Local adapter tests exercise the application request and error types against
real files and processes. Real in-process ACP transport tests inspect advertised capabilities,
allowed effects, refusals and continued event processing. CLI tests exercise
shared policy against real filesystem targets, including symlinked parents and
missing write targets, and preserve remembered command refusals across explicit
permission requests and direct effects. Temporary fixtures and subprocess markers establish
whether an effect happened; a policy-only assertion does not prove enforcement.

Formatting and every standard and required custom gate in `.magistr/project.json`
remain required. Existing architecture, purity, contrast, icon, screenshot and
latency gates retain their scope; no visual golden updates are part of this change.
