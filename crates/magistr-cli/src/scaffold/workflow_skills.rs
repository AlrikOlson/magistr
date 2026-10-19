use super::{ScaffoldFile, UpdatePolicy};

macro_rules! workflow {
    ($name:literal) => {
        (
            $name,
            concat!(".agents/skills/", $name, "/SKILL.md"),
            concat!(".claude/skills/", $name, "/SKILL.md"),
            include_str!(concat!("../../templates/workflows/", $name, ".md")),
        )
    };
}

/// `(name, Codex path, Claude path, embedded template)` per workflow.
const WORKFLOWS: &[(&str, &str, &str, &str)] = &[
    workflow!("craft"),
    workflow!("init"),
    workflow!("next"),
    workflow!("next-loop"),
    workflow!("roadmap"),
    workflow!("roadmap-refresh"),
    workflow!("signals"),
    workflow!("validate"),
];
const RUNTIME: &str = include_str!("../../templates/workflows/runtime.md");

/// The agent host a workflow skill is rendered for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Host {
    Codex,
    Claude,
}

impl Host {
    /// The character that invokes a skill on this host.
    pub(super) fn invocation(self) -> &'static str {
        match self {
            Self::Codex => "$",
            Self::Claude => "/",
        }
    }

    /// The host's name as its adapter template is filed.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn path(self, codex: &'static str, claude: &'static str) -> &'static str {
        match self {
            Self::Codex => codex,
            Self::Claude => claude,
        }
    }

    fn adapter(self) -> &'static str {
        match self {
            Self::Codex => include_str!("../../templates/workflows/hosts/codex.md"),
            Self::Claude => include_str!("../../templates/workflows/hosts/claude.md"),
        }
    }
}

/// Host adapters over the single maintained Magistr workflow contract.
///
/// The same rendering serves the embedded templates and a source checkout's
/// live ones (`canonical::CanonicalSource`), so the two never diverge in
/// what they substitute.
pub(super) struct WorkflowSkills;

impl WorkflowSkills {
    pub(super) fn codex_files() -> Vec<ScaffoldFile> {
        Self::files(Host::Codex)
    }

    pub(super) fn claude_files() -> Vec<ScaffoldFile> {
        Self::files(Host::Claude)
    }

    /// `(workflow name, scaffold path)` for every workflow skill of `host`.
    pub(super) fn workflow_paths(host: Host) -> Vec<(&'static str, &'static str)> {
        WORKFLOWS
            .iter()
            .map(|(name, codex, claude, _)| (*name, host.path(codex, claude)))
            .collect()
    }

    /// The scaffold path of `host`'s vendored routing skill.
    pub(super) fn routing_path(host: Host) -> &'static str {
        host.path(
            ".agents/skills/workbench-routing/SKILL.md",
            ".claude/skills/workbench-routing/SKILL.md",
        )
    }

    /// The scaffold path of `host`'s shared workflow runtime.
    pub(super) fn runtime_path(host: Host) -> &'static str {
        host.path(
            ".agents/skills/craft/references/runtime.md",
            ".claude/skills/craft/references/runtime.md",
        )
    }

    /// Render one workflow template for `host`.
    pub(super) fn render_workflow(host: Host, template: &str) -> String {
        template.replace("{{invoke}}", host.invocation()).replace(
            "{{next_recovery_guidance}}",
            magistr_engine::prompt::PromptConfig::NEXT_RECOVERY_GUIDANCE.trim_end(),
        )
    }

    /// Render the shared runtime with a host adapter appended.
    pub(super) fn render_runtime(runtime: &str, adapter: &str) -> String {
        format!(
            "{}\n\n<!-- Host adapter -->\n\n{}",
            runtime.trim_end().replace(
                "{{reasoning_guidance}}",
                magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE
            ),
            adapter
        )
    }

    /// Render the vendored routing skill with its provenance marker.
    pub(super) fn render_routing(template: &str) -> String {
        format!(
            "{}\n<!-- maintained with {} — run `magistr repair` after upgrading Magistr -->\n",
            template.trim_end(),
            super::workbench::ROUTING_SKILL_SOURCE
        )
    }

    fn files(host: Host) -> Vec<ScaffoldFile> {
        let mut files: Vec<_> = WORKFLOWS
            .iter()
            .map(|(_, codex, claude, content)| {
                Self::file(
                    host,
                    host.path(codex, claude),
                    Self::render_workflow(host, content),
                )
            })
            .collect();
        files.push(Self::file(
            host,
            Self::routing_path(host),
            Self::render_routing(super::STATIC_SKILL_WORKBENCH_ROUTING),
        ));
        files.push(Self::file(
            host,
            Self::runtime_path(host),
            Self::render_runtime(RUNTIME, host.adapter()),
        ));
        files
    }

    fn file(host: Host, path: &'static str, content: String) -> ScaffoldFile {
        ScaffoldFile {
            path,
            content,
            policy: match host {
                Host::Codex => UpdatePolicy::PreserveCustomized,
                Host::Claude => UpdatePolicy::Overwrite,
            },
        }
    }
}

#[cfg(test)]
#[path = "workflow_skills.tests.rs"]
mod tests;
