use std::io::Read;
use std::path::{Component, Path, PathBuf};

use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::formatter_options::FormatterOptions;

/// Subprocess formatter router with bounded I/O and workspace-aware Rust settings.
#[derive(Debug)]
pub struct NativeFormatter {
    root: PathBuf,
    options: FormatterOptions,
    tasks: TaskTracker,
}

impl NativeFormatter {
    pub fn new(root: PathBuf, options: FormatterOptions) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("resolve formatter workspace: {error}"))?;
        if !root.is_dir() {
            return Err("formatter workspace must be a directory".into());
        }
        if options.timeout.is_zero() || options.max_output_bytes == 0 {
            return Err("formatter timeout and byte limit must be positive".into());
        }
        Ok(Self {
            root,
            options,
            tasks: TaskTracker::new(),
        })
    }

    /// Format candidate bytes without supplying a writable filename to the process.
    /// Unsupported extensions pass through without resolving or starting a process.
    pub async fn format(&self, path: &str, source: &str) -> Result<String, String> {
        let relative = Path::new(path);
        if path.is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("formatter path must be a normalized relative file path".into());
        }
        // Empty documents have no layout to normalize. Preserve creation and
        // empty-anchor semantics regardless of which formatter is installed.
        if source.is_empty() {
            return Ok(String::new());
        }
        let extension = relative
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let is_rust = extension == "rs";
        let is_biome = matches!(
            extension.as_str(),
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" | "json" | "jsonc"
        );
        if !is_rust && !is_biome {
            return Ok(source.to_owned());
        }
        if source.len() > self.options.max_output_bytes {
            return Err(format!(
                "formatter input limit exceeded ({} bytes)",
                self.options.max_output_bytes
            ));
        }
        let target = self.root.join(relative);
        let parent = target.parent().ok_or("formatter path has no parent")?;
        let cwd = existing_parent(&self.root, parent)?;
        let (binary, arguments, cwd) = if is_rust {
            let binary = match &self.options.rustfmt {
                Some(binary) => binary.clone(),
                None => self.resolve_rustfmt().await?,
            };
            (
                binary,
                vec![
                    "--emit".to_owned(),
                    "stdout".to_owned(),
                    "--edition".to_owned(),
                    edition_for(&self.root, parent)?,
                ],
                cwd,
            )
        } else {
            (
                self.options
                    .biome
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("biome")),
                vec!["format".to_owned(), format!("--stdin-file-path={path}")],
                self.root.clone(),
            )
        };
        self.run_process(&binary, &arguments, &cwd, source).await
    }

    /// Temporary transaction formatters share the admitted service's tracker.
    pub(super) fn with_tasks(mut self, tasks: TaskTracker) -> Self {
        self.tasks = tasks;
        self
    }

    async fn run_process(
        &self,
        binary: &Path,
        arguments: &[String],
        cwd: &Path,
        input: &str,
    ) -> Result<String, String> {
        let cancel = CancellationToken::new();
        // Dropping the request only signals the independently tracked owner.
        // That task retains the child until kill + wait has completed.
        let _cancel_on_drop = cancel.clone().drop_guard();
        self.tasks
            .spawn(super::formatter_process::run(
                binary.to_owned(),
                arguments.to_vec(),
                cwd.to_owned(),
                input.to_owned(),
                self.options.clone(),
                has_toolchain_file(cwd),
                cancel,
            ))
            .await
            .map_err(|error| format!("formatter task failed: {error}"))?
    }

    async fn resolve_rustfmt(&self) -> Result<PathBuf, String> {
        // Resolve from the workspace, so rustup selects its pinned toolchain.
        // A failed pin must never silently fall back to another PATH toolchain.
        let result = self
            .run_process(
                Path::new("rustup"),
                &["which".to_owned(), "rustfmt".to_owned()],
                &self.root,
                "",
            )
            .await;
        if let Ok(output) = &result {
            let binary = PathBuf::from(output.trim());
            if binary.is_absolute() && binary.is_file() {
                return Ok(binary);
            }
        }
        if has_toolchain_file(&self.root) || std::env::var_os("RUSTUP_TOOLCHAIN").is_some() {
            return Err(format!(
                "rustfmt is unavailable for the pinned workspace toolchain: {}",
                result
                    .err()
                    .unwrap_or_else(|| "rustup returned no executable".into())
            ));
        }
        // Running the formatter itself validates the fallback executable; no shell
        // or runtime wrapper is used, and failure remains visible to the pipeline.
        Ok(PathBuf::from("rustfmt"))
    }
}

fn has_toolchain_file(directory: &Path) -> bool {
    directory.ancestors().any(|ancestor| {
        ancestor.join("rust-toolchain.toml").exists() || ancestor.join("rust-toolchain").exists()
    })
}

fn existing_parent(root: &Path, parent: &Path) -> Result<PathBuf, String> {
    for ancestor in parent.ancestors() {
        if !ancestor.starts_with(root) {
            break;
        }
        match ancestor.canonicalize() {
            Ok(canonical) => {
                if !canonical.starts_with(root) || !canonical.is_dir() {
                    return Err(
                        "formatter parent is outside the workspace or is not a directory".into(),
                    );
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("resolve formatter parent: {error}")),
        }
    }
    Err("formatter has no existing workspace parent directory".into())
}

fn edition_for(root: &Path, parent: &Path) -> Result<String, String> {
    for directory in parent.ancestors().take_while(|path| path.starts_with(root)) {
        let Some(manifest) = read_manifest(root, &directory.join("Cargo.toml"))? else {
            continue;
        };
        if let Some(edition) = manifest
            .get("package")
            .and_then(|package| package.get("edition"))
            .and_then(toml::Value::as_str)
        {
            return Ok(normalize_edition(edition).to_owned());
        }
        if let Some(root_manifest) = read_manifest(root, &root.join("Cargo.toml"))?
            && let Some(edition) = root_manifest
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(|package| package.get("edition"))
                .and_then(toml::Value::as_str)
        {
            return Ok(normalize_edition(edition).to_owned());
        }
    }
    // Preserve the reference formatter's default for loose source files.
    Ok("2021".to_owned())
}

fn normalize_edition(edition: &str) -> &str {
    match edition {
        "2015" | "2018" | "2021" | "2024" => edition,
        _ => "2021",
    }
}

fn read_manifest(root: &Path, path: &Path) -> Result<Option<toml::Value>, String> {
    let canonical = match path.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("resolve Cargo manifest: {error}")),
    };
    if !canonical.starts_with(root) {
        return Err("formatter Cargo manifest is outside the workspace".into());
    }
    // Manifests are configuration, not unbounded subprocess input.
    const MANIFEST_LIMIT: u64 = 1024 * 1024;
    let mut text = String::new();
    std::fs::File::open(&canonical)
        .map_err(|error| format!("open Cargo manifest: {error}"))?
        .take(MANIFEST_LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("read Cargo manifest: {error}"))?;
    if text.len() as u64 > MANIFEST_LIMIT {
        return Err("formatter Cargo manifest exceeds size limit".into());
    }
    toml::from_str(&text)
        .map(Some)
        .map_err(|error| format!("parse Cargo manifest: {error}"))
}

#[cfg(test)]
#[path = "formatter.tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "formatter_cancellation.tests.rs"]
mod cancellation_tests;
