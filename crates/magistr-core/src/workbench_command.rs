//! How Magistr launches workbench-mcp, the navigation/edit substrate — shared
//! by the scaffold (`.mcp.json`), the interview MCP config and the GUI's
//! sandboxed sessions, so every surface resolves the same launch line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Environment override: an explicit command to launch workbench-mcp with.
const COMMAND_VAR: &str = "WORKBENCH_MCP";
/// Environment override: a checkout of the workbench-mcp repository, run
/// through `bun run <repo>/src/main.ts`.
const REPO_VAR: &str = "WORKBENCH_MCP_REPO";
/// The default checkout location used when nothing is installed.
const KNOWN_CHECKOUT: &str = "Serious_Code/workbench-mcp";
/// The resolved launch line for the `workbench-mcp` MCP server entry.
///
/// Resolution order: `WORKBENCH_MCP` (explicit command) → `workbench-mcp` on
/// PATH → `WORKBENCH_MCP_REPO` (or the known checkout) run with `bun` → the
/// bare `workbench-mcp` name with an install hint. The scaffold always emits
/// an entry — a project configured before the tool is installed starts
/// working the moment it is — and surfaces the hint when nothing resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchCommand {
    /// The executable: `workbench-mcp`, `bun`, or an explicit override.
    pub command: String,
    /// Arguments that precede the workspace root (`["run", "<main.ts>"]` for
    /// a checkout; empty for the installed binary).
    pub leading_args: Vec<String>,
    /// Whether the command was actually found on this machine.
    pub available: bool,
}

impl WorkbenchCommand {
    /// Resolve from the process environment and PATH.
    #[must_use]
    pub fn resolve() -> Self {
        Self::resolve_with(
            |key| std::env::var(key).ok(),
            |bin| which::which(bin).is_ok(),
            Path::is_file,
        )
    }

    /// Resolve through injected lookups so every arm is testable without
    /// touching the environment or PATH.
    #[must_use]
    pub fn resolve_with(
        get_env: impl Fn(&str) -> Option<String>,
        on_path: impl Fn(&str) -> bool,
        is_file: impl Fn(&Path) -> bool,
    ) -> Self {
        let non_empty = |key: &str| get_env(key).filter(|v| !v.trim().is_empty());
        if let Some(cmd) = non_empty(COMMAND_VAR) {
            return Self {
                command: cmd.trim().to_owned(),
                leading_args: Vec::new(),
                available: true,
            };
        }
        if on_path("workbench-mcp") {
            return Self::installed(true);
        }
        let repo = non_empty(REPO_VAR)
            .map(PathBuf::from)
            .or_else(|| non_empty("HOME").map(|home| PathBuf::from(home).join(KNOWN_CHECKOUT)));
        if let Some(repo) = repo {
            let main = repo.join("src").join("main.ts");
            if is_file(&main) && on_path("bun") {
                return Self {
                    command: "bun".to_owned(),
                    leading_args: vec!["run".to_owned(), main.to_string_lossy().replace('\\', "/")],
                    available: true,
                };
            }
        }
        Self::installed(false)
    }

    fn installed(available: bool) -> Self {
        Self {
            command: "workbench-mcp".to_owned(),
            leading_args: Vec::new(),
            available,
        }
    }

    /// Rewrite any leading argument that is a path inside `dir` as a path
    /// relative to it.
    ///
    /// A `.mcp.json` is a committed file, so a machine's absolute path in it
    /// travels to every clone; when the workbench checkout being launched IS
    /// the project being scaffolded (the dogfooding case), `bun run
    /// /Users/x/repo/src/main.ts` becomes `bun run src/main.ts`, which works
    /// on every machine. A checkout outside `dir` keeps its absolute path —
    /// there is nothing to make it relative to.
    #[must_use]
    pub fn relative_to(mut self, dir: &Path) -> Self {
        let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        for arg in &mut self.leading_args {
            let path = PathBuf::from(arg.as_str());
            if !path.is_absolute() {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or(path);
            if let Ok(rel) = canonical.strip_prefix(&root) {
                *arg = rel.to_string_lossy().replace('\\', "/");
            }
        }
        self
    }

    /// Build a process before changing cwd, anchoring executable discovery and
    /// checkout paths to the caller's directory. The child runs in the workspace.
    pub fn process_command(&self, project_root: &Path) -> std::io::Result<std::process::Command> {
        let executable = which::which(&self.command)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::NotFound, error))?;
        // which may return a relative path when PATH itself has relative entries.
        // Anchor it without resolving symlinks: some tools dispatch by argv[0].
        let executable = std::env::current_dir()?.join(executable);
        let mut command = std::process::Command::new(executable);
        match self.leading_args.as_slice() {
            // The checkout form belongs to this resolver. Resolve its script
            // now so a relative WORKBENCH_MCP_REPO survives the child's chdir.
            [verb, script] if verb == "run" => {
                command.arg(verb).arg(Path::new(script).canonicalize()?);
            }
            args => {
                command.args(args);
            }
        }
        command.arg(project_root).current_dir(project_root);
        Ok(command)
    }

    /// The full argument list for a workspace root.
    #[must_use]
    pub fn args(&self, project_root: &str) -> Vec<String> {
        let mut args = self.leading_args.clone();
        args.push(project_root.to_owned());
        args
    }

    /// The `args` array body as a JSON fragment for explicit scaffold overrides
    /// (`"run", "/x/main.ts", "/root"`), each element JSON-escaped.
    #[must_use]
    pub fn args_json_fragment(&self, project_root: &str) -> String {
        self.args(project_root)
            .iter()
            .map(|a| serde_json::Value::String(a.clone()).to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The `.mcp.json` server entry.
    #[must_use]
    pub fn mcp_entry(&self, project_root: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "stdio",
            "command": self.command,
            "args": self.args(project_root),
        })
    }

    /// Put `workbench_command` / `workbench_args` into the template vars
    /// unless the caller already supplied them.
    pub fn inject_template_vars(&self, vars: &mut HashMap<String, String>, project_root: &str) {
        vars.entry("workbench_command".to_owned())
            .or_insert_with(|| self.command.clone());
        vars.entry("workbench_args".to_owned())
            .or_insert_with(|| self.args_json_fragment(project_root));
    }

    /// What to tell the user when nothing resolved.
    #[must_use]
    pub fn install_hint(&self) -> Option<String> {
        (!self.available).then(|| {
            format!(
                "workbench-mcp was not found. Install it (`bun install -g workbench-mcp`), \
                 or point {REPO_VAR} at a checkout, or set {COMMAND_VAR} to the launch command; \
                 the scaffolded .mcp.json entry starts working as soon as it resolves."
            )
        })
    }
}

#[cfg(test)]
#[path = "workbench_command.tests.rs"]
mod tests;
