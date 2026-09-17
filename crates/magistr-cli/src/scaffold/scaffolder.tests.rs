use super::*;
use crate::scaffold::{AdminOperation, Ownership};

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::Result;
use crate::scaffold::{ChangePlan, FileAction, PlannedChange};

/// Mock scaffolder that returns preconfigured results without filesystem access.
pub struct MockScaffolder {
    generated_files: Mutex<Vec<String>>,
    preview_result: Mutex<Option<ChangePlan>>,
}

impl MockScaffolder {
    /// Create a mock that returns the given file list from `generate()`.
    pub fn with_files(files: Vec<String>) -> Self {
        Self {
            generated_files: Mutex::new(files),
            preview_result: Mutex::new(None),
        }
    }

    /// Create a mock that returns an empty result.
    pub fn empty() -> Self {
        Self::with_files(Vec::new())
    }

    /// Set the preview result.
    pub fn with_preview(mut self, preview: ChangePlan) -> Self {
        self.preview_result = Mutex::new(Some(preview));
        self
    }
}

impl Scaffolder for MockScaffolder {
    fn generate(
        &self,
        _dir: &Path,
        _template_vars: &HashMap<String, String>,
        _generated_overrides: &HashMap<String, String>,
    ) -> Result<Vec<String>> {
        let files = self
            .generated_files
            .lock()
            .expect("MockScaffolder lock poisoned")
            .clone();
        Ok(files)
    }

    fn preview(
        &self,
        _dir: &Path,
        _template_vars: &HashMap<String, String>,
        _generated_overrides: &HashMap<String, String>,
    ) -> Result<ChangePlan> {
        let preview = self
            .preview_result
            .lock()
            .expect("MockScaffolder lock poisoned")
            .clone()
            .unwrap_or_else(|| ChangePlan::new(AdminOperation::Setup));
        Ok(preview)
    }
}

#[test]
fn mock_scaffolder_returns_configured_files() {
    let mock = MockScaffolder::with_files(vec![
        "CLAUDE.md".to_string(),
        ".claude/rules/workflow.md".to_string(),
    ]);
    let result = mock
        .generate(Path::new("/fake"), &HashMap::new(), &HashMap::new())
        .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"CLAUDE.md".to_string()));
}

#[test]
fn mock_scaffolder_returns_empty_preview() {
    let mock = MockScaffolder::empty();
    let preview = mock
        .preview(Path::new("/fake"), &HashMap::new(), &HashMap::new())
        .unwrap();
    assert_eq!(preview.files.len(), 0);
}

#[test]
fn mock_scaffolder_returns_custom_preview() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![PlannedChange {
            ownership: Ownership::Unclaimed,
            ownership_context: None,
            path: "CLAUDE.md".to_string(),
            action: FileAction::Create,
            new_content: "# Test".to_string(),
            existing_content: None,
            diff: None,
        }],
    };
    let mock = MockScaffolder::empty().with_preview(preview);
    let result = mock
        .preview(Path::new("/fake"), &HashMap::new(), &HashMap::new())
        .unwrap();
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].action, FileAction::Create);
}

#[test]
fn mock_scaffolder_usable_as_trait_object() {
    let scaffolder: Box<dyn Scaffolder> = Box::new(MockScaffolder::empty());
    let result = scaffolder
        .generate(Path::new("/fake"), &HashMap::new(), &HashMap::new())
        .unwrap();
    assert!(result.is_empty());
}
