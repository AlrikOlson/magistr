//! Codex's regular scaffold files; shared config and instructions have their
//! own component ownership in `CodexSetup`.

use std::collections::HashMap;

use super::emitters::AgentEmitter;
use super::workflow_skills::WorkflowSkills;
use super::{ScaffoldFile, UpdatePolicy};

pub(crate) struct CodexEmitter;

impl AgentEmitter for CodexEmitter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn files(&self, _vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        let mut files = WorkflowSkills::codex_files();
        files.push(ScaffoldFile {
            path: ".codex/run",
            content: format!(
                "#!/bin/sh\nexec {} codex-launch --project-root \"$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\" -- \"$@\"\n",
                crate::mcp_target::shell_command(),
            ),
            policy: UpdatePolicy::PreserveCustomized,
        });
        files
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        self.files(&HashMap::new())
            .into_iter()
            .map(|file| file.path)
            .collect()
    }
}

#[cfg(test)]
#[path = "codex_emitter.tests.rs"]
mod tests;
