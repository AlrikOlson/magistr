//! Purpose-derived authority applied by shared session startup.

use magistr_application::sessions::{
    PermissionKind, PolicyDecision, SessionPolicy, SessionPolicyRequest, SessionPurpose,
};

use super::{AgentConfig, ToolPermissions};
use crate::permissions::SessionPermissionPolicy;

const INSTRUCTIONS: &str = "You are updating the project's roadmap in response to the user's request. Read the current native roadmap and relevant source context, make the requested planning changes through the native roadmap tools, and explain what changed in plain language. Keep existing in-progress work intact. Do not implement code, start or complete work items, run implementation workflows or gates, or create/export ROADMAP.md or ROADMAP.json. This is a planning conversation, not permission to begin the planned work. Record meaningful findings and explored alternatives when they inform the plan. Ask a clear question if essential information is missing. Finish with a brief account of the changes and what remains ready or unfinished.";

/// Apply the intent already held by history without replacing its identity or resume token.
pub(super) fn apply(config: &mut AgentConfig) {
    if config.history.purpose() != SessionPurpose::Roadmap {
        return;
    }
    config.workflow_permissions = false;
    config.allowed_tools = ToolPermissions::for_roadmap_request().into_strings();
    config.system_prompt = Some(match &config.system_prompt {
        Some(context) => format!("{context}\n\n{INSTRUCTIONS}"),
        None => INSTRUCTIONS.to_owned(),
    });
}

/// The request's purpose remains a hard ceiling even under a permissive project
/// profile. Allowed tools still pass through standing refusals and governance.
pub(super) struct RoadmapPolicy {
    inner: SessionPermissionPolicy,
}

impl RoadmapPolicy {
    pub(super) fn new(inner: SessionPermissionPolicy) -> Self {
        Self { inner }
    }

    fn permitted(request: &SessionPolicyRequest) -> bool {
        let title = request
            .tool_title
            .split(':')
            .next()
            .unwrap_or_default()
            .trim();
        let name = title.rsplit("__").next().unwrap_or(title);
        ToolPermissions::for_roadmap_request().contains(name)
    }
}

impl SessionPolicy for RoadmapPolicy {
    fn can_use_model(&self, model: &str) -> bool {
        self.inner.can_use_model(model)
    }
    fn model_changed(&mut self, model: Option<String>) {
        self.inner.model_changed(model);
    }

    fn decide(&mut self, request: &SessionPolicyRequest) -> PolicyDecision {
        if Self::permitted(request) {
            self.inner.decide(request)
        } else {
            PolicyDecision::Deny
        }
    }

    fn answered(&mut self, request: &SessionPolicyRequest, kind: PermissionKind) {
        if Self::permitted(request) {
            self.inner.answered(request, kind);
        }
    }

    fn constrain(&mut self, request: &SessionPolicyRequest, condition: &str) {
        if Self::permitted(request) {
            self.inner.constrain(request, condition);
        }
    }
}

#[cfg(test)]
#[path = "purpose_policy.tests.rs"]
mod tests;
