//! Shared permission adapter for interactive and unattended CLI sessions.
//! Normalization and persistence live here; policy precedence lives in the domain.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1::ToolKind;
use magistr_acp::AcpPermissionRequest;
use magistr_cockpit::agent::{PermissionKind, PolicyDecision, SessionPolicy};
use magistr_engine::policy::{
    Charter, PermissionContext, PermissionMode, PermissionOutcome, PermissionPolicyService,
    PermissionResolution, PermissionSource, PolicyEffect, PolicyEngine, PolicyRequest, PolicyRule,
};

use super::append_entry::append_entry;
use super::apply_retention::apply_retention;
use super::audit_entry::AuditEntry;
use super::audit_log_path::audit_log_path;
use super::audit_retention::AuditRetention;
use super::auto_allow_rule::AutoAllowRule;
use super::charter_store::CharterStore;
use super::decision_reason::DecisionReason;
use super::delegation_scope::DelegationScope;
use super::evaluate::evaluate;
use super::high_risk_bash_patterns::HIGH_RISK_BASH_PATTERNS;
use super::permission_decision::PermissionDecision;
use super::permission_profile::PermissionProfile;
use super::policy_request_for::policy_request_for;
use super::profile_action::ProfileAction;
use super::resolve_against::{checked_resolve_against, resolve_against};

/// Shared host adapter for the domain's deterministic permission service.
/// Every surface supplies ACP requests to this owner of normalization, risk
/// classification, local configuration, the charter and audit persistence.
/// Protected targets and governance are immutable floors; standing refusals
/// and written conditions apply to workflow and safe-operation allowances too.
/// This is application authorization, not an operating-system sandbox.
pub struct SessionPermissionPolicy {
    profile: PermissionProfile,
    auto_rules: Vec<AutoAllowRule>,
    gate_commands: Vec<String>,
    protected_paths: Vec<PathBuf>,
    /// The session's working directory — what a relative path the agent
    /// names is relative to. `None` compares relative targets by their
    /// trailing components instead.
    root: Option<PathBuf>,
    mode: PermissionMode,
    /// Where evaluate decisions are appended; `None` drops them.
    audit_path: Option<PathBuf>,
    /// Who is running the session — recorded on every audit entry.
    actor: Option<String>,
    /// Retention bounds applied to the log after each append.
    retention: AuditRetention,
    /// The org's compiled guardrails; empty in solo mode.
    governance: PolicyEngine,
    /// The model this session runs on, for model-governance rules.
    model: Option<String>,
    /// The reader's standing orders for this project, and their track
    /// record; empty until one is given.
    charter: Charter,
    /// Where the charter is written back after every change; `None`
    /// keeps it in memory for the session.
    charter_store: Option<CharterStore>,
}

impl SessionPermissionPolicy {
    /// Construct a host policy from already resolved project configuration.
    /// The session resolver passes the same governance snapshot that selected
    /// its model; later file changes cannot replace either decision.
    ///
    /// # Errors
    ///
    /// A configured charter cannot be read or decoded.
    pub fn for_project_snapshot(
        project_dir: &Path,
        config: super::PermissionConfig,
        governance: magistr_engine::policy::GovernancePolicy,
        profile: PermissionProfile,
        workflow: bool,
        model: Option<&str>,
    ) -> crate::error::Result<Self> {
        let identity = crate::team::Identity::resolve(project_dir);
        let store = CharterStore::at(project_dir);
        let charter = store.load()?;
        Ok(Self::new(
            profile,
            config.rules,
            crate::orchestration::commands::run::collect_gate_commands(project_dir),
            crate::orchestration::commands::run::protected_paths_for_project(project_dir),
            workflow,
        )
        .rooted_at(Some(project_dir.to_path_buf()))
        .governed_by(governance.compile())
        .on_model(model.map(str::to_owned))
        .acting_as(Some(identity.id().to_string()))
        .retain(config.retention)
        .chartered(charter, Some(store)))
    }

    /// A policy over the given profile, rules and protected paths;
    /// `workflow` turns on the tool-kind auto-approval `magistr next`
    /// runs under. Audit entries go to the default audit log.
    #[must_use]
    pub fn new(
        profile: PermissionProfile,
        auto_rules: Vec<AutoAllowRule>,
        gate_commands: Vec<String>,
        protected_paths: Vec<PathBuf>,
        workflow: bool,
    ) -> Self {
        Self {
            profile,
            auto_rules,
            gate_commands,
            protected_paths,
            root: None,
            mode: if workflow {
                PermissionMode::Workflow
            } else {
                PermissionMode::Standard
            },
            audit_path: audit_log_path(),
            actor: None,
            retention: AuditRetention::default(),
            governance: PolicyEngine::default(),
            model: None,
            charter: Charter::default(),
            charter_store: None,
        }
    }

