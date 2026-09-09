use super::{ScaffoldFile, UpdatePolicy};

macro_rules! workflow {
    ($name:literal) => {
        (
            concat!(".agents/skills/", $name, "/SKILL.md"),
            concat!(".claude/skills/", $name, "/SKILL.md"),
            include_str!(concat!("../../templates/workflows/", $name, ".md")),
        )
    };
}

const WORKFLOWS: &[(&str, &str, &str)] = &[
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

#[derive(Clone, Copy)]
enum Host {
    Codex,
    Claude,
}

impl Host {
    fn invocation(self) -> &'static str {
        match self {
            Self::Codex => "$",
            Self::Claude => "/",
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
pub(super) struct WorkflowSkills;

impl WorkflowSkills {
    pub(super) fn codex_files() -> Vec<ScaffoldFile> {
        Self::files(Host::Codex)
    }

    pub(super) fn claude_files() -> Vec<ScaffoldFile> {
        Self::files(Host::Claude)
    }

    pub(super) fn is_claude_workflow(path: &str) -> bool {
        WORKFLOWS.iter().any(|(_, claude, _)| *claude == path)
    }

    fn files(host: Host) -> Vec<ScaffoldFile> {
        let mut files: Vec<_> = WORKFLOWS
            .iter()
            .map(|(codex, claude, content)| {
                Self::file(
                    host,
                    host.path(codex, claude),
                    content.replace("{{invoke}}", host.invocation()),
                )
            })
            .collect();
        files.push(Self::file(
            host,
            host.path(
                ".agents/skills/workbench-routing/SKILL.md",
                ".claude/skills/workbench-routing/SKILL.md",
            ),
            format!(
                "{}\n<!-- vendored verbatim from {} — re-run `magistr init` after upgrading workbench-mcp -->\n",
                super::STATIC_SKILL_WORKBENCH_ROUTING.trim_end(),
                super::workbench::ROUTING_SKILL_SOURCE
            ),
        ));
        files.push(Self::file(
            host,
            host.path(
                ".agents/skills/craft/references/runtime.md",
                ".claude/skills/craft/references/runtime.md",
            ),
            format!(
                "{}\n\n<!-- Host adapter -->\n\n{}",
                RUNTIME.trim_end(),
                host.adapter()
            ),
        ));
        files
    }

    fn file(host: Host, path: &'static str, content: String) -> ScaffoldFile {
        ScaffoldFile {
            path,
            content,
            executable: false,
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
