use crate::detect::detection::Detection;
use serde::{Deserialize, Serialize};

use super::{
    CiProvider, Formatter, Framework, Language, Linter, PackageManager, TestFramework, Workspace,
};

/// A detection contributed by a plugin (category -> detected value).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDetection {
    /// Detection category (e.g., "framework", "tool").
    pub category: String,
    /// What was detected (e.g., "internal-framework-x").
    pub name: String,
    /// Confidence score (0-100).
    pub confidence: u8,
    /// Which plugin contributed this detection.
    pub plugin_name: String,
}

/// Detected project stack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stack {
    pub project_name: String,
    pub language: Language,
    pub package_manager: PackageManager,
    pub framework: Framework,
    pub test_framework: TestFramework,
    pub linter: Linter,
    pub formatter: Formatter,
    #[serde(default = "default_workspace")]
    pub workspace: Workspace,
    /// Detected CI/CD provider.
    #[serde(default = "default_ci_provider")]
    pub ci_provider: CiProvider,
    /// Extra detections contributed by installed plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_detections: Vec<PluginDetection>,
}

fn default_workspace() -> Workspace {
    Workspace::None
}

fn default_ci_provider() -> CiProvider {
    CiProvider::None
}

impl Default for Stack {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            language: Language::Unknown,
            package_manager: PackageManager::Unknown,
            framework: Framework::None,
            test_framework: TestFramework::Unknown,
            linter: Linter::None,
            formatter: Formatter::None,
            workspace: Workspace::None,
            ci_provider: CiProvider::None,
            extra_detections: Vec::new(),
        }
    }
}

/// Full stack detection with per-category confidence scores.
///
/// Used by `magistr detect --smart` to show
/// detection confidence and flag low-confidence results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartStack {
    pub project_name: String,
    pub language: Detection<Language>,
    pub package_manager: Detection<PackageManager>,
    pub framework: Detection<Framework>,
    pub test_framework: Detection<TestFramework>,
    pub linter: Detection<Linter>,
    pub formatter: Detection<Formatter>,
    pub workspace: Detection<Workspace>,
    /// Detected CI/CD provider with confidence.
    pub ci_provider: Detection<CiProvider>,
    /// Additional framework detail (e.g., "App Router", "Pages Router", "REST Framework").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework_detail: Option<String>,
}

impl SmartStack {
    /// Convert to a plain `Stack`, discarding confidence metadata.
    pub fn to_stack(&self) -> Stack {
        Stack {
            project_name: self.project_name.clone(),
            language: self.language.value,
            package_manager: self.package_manager.value,
            framework: self.framework.value,
            test_framework: self.test_framework.value,
            linter: self.linter.value,
            formatter: self.formatter.value,
            workspace: self.workspace.value,
            ci_provider: self.ci_provider.value,
            extra_detections: Vec::new(),
        }
    }

    /// Return category names where confidence is low and manual override is recommended.
    pub fn low_confidence_categories(&self) -> Vec<&'static str> {
        let mut result = Vec::new();
        if self.language.needs_override() {
            result.push("language");
        }
        if self.package_manager.needs_override() {
            result.push("package_manager");
        }
        if self.framework.needs_override() {
            result.push("framework");
        }
        if self.test_framework.needs_override() {
            result.push("test_framework");
        }
        if self.linter.needs_override() {
            result.push("linter");
        }
        if self.formatter.needs_override() {
            result.push("formatter");
        }
        if self.workspace.needs_override() {
            result.push("workspace");
        }
        if self.ci_provider.needs_override() {
            result.push("ci_provider");
        }
        result
    }
}
