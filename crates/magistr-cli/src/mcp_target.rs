/// The portable launch definition shared by every integration.
#[derive(Debug, Clone, Copy)]
pub struct McpTarget;

impl McpTarget {
    #[must_use]
    pub fn command(self) -> &'static str {
        if npm_installation() { "npx" } else { "magistr" }
    }

    #[must_use]
    pub fn args(self, project_root: &str) -> Vec<String> {
        let mut args = npm_prefix();
        args.extend(["mcp".into(), "--dir".into(), project_root.into()]);
        args
    }

    /// Recognize generated registrations while retaining customized launchers.
    pub(crate) fn owns_entry(self, root: &std::path::Path, entry: &serde_json::Value) -> bool {
        let Some(object) = entry.as_object() else {
            return false;
        };
        if object.len() != 3 || entry["type"] != "stdio" {
            return false;
        }
        let Some(args) = entry["args"].as_array() else {
            return false;
        };
        let args = match entry["command"].as_str() {
            Some("magistr") => args.as_slice(),
            Some("npx")
                if args.len() == 7
                    && args[0] == "--yes"
                    && args[1] == "--package"
                    && args[3] == "magistr"
                    && args[2]
                        .as_str()
                        .and_then(|s| s.strip_prefix("magistr@"))
                        .is_some_and(|v| {
                            let parts: Vec<_> = v.split('.').collect();
                            parts.len() == 3
                                && parts
                                    .iter()
                                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                        }) =>
            {
                &args[4..]
            }
            _ => return false,
        };
        matches!(args, [verb, flag, directory] if verb == "mcp" && flag == "--dir"
            && directory.as_str().is_some_and(|p| p == "." || std::path::Path::new(p) == root))
    }

    #[must_use]
    pub fn mcp_entry(self, project_root: &str) -> serde_json::Value {
        serde_json::json!({"command":self.command(),"args":self.args(project_root),"type":"stdio"})
    }
}

/// npm installs include a version-matched native executable. Persist an npx
/// invocation so a one-shot init continues to work after npm clears its cache.
fn npm_installation() -> bool {
    let Ok(executable) = std::env::current_exe().and_then(|path| path.canonicalize()) else {
        return false;
    };
    let Some(package) = executable.parent().and_then(std::path::Path::parent) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(package.join("package.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    value["name"] == "magistr"
        && value["version"] == env!("CARGO_PKG_VERSION")
        && value["bin"]["magistr"] == "bin/magistr"
}
fn npm_prefix() -> Vec<String> {
    if npm_installation() {
        vec![
            "--yes".into(),
            "--package".into(),
            format!("magistr@{}", env!("CARGO_PKG_VERSION")),
            "magistr".into(),
        ]
    } else {
        Vec::new()
    }
}
pub(crate) fn shell_command() -> String {
    std::iter::once(McpTarget.command().to_owned())
        .chain(npm_prefix())
        .map(|arg| shell_escape::escape(arg.into()).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve a portable "." launch from inside its project, stopping at a working-copy boundary.
pub(crate) fn project_root(dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let dir = dir.canonicalize()?;
    if !dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Choose a project directory",
        ));
    }
    Ok(dir
        .ancestors()
        .find(|root| root.join(".magistr/project.json").is_file() || root.join(".git").exists())
        .unwrap_or(&dir)
        .to_path_buf())
}