    /// Resolve the agent's relative paths against `root` (the session's
    /// working directory) before matching them to the protected entries —
    /// `ROADMAP.md` is `/project/ROADMAP.md` once the policy knows where
    /// the session lives.
    #[must_use]
    pub fn rooted_at(mut self, root: Option<PathBuf>) -> Self {
        self.root = root;
        self
    }

    /// Enforce the org's governance policy on top of the local decision.
    #[must_use]
    pub fn governed_by(mut self, governance: PolicyEngine) -> Self {
        self.governance = governance;
        self
    }

    /// Tell model-governance rules which model this session runs on.
    #[must_use]
    pub fn on_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    /// Run under the reader's `charter`, written back to `store` after
    /// every answer, rule and accepted offer (`None` keeps it for the
    /// session only).
    #[must_use]
    pub fn chartered(mut self, charter: Charter, store: Option<CharterStore>) -> Self {
        self.charter = charter;
        self.charter_store = store;
        self
    }

    /// The charter as it stands — what the rail reads its rows and
    /// offers from.
    #[must_use]
    pub fn charter(&self) -> &Charter {
        &self.charter
    }

    /// Send audit entries to `path` instead of the default log — the
    /// tests' hook; `None` drops them.
    #[must_use]
    pub fn audit_to(mut self, path: Option<PathBuf>) -> Self {
        self.audit_path = path;
        self
    }

    /// Record `actor` (the resolved team identity id) on every audit
    /// entry this policy writes.
    #[must_use]
    pub fn acting_as(mut self, actor: Option<String>) -> Self {
        self.actor = actor;
        self
    }

    /// Prune the audit log to `retention` after each append.
    #[must_use]
    pub fn retain(mut self, retention: AuditRetention) -> Self {
        self.retention = retention;
        self
    }

    /// Log an evaluate decision to the audit log (best-effort — a
    /// failed write never blocks the agent).
    fn log(&self, decision: &PermissionDecision) {
        let Some(path) = &self.audit_path else {
            return;
        };
        let entry = AuditEntry::from_decision(
            &decision.tool_name,
            decision.risk,
            decision.action,
            &decision.reason,
            None,
            None,
            self.actor.as_deref(),
        );
        let _ = append_entry(path, &entry);
        let _ = apply_retention(path, &self.retention);
    }
}

impl SessionPermissionPolicy {
    /// Evaluate a request once through the common policy contract. Every
    /// outcome carries its exact request scope and policy configuration version.
    #[must_use]
    pub fn evaluate_request(&mut self, request: &AcpPermissionRequest) -> PermissionResolution {
        let (act, resolution_error) = self.normalized_request(request);
        let evaluated = evaluate(
            &request.tool_title,
            &request.tool_input,
            self.profile,
            &self.auto_rules,
            &self.gate_commands,
        );
        let context = PermissionContext {
            mode: self.mode,
            profile_effect: match (resolution_error.as_ref(), evaluated.action) {
                (Some(_), _) => PolicyEffect::Deny,
                (None, ProfileAction::Allow) => PolicyEffect::Allow,
                (None, ProfileAction::Ask) => PolicyEffect::RequireApproval,
            },
            profile_reason: resolution_error.unwrap_or_else(|| evaluated.reason.to_string()),
            local_allowance: should_auto_approve(request, false),
            workflow_allowance: should_auto_approve(request, true),
            protected: is_write_to_protected_file(
                &request.tool_title,
                &act.paths,
                request.tool_kind.as_ref(),
                &self.protected_paths,
                self.root.as_deref(),
            ),
            scope: self.request_scope(request, &act),
            policy_version: self.policy_version(),
        };
        let resolution =
            PermissionPolicyService::evaluate(&act, &context, &self.charter, &self.governance);
        // Preserve the legacy log's omission of ungoverned local shortcuts
        // and protected-target refusals. A standing or governance rule that
        // changes a shortcut is audited, including its real rule identity.
        if matches!(
            resolution.source,
            PermissionSource::Profile
                | PermissionSource::Charter(_)
                | PermissionSource::Governance(_)
        ) {
            self.log(&PermissionDecision {
                tool_name: evaluated.tool_name,
                risk: evaluated.risk,
                action: if resolution.outcome == PermissionOutcome::Allow {
                    ProfileAction::Allow
                } else {
                    ProfileAction::Ask
                },
                reason: DecisionReason::PolicyResolution(resolution.reason.clone()),
            });
        }
        resolution
    }

