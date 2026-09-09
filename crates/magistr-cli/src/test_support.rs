//! Shared test infrastructure for the magistr codebase.
//!
//! Provides reusable fixtures, mock trait implementations, and assertion
//! helpers that reduce boilerplate across unit and integration tests.
//!
//! ## Fixtures
//!
//! - [`TempProject`] — temporary project directory backed by `tempfile::TempDir`
//! - [`MockProjectDir`] — builder for `TempProject` with declarative setup
//!
//! ## Mock trait implementations
//!
//! - [`MockGatePlugin`] — configurable `GatePlugin` that records calls
//! - [`MockOutputRenderer`] — captures all output for assertion
//! - [`MockHeadlessRenderer`] — captures emitted `HeadlessEvent`s
//!
//! ## Assertion helpers
//!
//! - [`assert_file_exists`] — assert a file exists relative to a directory
//! - [`assert_file_contains`] — assert file content contains a substring

use std::path::{Path, PathBuf};

// ── TempProject fixture ─────────────────────────────────────────────────────

/// A temporary project directory for test isolation.
///
/// Wraps `tempfile::TempDir` and provides convenience accessors for paths.
/// The directory is automatically deleted when the `TempProject` is dropped.
pub struct TempProject {
    dir: tempfile::TempDir,
}

impl Default for TempProject {
    fn default() -> Self {
        Self::new()
    }
}

impl TempProject {
    /// Create a bare temporary project directory (no `.magistr/` or other files).
    pub fn new() -> Self {
        Self {
            dir: tempfile::TempDir::new().expect("failed to create temp dir"),
        }
    }

    /// Create a temporary project with a `.magistr/` subdirectory.
    pub fn with_magistr_dir() -> Self {
        let project = Self::new();
        std::fs::create_dir_all(project.path().join(".magistr"))
            .expect("failed to create .magistr dir");
        project
    }

    /// The project root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The project root as a `&str` (for APIs like `App::new` that take `&str`).
    #[must_use]
    pub fn path_str(&self) -> &str {
        self.dir
            .path()
            .to_str()
            .expect("temp dir path is not UTF-8")
    }

    /// Write a file at a relative path within the project.
    ///
    /// Creates parent directories as needed.
    pub fn write_file(&self, relative_path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
        let full = self.dir.path().join(relative_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&full, content).expect("failed to write file");
    }

    /// Read a file at a relative path within the project.
    #[must_use]
    pub fn read_file(&self, relative_path: impl AsRef<Path>) -> String {
        let full = self.dir.path().join(relative_path);
        std::fs::read_to_string(&full).expect("failed to read file")
    }

    /// Check if a file exists at a relative path within the project.
    #[must_use]
    pub fn file_exists(&self, relative_path: impl AsRef<Path>) -> bool {
        self.dir.path().join(relative_path).exists()
    }
}

// ── MockProjectDir builder ──────────────────────────────────────────────────

/// Builder for [`TempProject`] with declarative project structure setup.
///
/// # Example
///
/// ```ignore
/// let project = MockProjectDir::new()
///     .with_magistr_dir()
///     .with_config(serde_json::json!({"language": "rust"}))
///     .with_file("src/main.rs", "fn main() {}")
///     .build();
/// ```
pub struct MockProjectDir {
    create_magistr: bool,
    config: Option<serde_json::Value>,
    files: Vec<(PathBuf, Vec<u8>)>,
}

impl Default for MockProjectDir {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProjectDir {
    /// Start building a new mock project directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            create_magistr: false,
            config: None,
            files: Vec::new(),
        }
    }

    /// Create a `.magistr/` subdirectory.
    #[must_use]
    pub fn with_magistr_dir(mut self) -> Self {
        self.create_magistr = true;
        self
    }

    /// Write a `.magistr/project.json` with the given JSON value.
    ///
    /// Implies `with_magistr_dir()`.
    #[must_use]
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.create_magistr = true;
        self.config = Some(config);
        self
    }

    /// Add an arbitrary file at a relative path.
    #[must_use]
    pub fn with_file(mut self, path: impl Into<PathBuf>, content: impl Into<Vec<u8>>) -> Self {
        self.files.push((path.into(), content.into()));
        self
    }

    /// Build the [`TempProject`], creating all configured directories and files.
    pub fn build(self) -> TempProject {
        let project = TempProject::new();

        if self.create_magistr {
            std::fs::create_dir_all(project.path().join(".magistr"))
                .expect("failed to create .magistr dir");
        }

        if let Some(config) = &self.config {
            let config_str =
                serde_json::to_string_pretty(config).expect("failed to serialize config");
            project.write_file(".magistr/project.json", config_str);
        }

        for (path, content) in &self.files {
            project.write_file(path, content);
        }

        project
    }
}

// ── Mock GatePlugin ─────────────────────────────────────────────────────────

use crate::plugin::gate_trait::{GateConfig, GateFinding, GateOutcome, GatePlugin};
use crate::plugin::types::PluginError;

/// Mock implementation of [`GatePlugin`] for testing.
///
/// Configurable to return pass/fail outcomes and records all gate runs
/// for later assertion.
pub struct MockGatePlugin {
    plugin_name: String,
    gates: Vec<String>,
    /// Pre-configured outcome to return on each `run_gate` call.
    pub outcome: GateOutcome,
    /// Recorded gate runs (gate name, project dir).
    pub calls: Vec<(String, String)>,
}

