//! Planning-only session setup for requests made from Roadmap.

use magistr_acp::AcpPermissionRequest;
use magistr_cockpit::agent::{PermissionKind, PolicyDecision, SessionPolicy};

use crate::agent::{AgentConfig, ToolPermissions};
use crate::permissions::SessionPermissionPolicy;

const INSTRUCTIONS: &str = "You are updating the project's roadmap in response to the user's request. Read the current native roadmap and relevant source context, make the requested planning changes through the native roadmap tools, and explain what changed in plain language. Keep existing in-progress work intact. Do not implement code, start or complete work items, run implementation workflows or gates, or create/export ROADMAP.md or ROADMAP.json. This is a planning conversation, not permission to begin the planned work. Record meaningful findings and explored alternatives when they inform the plan. Ask a clear question if essential information is missing. Finish with a brief account of the changes and what remains ready or unfinished.";

/// Narrow the launch configuration without replacing user provider/model choices.
pub(crate) fn config(base: &AgentConfig) -> AgentConfig {
    let mut config = base.clone();
    config.prompt.clear();
    config.resume = None;
    config.continue_conversation = false;
    config.workflow_permissions = false;
    config.planning_only = true;
    config.allowed_tools = ToolPermissions::for_roadmap_request().into_strings();
    config.system_prompt = Some(match &base.system_prompt {
        Some(context) => format!("{context}\n\n{INSTRUCTIONS}"),
        None => INSTRUCTIONS.to_owned(),
    });
    config
}

#[cfg(test)]
#[path = "roadmap_request.tests.rs"]
mod tests;

/// The request's purpose remains a hard ceiling even under a permissive project
/// profile. Allowed tools still pass through standing refusals and governance.
pub(crate) struct RoadmapPolicy {
    inner: SessionPermissionPolicy,
}

impl RoadmapPolicy {
    pub(crate) fn new(inner: SessionPermissionPolicy) -> Self {
        Self { inner }
    }

    fn permitted(request: &AcpPermissionRequest) -> bool {
        let title = request
            .tool_title
            .split(':')
            .next()
            .unwrap_or_default()
            .trim();
        let name = title.rsplit("__").next().unwrap_or(title);
        if name == "roadmap_set_status" {
            return request
                .tool_input
                .get("status")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|status| matches!(status, "backlog" | "pending"));
        }
        ToolPermissions::for_roadmap_request().contains(name)
    }
}

impl SessionPolicy for RoadmapPolicy {
    fn decide(&mut self, request: &AcpPermissionRequest) -> PolicyDecision {
        if Self::permitted(request) {
            self.inner.decide(request)
        } else {
            PolicyDecision::Deny
        }
    }

    fn answered(&mut self, request: &AcpPermissionRequest, kind: PermissionKind) {
        if Self::permitted(request) {
            self.inner.answered(request, kind);
        }
    }

    fn constrain(&mut self, request: &AcpPermissionRequest, condition: &str) {
        if Self::permitted(request) {
            self.inner.constrain(request, condition);
        }
    }
}