    fn normalized_request(
        &self,
        request: &AcpPermissionRequest,
    ) -> (PolicyRequest, Option<String>) {
        let mut act = policy_request_for(
            &request.tool_title,
            &request.tool_input,
            self.model.as_deref(),
        );
        let root = match self
            .root
            .as_deref()
            .map(|root| checked_resolve_against(None, root))
            .transpose()
        {
            Ok(root) => root,
            Err(error) => return (act, Some(error)),
        };
        act.path_aliases.clear();
        for path in &mut act.paths {
            let resolved = match checked_resolve_against(root.as_deref(), Path::new(path)) {
                Ok(resolved) => resolved,
                Err(error) => return (act, Some(error)),
            };
            let aliases = root
                .as_deref()
                .and_then(|root| resolved.strip_prefix(root).ok())
                .map(|relative| vec![relative.display().to_string()])
                .unwrap_or_default();
            *path = resolved.display().to_string();
            act.path_aliases.push(aliases);
        }
        (act, None)
    }

    /// Stable within equal configurations and changed whenever a policy input
    /// changes. FNV-1a is an identity checksum, not a security authenticator.
    #[must_use]
    pub fn policy_version(&self) -> u64 {
        let root = self.root.as_deref().map(|root| resolve_against(None, root));
        let protected: Vec<_> = self
            .protected_paths
            .iter()
            .map(|path| resolve_against(root.as_deref(), path))
            .collect();
        let bytes = serde_json::to_vec(&(
            "shared-permissions-v1",
            self.profile,
            &self.auto_rules,
            &self.gate_commands,
            protected,
            root,
            self.mode,
            &self.model,
            &self.governance,
            &self.charter.delegations,
        ))
        .expect("policy inputs contain only JSON-serializable values");
        bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
    }

    fn request_scope(&self, request: &AcpPermissionRequest, act: &PolicyRequest) -> String {
        let root = self.root.as_deref().map(|root| resolve_against(None, root));
        // Retain the complete transport input too: URLs and other tool-specific
        // arguments must never share approval just because both have no paths.
        // Equivalent path spellings may have distinct scopes; that is conservative.
        serde_json::to_string(&(
            root,
            &act.tool,
            &act.command,
            &act.paths,
            &act.model,
            &act.risk,
            &act.gate,
            &request.tool_kind,
            &request.tool_input,
        ))
        .expect("request fields contain only JSON-serializable values")
    }

    /// The charter's scope for `request`, cut against the session's
    /// root.
    fn scope_of(&self, request: &AcpPermissionRequest) -> Option<DelegationScope> {
        if request.tool_title.contains(['*', '?']) {
            return None;
        }
        let (act, resolution_error) = self.normalized_request(request);
        if resolution_error.is_some() {
            return None;
        }
        let root = self.root.as_deref().map(|root| resolve_against(None, root));
        // A terminal's cwd is a policy target, but remembered process choices
        // keep the same command scope as an explicit execution permission ask.
        if request.tool_kind == Some(ToolKind::Execute) && act.command.is_some() {
            return DelegationScope::of(&request.tool_title, &request.tool_input, root.as_deref());
        }
        if !act.paths.is_empty() {
            let mut scopes = act.paths.iter().map(|path| {
                DelegationScope::of(
                    &request.tool_title,
                    &serde_json::json!({"file_path":path}),
                    root.as_deref(),
                )
            });
            let scope = scopes.next()??;
            if scope.key.starts_with("tool:") {
                // An unrepresentable path (outside the root, or containing
                // glob syntax) must not turn into a tool-wide delegation.
                return None;
            }
            // Never author a standing rule over only the first of several scopes.
            return scopes
                .all(|candidate| candidate.is_some_and(|candidate| candidate.key == scope.key))
                .then_some(scope);
        }
        DelegationScope::of(&request.tool_title, &request.tool_input, root.as_deref())
    }

    /// Write `rule` into the charter and the charter to its store —
    /// best-effort, as the audit log is: a failed write never blocks
    /// the session, and the rule holds in memory for the rest of it.
    fn write_rule(&mut self, rule: PolicyRule) {
        self.charter.delegate(rule);
        self.save_charter();
    }