impl MockGatePlugin {
    /// Create a mock gate plugin that always passes.
    #[must_use]
    pub fn passing(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            plugin_name: name.clone(),
            gates: vec![name.clone()],
            outcome: GateOutcome {
                passed: true,
                gate_name: name,
                findings: Vec::new(),
                summary: "All checks passed".to_string(),
                duration: std::time::Duration::from_millis(10),
            },
            calls: Vec::new(),
        }
    }

    /// Create a mock gate plugin that always fails with the given findings.
    #[must_use]
    pub fn failing(name: impl Into<String>, findings: Vec<GateFinding>) -> Self {
        let name = name.into();
        let summary = format!("{} finding(s)", findings.len());
        Self {
            plugin_name: name.clone(),
            gates: vec![name.clone()],
            outcome: GateOutcome {
                passed: false,
                gate_name: name,
                findings,
                summary,
                duration: std::time::Duration::from_millis(10),
            },
            calls: Vec::new(),
        }
    }

    /// Set which gate names this plugin supports.
    #[must_use]
    pub fn with_gates(mut self, gates: Vec<String>) -> Self {
        self.gates = gates;
        self
    }
}

impl GatePlugin for MockGatePlugin {
    fn run_gate(
        &mut self,
        config: GateConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GateOutcome, PluginError>> + Send + '_>,
    > {
        self.calls
            .push((config.gate_name.clone(), config.project_dir.clone()));
        let mut outcome = self.outcome.clone();
        outcome.gate_name = config.gate_name;
        Box::pin(async move { Ok(outcome) })
    }

    fn name(&self) -> &str {
        &self.plugin_name
    }

    fn supported_gates(&self) -> &[String] {
        &self.gates
    }
}

// ── Mock OutputRenderer ─────────────────────────────────────────────────────

use crate::output::renderer::{DataRenderer, OutputRenderer, TextRenderer};

/// Mock implementation of [`OutputRenderer`] that captures all output.
///
/// Each method appends to the `output` vec with a tagged prefix so tests
/// can assert on both the method called and the content rendered.
#[derive(Debug, Default)]
pub struct MockOutputRenderer {
    /// Captured output lines, each prefixed with the method name.
    pub output: std::cell::RefCell<Vec<String>>,
}

impl MockOutputRenderer {
    /// Create a new empty mock renderer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all captured output lines.
    pub fn lines(&self) -> Vec<String> {
        self.output.borrow().clone()
    }

    /// Check if any captured line contains the given substring.
    #[must_use]
    pub fn contains(&self, needle: &str) -> bool {
        self.output.borrow().iter().any(|l| l.contains(needle))
    }
}

impl TextRenderer for MockOutputRenderer {
    fn success(&self, _w: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        self.output.borrow_mut().push(format!("success: {msg}"));
        Ok(())
    }

    fn error(&self, _w: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        self.output.borrow_mut().push(format!("error: {msg}"));
        Ok(())
    }

    fn warning(&self, _w: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        self.output.borrow_mut().push(format!("warning: {msg}"));
        Ok(())
    }

    fn info(&self, _w: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        self.output.borrow_mut().push(format!("info: {msg}"));
        Ok(())
    }
}

impl DataRenderer for MockOutputRenderer {
    fn table(
        &self,
        _w: &mut dyn std::io::Write,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> std::io::Result<()> {
        self.output
            .borrow_mut()
            .push(format!("table: {}x{}", headers.len(), rows.len()));
        Ok(())
    }

    fn json(&self, _w: &mut dyn std::io::Write, data: &serde_json::Value) -> std::io::Result<()> {
        self.output.borrow_mut().push(format!("json: {data}"));
        Ok(())
    }

    fn kv(&self, _w: &mut dyn std::io::Write, pairs: &[(String, String)]) -> std::io::Result<()> {
        self.output
            .borrow_mut()
            .push(format!("kv: {} pairs", pairs.len()));
        Ok(())
    }

    fn list(&self, _w: &mut dyn std::io::Write, items: &[String]) -> std::io::Result<()> {
        self.output
            .borrow_mut()
            .push(format!("list: {} items", items.len()));
        Ok(())
    }
}

impl OutputRenderer for MockOutputRenderer {}

// ── Mock HeadlessRenderer ───────────────────────────────────────────────────

use crate::headless::event::HeadlessEvent;
use crate::headless::renderer::HeadlessRenderer;

/// Mock implementation of [`HeadlessRenderer`] that captures emitted events.
#[derive(Debug, Default)]
pub struct MockHeadlessRenderer {
    /// Captured events in emission order.
    pub events: std::cell::RefCell<Vec<HeadlessEvent>>,
}

impl MockHeadlessRenderer {
    /// Create a new empty mock renderer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of captured events.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.borrow().len()
    }
}

impl HeadlessRenderer for MockHeadlessRenderer {
    fn emit(&self, _w: &mut dyn std::io::Write, event: HeadlessEvent) -> std::io::Result<()> {
        self.events.borrow_mut().push(event);
        Ok(())
    }
}

// ── Assertion helpers ───────────────────────────────────────────────────────

/// Assert that a file exists at `relative_path` within `base_dir`.
///
/// # Panics
///
/// Panics with a descriptive message if the file does not exist.
pub fn assert_file_exists(base_dir: &Path, relative_path: impl AsRef<Path>) {
    let full = base_dir.join(relative_path.as_ref());
    assert!(full.exists(), "expected file to exist: {}", full.display());
}

/// Assert that a file at `relative_path` within `base_dir` contains `needle`.
///
/// # Panics
///
/// Panics if the file does not exist or does not contain `needle`.
pub fn assert_file_contains(base_dir: &Path, relative_path: impl AsRef<Path>, needle: &str) {
    let full = base_dir.join(relative_path.as_ref());
    let content = std::fs::read_to_string(&full).unwrap_or_else(|e| {
        panic!("failed to read {}: {e}", full.display());
    });
    assert!(
        content.contains(needle),
        "expected {} to contain {needle:?}, but content was:\n{content}",
        full.display()
    );
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "test_support.tests.rs"]
mod tests;
