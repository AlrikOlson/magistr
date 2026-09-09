//! The cockpit session's permission policy — the decision the legacy
//! `spawn_acp_permission_handler` applied, behind the session's port.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1::ToolKind;
use magistr_acp::AcpPermissionRequest;
use magistr_cockpit::agent::{PermissionKind, PolicyDecision, SessionPolicy};
use magistr_engine::policy::{
    Charter, PolicyEffect, PolicyEngine, PolicyRequest, PolicyRule, RiskTier,
};

use super::append_entry::append_entry;
use super::apply_retention::apply_retention;
use super::audit_entry::AuditEntry;
use super::audit_log_path::audit_log_path;
use super::audit_retention::AuditRetention;
use super::auto_allow_rule::AutoAllowRule;
use super::charter_store::CharterStore;
use super::classify_risk::classify_risk;
use super::decision_reason::DecisionReason;
use super::delegation_scope::DelegationScope;
use super::evaluate::evaluate;
use super::high_risk_bash_patterns::HIGH_RISK_BASH_PATTERNS;
use super::permission_decision::PermissionDecision;
use super::permission_profile::PermissionProfile;
use super::policy_request_for::policy_request_for;
use super::profile_action::ProfileAction;
use super::resolve_against::resolve_against;
use super::risk_level::RiskLevel;

/// What the cockpit session asks before a permission request reaches
/// the strip: a write to a protected file is denied, workflow mode
/// auto-approves the standard development operations by ACP tool kind,
/// and the rest is the profile's call via [`evaluate`] — with that
/// decision appended to the audit log, as the legacy handler did.
///
/// On top of all of that sits the org's governance policy. Whatever
/// the local decision was, the answer is the *tighter* of it and the
/// policy's effect, so the roster can refuse a tool, a command or a path
/// that the profile — even `yolo` — and the auto-approve path would
/// otherwise have waved through. There is no branch to forget: the
/// tightening is a `max`, and every governed decision is audited by rule
/// id so the compliance report has something to count.
///
/// Between the profile and the roster sits the reader's own [`Charter`]:
/// the rules they wrote by answering asks — an
/// *always allow* under a scope, a *never*, a written condition — are
/// applied to the profile's answer on the `evaluate` path, and every
/// answer they give is tallied under its scope so the charter can offer
/// the next widening. A charter widens only what the profile would have
/// asked about; the roster's `max` is folded over it afterwards, so the
/// widening is capped by the same arithmetic that caps the profile.
pub struct CockpitPolicy {
    profile: PermissionProfile,
    auto_rules: Vec<AutoAllowRule>,
    gate_commands: Vec<String>,
    protected_paths: Vec<PathBuf>,
    /// The session's working directory — what a relative path the agent
    /// names is relative to. `None` compares relative targets by their
    /// trailing components instead.
    root: Option<PathBuf>,
    workflow: bool,
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

impl CockpitPolicy {
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
            workflow,
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

impl CockpitPolicy {
    /// What this machine's own configuration says, before the org's
    /// policy gets a word in. The second half of the pair is the
    /// decision to audit, present only on the `evaluate` path — the
    /// protected-file and auto-approve shortcuts never logged and
    /// continue not to.
    fn local_effect(
        &self,
        request: &AcpPermissionRequest,
        act: &PolicyRequest,
    ) -> (PolicyEffect, Option<PermissionDecision>) {
        // Protected files (e.g. the roadmap YAML — managed by MCP tools)
        // are blocked before anything else can allow them.
        if is_write_to_protected_file(
            &request.tool_title,
            &act.paths,
            request.tool_kind.as_ref(),
            &self.protected_paths,
            self.root.as_deref(),
        ) {
            return (PolicyEffect::Deny, None);
        }
        if should_auto_approve(request, self.workflow) {
            return (PolicyEffect::Allow, None);
        }
        let decision = evaluate(
            &request.tool_title,
            &request.tool_input,
            self.profile,
            &self.auto_rules,
            &self.gate_commands,
        );
        let effect = match decision.action {
            ProfileAction::Allow => PolicyEffect::Allow,
            ProfileAction::Ask => PolicyEffect::RequireApproval,
        };
        // The reader's charter: a delegation that matches replaces the
        // profile's answer and is what the audit names. The protected
        // files and the workflow shortcuts above never reach it — a
        // charter widens what would have *asked*, nothing else.
        let widened = self.charter.apply(effect, act);
        match widened.binding {
            Some(rule) => (
                widened.effect,
                Some(PermissionDecision {
                    tool_name: decision.tool_name,
                    risk: decision.risk,
                    action: if widened.effect == PolicyEffect::Allow {
                        ProfileAction::Allow
                    } else {
                        ProfileAction::Ask
                    },
                    reason: DecisionReason::Delegation(rule),
                }),
            ),
            None => (effect, Some(decision)),
        }
    }