    fn save_charter(&self) {
        if let Some(store) = &self.charter_store {
            let _ = store.save(&self.charter);
        }
    }
}

impl SessionPolicy for SessionPermissionPolicy {
    fn decide(&mut self, request: &AcpPermissionRequest) -> PolicyDecision {
        let resolution = self.evaluate_request(request);
        match resolution.outcome {
            PermissionOutcome::Allow => PolicyDecision::Allow,
            PermissionOutcome::Deny => PolicyDecision::Deny,
            PermissionOutcome::Ask => PolicyDecision::Ask(resolution.risk),
        }
    }

    fn answered(&mut self, request: &AcpPermissionRequest, kind: PermissionKind) {
        let Some(scope) = self.scope_of(request) else {
            return;
        };
        let allowed = matches!(
            kind,
            PermissionKind::AllowOnce | PermissionKind::AllowAlways
        );
        self.charter
            .record(&scope.key, &scope.subject, &scope.condition, allowed);
        // A standing answer is a rule: `⇧A` always allows under the
        // scope, a never refuses it. A once-only answer leaves only its
        // mark on the tally — the whole point of once is that it is
        // not a rule.
        let rule = match kind {
            PermissionKind::AllowAlways => Some(PolicyRule::new(
                Charter::delegation_id(&scope.key),
                scope.condition,
                PolicyEffect::Allow,
            )),
            PermissionKind::RejectAlways => Some(PolicyRule::new(
                format!("refuse:{}", scope.key),
                scope.condition,
                PolicyEffect::Deny,
            )),
            PermissionKind::AllowOnce | PermissionKind::RejectOnce => None,
        };
        match rule {
            Some(rule) => self.write_rule(rule.describe(scope.subject)),
            None => self.save_charter(),
        }
    }

    fn constrain(&mut self, request: &AcpPermissionRequest, condition: &str) {
        let condition = condition.trim();
        if condition.is_empty() {
            return;
        }
        let Some(scope) = self.scope_of(request) else {
            return;
        };
        // A written condition is a reason to be asked again here. The
        // engine cannot evaluate the reader's prose, so the rule it
        // becomes is the one thing it can honour: ask under this scope,
        // in the reader's own words, even where the profile would not.
        self.write_rule(
            PolicyRule::new(
                format!("ask:{}", scope.key),
                scope.condition,
                PolicyEffect::RequireApproval,
            )
            .describe(condition),
        );
    }

    fn delegate(&mut self, key: &str) -> bool {
        let accepted = self.charter.accept(key).is_some();
        if accepted {
            self.save_charter();
        }
        accepted
    }
}

/// Check if a Write/Edit tool call targets a protected file.
///
/// Uses `tool_kind` from ACP when available for backend-agnostic
/// detection, falling back to tool name matching for Claude-style names.
///
/// Both sides are put through [`resolve_against`] before they are
/// compared, so the spelling the agent chose — relative, `./`-prefixed,
/// climbing through `..`, or via a symlinked root — cannot slip past an
/// absolute protected entry. Without a root a relative target is matched
/// by its trailing path *components* (`Path::ends_with`), never by a
/// string suffix, so `OLDROADMAP.md` is not `ROADMAP.md`.
fn is_write_to_protected_file(
    tool_name: &str,
    paths: &[String],
    tool_kind: Option<&ToolKind>,
    protected_paths: &[PathBuf],
    root: Option<&Path>,
) -> bool {
    if protected_paths.is_empty() {
        return false;
    }

    let is_write_tool = matches!(
        tool_kind,
        Some(ToolKind::Edit | ToolKind::Delete | ToolKind::Move)
    ) || tool_name.eq_ignore_ascii_case("write")
        || tool_name.eq_ignore_ascii_case("edit")
        || tool_name.eq_ignore_ascii_case("delete")
        || tool_name.eq_ignore_ascii_case("move");

    if !is_write_tool {
        return false;
    }

    // Use the same complete path set as governance. A multi-file patch may
    // touch a protected target after an ordinary file or through a rename.
    paths.iter().any(|file_path| {
        let Ok(target) = checked_resolve_against(root, Path::new(file_path)) else {
            return true;
        };
        protected_paths.iter().any(|p| {
            let Ok(protected) = checked_resolve_against(root, p) else {
                return true;
            };
            protected == target || target.is_relative() && protected.ends_with(&target)
        })
    })
}

