use super::risk_tier::RiskTier;

/// The thing a policy is evaluated against: one governed act.
///
/// Every field is optional because policy runs at several points in the
/// system — a tool call knows its tool name and maybe a command or a
/// path, a session start knows only its model, a gate run knows only its
/// gate name. A condition over a field the request does not carry never
/// matches, so an under-populated request can only ever be *more*
/// permissive than a fully populated one — never less.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyRequest {
    /// The tool being invoked, e.g. `Bash` or `mcp__magistr__gate`.
    pub tool: Option<String>,
    /// The shell command, when the tool runs one.
    pub command: Option<String>,
    /// Every filesystem path the act touches.
    pub paths: Vec<String>,
    /// The model the session runs on.
    pub model: Option<String>,
    /// The caller's own risk classification.
    pub risk: Option<RiskTier>,
    /// The quality gate being run.
    pub gate: Option<String>,
}

impl PolicyRequest {
    /// An empty request — nothing is known yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A request for a tool invocation.
    #[must_use]
    pub fn tool(name: impl Into<String>) -> Self {
        Self {
            tool: Some(name.into()),
            ..Self::default()
        }
    }

    /// Set the shell command (self-consuming builder).
    #[must_use]
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Add a path the act touches (self-consuming builder).
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Set the model (self-consuming builder).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the risk tier (self-consuming builder).
    #[must_use]
    pub fn with_risk(mut self, risk: RiskTier) -> Self {
        self.risk = Some(risk);
        self
    }

    /// Set the gate name (self-consuming builder).
    #[must_use]
    pub fn with_gate(mut self, gate: impl Into<String>) -> Self {
        self.gate = Some(gate.into());
        self
    }

    /// A short human-readable summary, for audit entries and reports.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(tool) = &self.tool {
            parts.push(format!("tool={tool}"));
        }
        if let Some(command) = &self.command {
            parts.push(format!("command={command}"));
        }
        if !self.paths.is_empty() {
            parts.push(format!("paths={}", self.paths.join(",")));
        }
        if let Some(model) = &self.model {
            parts.push(format!("model={model}"));
        }
        if let Some(gate) = &self.gate {
            parts.push(format!("gate={gate}"));
        }
        if let Some(risk) = self.risk {
            parts.push(format!("risk={risk}"));
        }
        parts.join(" ")
    }
}

#[cfg(test)]
#[path = "policy_request.tests.rs"]
mod tests;
