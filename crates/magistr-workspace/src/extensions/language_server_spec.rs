use std::path::{Path, PathBuf};

use crate::worker::WorkerSubprocessSpec;

/// One host-curated language-server launch spec. Only the host mints these;
/// the guest selects one by language id and never names a binary or argv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServerSpec {
    pub language_id: String,
    pub bin: String,
    pub args: Vec<String>,
}

impl LanguageServerSpec {
    /// Probe the workspace for detectable servers: a TypeScript project with
    /// `typescript-language-server` on PATH, and a Cargo project (root or one
    /// level down) with a rust-analyzer that actually runs. Detection failure
    /// is an empty table, never an error.
    #[must_use]
    pub fn detect(root: &Path) -> Vec<Self> {
        Self::detect_with(root, &find_on_path, &resolve_rust_analyzer)
    }

    /// Detection with injected binary probes, so tests never shell out.
    #[must_use]
    pub fn detect_with(
        root: &Path,
        lookup: &dyn Fn(&str) -> Option<PathBuf>,
        rust: &dyn Fn(&Path) -> Option<PathBuf>,
    ) -> Vec<Self> {
        let mut specs = Vec::new();
        if (root.join("tsconfig.json").exists() || root.join("package.json").exists())
            && let Some(bin) = lookup("typescript-language-server")
        {
            specs.push(Self {
                language_id: "typescript".into(),
                bin: bin.display().to_string(),
                args: vec!["--stdio".into()],
            });
        }
        if let Some(cargo_dir) = find_cargo_project_dir(root)
            && let Some(bin) = rust(&cargo_dir)
        {
            specs.push(Self {
                language_id: "rust".into(),
                bin: bin.display().to_string(),
                args: Vec::new(),
            });
        }
        specs
    }
}

impl From<&LanguageServerSpec> for WorkerSubprocessSpec {
    fn from(spec: &LanguageServerSpec) -> Self {
        Self {
            language_id: spec.language_id.clone(),
            bin: spec.bin.clone(),
            args: spec.args.clone(),
        }
    }
}

/// The directory holding the Cargo project a root serves: the root itself,
/// else one sorted, bounded level down.
fn find_cargo_project_dir(root: &Path) -> Option<PathBuf> {
    if root.join("Cargo.toml").exists() {
        return Some(root.to_path_buf());
    }
    let mut names: Vec<String> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && name != "node_modules" && name != "target")
        .collect();
    names.sort();
    names
        .into_iter()
        .take(64)
        .map(|name| root.join(name))
        .find(|candidate| candidate.join("Cargo.toml").exists())
}

fn find_on_path(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())?
        .into_iter()
        .map(|dir| dir.join(bin))
        .find(|candidate| candidate.is_file())
}

/// Does `bin --version` exit 0? Rejects a rustup proxy whose active
/// toolchain lacks the component.
fn runs(bin: &Path) -> bool {
    std::process::Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Ask the toolchain manager first (`rustup which rust-analyzer` honors a
/// pinned toolchain), then a PATH `rust-analyzer` confirmed runnable.
fn resolve_rust_analyzer(cargo_dir: &Path) -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .current_dir(cargo_dir)
        .output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    find_on_path("rust-analyzer").filter(|bin| runs(bin))
}

#[cfg(test)]
#[path = "language_server_spec.tests.rs"]
mod tests;