/// Determine whether a permission request should be auto-approved.
///
/// Uses `tool_kind` from ACP (backend-agnostic) as the primary signal,
/// with `tool_title` and command content analysis as fallbacks. In
/// workflow mode, all standard development operations are auto-approved —
/// only truly unknown tools go on to the profile.
fn should_auto_approve(req: &AcpPermissionRequest, is_workflow: bool) -> bool {
    if is_workflow && let Some(ref kind) = req.tool_kind {
        match kind {
            ToolKind::Read
            | ToolKind::Search
            | ToolKind::Fetch
            | ToolKind::Think
            | ToolKind::Edit
            | ToolKind::Execute
            | ToolKind::Delete
            | ToolKind::Move
            | ToolKind::SwitchMode => return true,
            // Unknown/Other — fall through to name-based checks.
            _ => {}
        }
    }

    // Name-based fallback: catch Write/Edit that leaked past
    // `--allow-tool` (e.g. a backend sending human-readable titles).
    let title_lower = req.tool_title.to_ascii_lowercase();
    if matches!(
        title_lower.as_str(),
        "write" | "edit" | "read" | "glob" | "grep"
    ) {
        return is_workflow;
    }

    // Shell command analysis — safe commands pass even outside workflow.
    if is_shell_command(req) {
        if is_workflow {
            return true;
        }
        return is_safe_shell_command(&req.tool_input);
    }

    // MCP tools. Magistr's own are always approved. In workflow mode every
    // MCP tool is: the session's servers are exactly the project's own
    // `.mcp.json` (the agent runs the CLI under --strict-mcp-config), the
    // protected-path deny has already run, and an unanswered ask parks an
    // unattended loop until the transport's auto-approve timeout.
    if title_lower.starts_with("magistr_") || title_lower.starts_with("magistr-") {
        return true;
    }
    is_workflow && title_lower.starts_with("mcp__")
}

/// Check if a permission request is for a shell/bash command.
///
/// Different backends use different names: "Bash" (Claude), "shell"
/// (Copilot), or human-readable titles like "Check recent commits".
fn is_shell_command(req: &AcpPermissionRequest) -> bool {
    if req.tool_kind.as_ref() == Some(&ToolKind::Execute) {
        return true;
    }
    let title = &req.tool_title;
    if title == "Bash" || title.starts_with("shell") {
        return true;
    }
    req.tool_input
        .get("command")
        .and_then(|v| v.as_str())
        .is_some()
}

/// Check if a shell command is safe enough to auto-approve outside
/// workflow mode.
fn is_safe_shell_command(tool_input: &serde_json::Value) -> bool {
    let command = tool_input
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if command.is_empty() {
        return true;
    }

    let effective_cmd = strip_cd_prefix(command);

    for pattern in HIGH_RISK_BASH_PATTERNS {
        if effective_cmd.contains(pattern) {
            return false;
        }
    }

    const SAFE_PREFIXES: &[&str] = &[
        "ls",
        "cat",
        "echo",
        "pwd",
        "which",
        "whoami",
        "date",
        "env",
        "printenv",
        "head",
        "tail",
        "wc",
        "sort",
        "uniq",
        "grep",
        "find",
        "tree",
        "file",
        "stat",
        "du",
        "df",
        "git status",
        "git log",
        "git diff",
        "git show",
        "git branch",
        "git --no-pager",
        "cargo check",
        "cargo test",
        "cargo clippy",
        "cargo build",
        "cargo fmt",
        "cargo insta",
        "npm test",
        "npm run",
        "npx ",
        "node ",
        "python -m pytest",
        "pytest",
        "ruff",
        "mypy",
        "go test",
        "go build",
        "go vet",
        "rustup",
        "rustc --version",
        "make",
        "cmake",
    ];

    SAFE_PREFIXES
        .iter()
        .any(|prefix| effective_cmd.starts_with(prefix))
}

/// Strip a leading `cd <path> &&` from a command string — agents
/// frequently prefix commands with it, and the classification wants the
/// actual command.
fn strip_cd_prefix(command: &str) -> &str {
    let trimmed = command.trim();
    if !trimmed.starts_with("cd ") {
        return trimmed;
    }
    if let Some(pos) = trimmed.find("&&") {
        trimmed[pos + 2..].trim_start()
    } else {
        // Just a bare `cd` — safe.
        trimmed
    }
}

#[cfg(test)]
#[path = "session_permission_policy.tests.rs"]
mod tests;
