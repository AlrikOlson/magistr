//! Shared context layer — allows agents to read each other's findings.
//!
//! The `SharedContext` is a thread-safe scratchpad where workers can publish
//! findings and query what other workers have discovered, avoiding duplicated work.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::topology::AgentId;

// ─── Finding ───────────────────────────────────────────────────────────────

/// A single finding published by a worker agent.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Which agent published this finding.
    pub agent_id: AgentId,
    /// Key for retrieval (e.g., "api_pattern", "test_strategy").
    pub key: String,
    /// Free-form text value.
    pub value: String,
    /// When this finding was published.
    pub published_at: Instant,
}

// ─── Shared Context ────────────────────────────────────────────────────────

/// Thread-safe shared context for inter-agent knowledge sharing.
///
/// Workers publish findings with a key-value interface. Other workers
/// (or the supervisor) can query for findings by key or by agent.
#[derive(Debug, Clone)]
pub struct SharedContext {
    /// Inner state protected by a read-write lock.
    inner: Arc<RwLock<ContextInner>>,
}

#[derive(Debug, Default)]
struct ContextInner {
    /// All findings, keyed by their key string.
    findings: HashMap<String, Vec<Finding>>,
}

impl SharedContext {
    /// Create a new empty shared context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextInner::default())),
        }
    }

    /// Publish a finding to the shared context.
    pub fn publish(&self, agent_id: AgentId, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let finding = Finding {
            agent_id,
            key: key.clone(),
            value: value.into(),
            published_at: Instant::now(),
        };
        let mut inner = self.inner.write().expect("shared context lock poisoned");
        inner.findings.entry(key).or_default().push(finding);
    }

    /// Query all findings with a given key.
    #[must_use]
    pub fn query(&self, key: &str) -> Vec<Finding> {
        let inner = self.inner.read().expect("shared context lock poisoned");
        inner.findings.get(key).cloned().unwrap_or_default()
    }

    /// Query all findings from a specific agent.
    #[must_use]
    pub fn findings_by_agent(&self, agent_id: AgentId) -> Vec<Finding> {
        let inner = self.inner.read().expect("shared context lock poisoned");
        inner
            .findings
            .values()
            .flatten()
            .filter(|f| f.agent_id == agent_id)
            .cloned()
            .collect()
    }

    /// Get all keys that have findings.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let inner = self.inner.read().expect("shared context lock poisoned");
        inner.findings.keys().cloned().collect()
    }

    /// Get the total number of findings.
    #[must_use]
    pub fn total_findings(&self) -> usize {
        let inner = self.inner.read().expect("shared context lock poisoned");
        inner.findings.values().map(|v| v.len()).sum()
    }

    /// Clear all findings.
    pub fn clear(&self) {
        let mut inner = self.inner.write().expect("shared context lock poisoned");
        inner.findings.clear();
    }

    /// Build a context injection string containing all findings relevant to
    /// the given role and task description.
    ///
    /// Filters findings based on role-key affinity:
    /// - `"researcher"` → all findings
    /// - `"coder"` → research_findings, api_pattern, architecture
    /// - `"reviewer"` → coder_output, research_findings
    /// - `"tester"` → coder_output, test_strategy
    /// - Others → all findings
    ///
    /// Returns an empty string if no relevant findings exist.
    #[must_use]
    pub fn build_injection(&self, role: &str, _task_description: &str) -> String {
        let inner = self.inner.read().expect("shared context lock poisoned");
        if inner.findings.is_empty() {
            return String::new();
        }

        let relevant_keys: Option<&[&str]> = match role {
            "coder" => Some(&["research_findings", "api_pattern", "architecture"]),
            "reviewer" => Some(&["coder_output", "research_findings"]),
            "tester" => Some(&["coder_output", "test_strategy"]),
            _ => None, // researcher and others get everything
        };

        let findings: Vec<&Finding> = inner
            .findings
            .iter()
            .filter(|(key, _)| match relevant_keys {
                Some(keys) => keys.iter().any(|k| key.contains(k)),
                None => true,
            })
            .flat_map(|(_, v)| v.iter())
            .collect();

        if findings.is_empty() {
            return String::new();
        }

        let mut out = String::from("## Shared Context (from other agents)\n\n");
        for (i, f) in findings.iter().enumerate() {
            out.push_str(&format!(
                "### Finding {} ({})\n\n{}\n\n",
                i + 1,
                f.key,
                f.value
            ));
        }
        out
    }

    /// Extract key results from a completed agent's summary and publish them.
    ///
    /// Publishes the summary under the agent's role key (e.g., `"coder_output"`,
    /// `"research_findings"`).
    pub fn extract_and_publish(&self, agent_id: AgentId, role: &str, summary: &str) {
        let key = match role {
            "researcher" => "research_findings",
            "coder" => "coder_output",
            "reviewer" => "review_feedback",
            "tester" => "test_results",
            "documenter" => "documentation",
            _ => "agent_output",
        };
        self.publish(agent_id, key, summary);
    }
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "context.tests.rs"]
mod tests;