    /// The charter's scope for `request`, cut against the session's
    /// root.
    fn scope_of(&self, request: &AcpPermissionRequest) -> Option<DelegationScope> {
        DelegationScope::of(
            &request.tool_title,
            &request.tool_input,
            self.root.as_deref(),
        )
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

impl SessionPolicy for CockpitPolicy {
    fn decide(&mut self, request: &AcpPermissionRequest) -> PolicyDecision {
        // The act as the condition language sees it, built once: the
        // charter and the roster read the same fields.
        let act = policy_request_for(
            &request.tool_title,
            &request.tool_input,
            self.model.as_deref(),
        );
        let (local, logged) = self.local_effect(request, &act);
        // The tier the cockpit acts on is the one this policy already
        // classified — the same `classify_risk` that decides what the
        // audit entry says. Carried rather than re-derived downstream,
        // so a checkpoint before a risky allow and the
        // audit row can never disagree about the same call.
        let risk = risk_tier(classify_risk(&request.tool_title, &request.tool_input));

        if self.governance.is_empty() {
            if let Some(decision) = logged {
                self.log(&decision);
            }
            return effect_to_decision(local, risk);
        }

        let governed = self.governance.evaluate(&act);
        let effect = governed.effect.tighten(local);

        match (&governed.binding, logged) {
            // A policy rule fired: audit it by rule id, so the compliance
            // report can count violations without a second log.
            (Some(rule), _) => {
                self.log(&PermissionDecision {
                    tool_name: request.tool_title.clone(),
                    risk: classify_risk(&request.tool_title, &request.tool_input),
                    action: if effect == PolicyEffect::Allow {
                        ProfileAction::Allow
                    } else {
                        ProfileAction::Ask
                    },
                    reason: DecisionReason::GovernancePolicy(rule.clone()),
                });
            }
            (None, Some(decision)) => self.log(&decision),
            (None, None) => {}
        }

        effect_to_decision(effect, risk)
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

/// Map a resolved effect onto the cockpit's three-way answer. `Warn` is
/// permitted-but-recorded: the audit entry is the point of it, so the
/// session is not interrupted.
fn effect_to_decision(effect: PolicyEffect, risk: RiskTier) -> PolicyDecision {
    match effect {
        PolicyEffect::Allow | PolicyEffect::Warn => PolicyDecision::Allow,
        PolicyEffect::RequireApproval => PolicyDecision::Ask(risk),
        PolicyEffect::Deny => PolicyDecision::Deny,
    }
}

/// The CLI's classification as the engine layer names it. The two
/// enums are the same three tiers under two names — the engine cannot
/// depend on the CLI's, which is what `RiskTier`'s own doc says — so
/// this is the one place they are joined.
const fn risk_tier(risk: RiskLevel) -> RiskTier {
    match risk {
        RiskLevel::Low => RiskTier::Low,
        RiskLevel::Medium => RiskTier::Medium,
        RiskLevel::High => RiskTier::High,
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

    let is_write_tool = tool_kind == Some(&ToolKind::Edit)
        || tool_name.eq_ignore_ascii_case("write")
        || tool_name.eq_ignore_ascii_case("edit");

    if !is_write_tool {
        return false;
    }

    // Use the same complete path set as governance. A multi-file patch may
    // touch a protected target after an ordinary file or through a rename.
    paths.iter().any(|file_path| {
        let target = resolve_against(root, Path::new(file_path));
        protected_paths.iter().any(|p| {
            let protected = resolve_against(root, p);
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
#[path = "cockpit_policy.tests.rs"]
mod tests;
