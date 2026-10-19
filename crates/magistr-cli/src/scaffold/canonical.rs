//! The canonical inputs a target repository carries for files the scaffold
//! would otherwise render from copies embedded in this binary.
//!
//! Magistr source checkouts render their live workflow and routing templates.
//! Installed packages render the same embedded templates. Inputs are captured
//! once per plan, so preview and application agree without a second owner.
//!
//! A canonical input that exists but cannot be read, or one the source
//! checkout is missing, is a refusal for the paths rendered from it: the
//! embedded copy is never written in its place.

use std::collections::BTreeMap;
use std::path::Path;

use super::Refusal;
use super::workflow_skills::{Host, WorkflowSkills};

/// Where Magistr's own source checkout keeps the workflow templates.
pub(crate) const WORKFLOW_TEMPLATES: &str = "crates/magistr-cli/templates/workflows";

/// Where Magistr's own source checkout keeps the vendored routing skill.
const ROUTING_TEMPLATE: &str = "crates/magistr-cli/templates/skills/workspace-routing/SKILL.md";

/// One canonical input as it was read while planning.
#[derive(Debug, Clone)]
enum Canonical {
    /// The rendered content the plan must use.
    Source(String),
    /// The input is missing or unreadable; the paths rendered from it are refused.
    Unavailable { source: String, cause: String },
}

/// The canonical inputs captured for one plan.
#[derive(Debug, Clone, Default)]
pub(crate) struct CanonicalSource {
    inputs: BTreeMap<&'static str, Canonical>,
}

impl CanonicalSource {
    /// Read every canonical input `dir` carries, once.
    pub(crate) fn capture(dir: &Path) -> Self {
        let mut inputs = BTreeMap::new();
        if is_dir(&dir.join(WORKFLOW_TEMPLATES)) {
            for host in [Host::Codex, Host::Claude] {
                Self::capture_workflows(dir, host, &mut inputs);
            }
        }
        Self { inputs }
    }

    fn capture_workflows(dir: &Path, host: Host, inputs: &mut BTreeMap<&'static str, Canonical>) {
        for (name, path) in WorkflowSkills::workflow_paths(host) {
            let source = format!("{WORKFLOW_TEMPLATES}/{name}.md");
            let rendered = read(dir, &source, |template| {
                WorkflowSkills::render_workflow(host, &template)
            });
            inputs.insert(path, rendered.unwrap_or_else(|| missing(&source)));
        }
        let runtime = format!("{WORKFLOW_TEMPLATES}/runtime.md");
        let adapter = format!("{WORKFLOW_TEMPLATES}/hosts/{}.md", host.name());
        let rendered = match (
            read(dir, &runtime, |content| content),
            read(dir, &adapter, |content| content),
        ) {
            (Some(Canonical::Source(runtime)), Some(Canonical::Source(adapter))) => {
                Canonical::Source(WorkflowSkills::render_runtime(&runtime, &adapter))
            }
            (Some(unavailable @ Canonical::Unavailable { .. }), _)
            | (_, Some(unavailable @ Canonical::Unavailable { .. })) => unavailable,
            (None, _) => missing(&runtime),
            (_, None) => missing(&adapter),
        };
        inputs.insert(WorkflowSkills::runtime_path(host), rendered);
        let routing = read(dir, ROUTING_TEMPLATE, |template| {
            WorkflowSkills::render_routing(&template)
        });
        inputs.insert(
            WorkflowSkills::routing_path(host),
            routing.unwrap_or_else(|| missing(ROUTING_TEMPLATE)),
        );
    }

    /// Whether the plan renders `path` from a canonical input rather than
    /// the embedded copy.
    pub(crate) fn is_canonical(&self, path: &str) -> bool {
        self.inputs.contains_key(path)
    }

    /// The content the plan must use for `path`: the canonical rendering
    /// when the target carries one, `embedded` everywhere else, or the
    /// refusal that keeps a stale embedded copy off a canonical path.
    pub(crate) fn resolve(&self, path: &str, embedded: String) -> Result<String, Refusal> {
        match self.inputs.get(path) {
            None => Ok(embedded),
            Some(Canonical::Source(content)) => Ok(content.clone()),
            Some(Canonical::Unavailable { source, cause }) => Err(Refusal::CanonicalUnavailable {
                source: source.clone(),
                cause: cause.clone(),
            }),
        }
    }
}

/// Read one canonical input: `None` when the target does not carry it,
/// the rendered content when it does, unavailable when it is there but
/// cannot be read as a regular file.
fn read(dir: &Path, source: &str, render: impl FnOnce(String) -> String) -> Option<Canonical> {
    match super::CodexSetup::read(dir, source) {
        Ok(None) => None,
        Ok(Some(content)) => Some(Canonical::Source(render(content))),
        Err(error) => Some(Canonical::Unavailable {
            source: source.to_owned(),
            cause: error.to_string(),
        }),
    }
}

fn missing(source: &str) -> Canonical {
    Canonical::Unavailable {
        source: source.to_owned(),
        cause: "missing from the source checkout".to_owned(),
    }
}

fn is_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

#[cfg(test)]
#[path = "canonical.tests.rs"]
mod tests;
