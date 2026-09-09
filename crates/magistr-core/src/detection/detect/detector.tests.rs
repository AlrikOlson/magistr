use super::*;

use std::path::Path;
use std::sync::Mutex;

use crate::error::Result;
use crate::types::{Framework, Language, Stack};

/// Mock detector that returns a preconfigured stack.
pub struct MockDetector {
    stack: Mutex<Option<Stack>>,
}

impl MockDetector {
    /// Create a mock that returns the given stack.
    pub fn with_stack(stack: Stack) -> Self {
        Self {
            stack: Mutex::new(Some(stack)),
        }
    }

    /// Create a mock that returns a default Rust stack.
    pub fn rust_project() -> Self {
        Self::with_stack(Stack {
            project_name: "mock-project".to_string(),
            language: Language::Rust,
            framework: Framework::None,
            ..Stack::default()
        })
    }
}

impl Detector for MockDetector {
    fn detect_stack(&self, _dir: &Path) -> Result<Stack> {
        let stack = self
            .stack
            .lock()
            .expect("MockDetector lock poisoned")
            .clone()
            .unwrap_or_default();
        Ok(stack)
    }
}

#[test]
fn mock_detector_returns_configured_stack() {
    let mock = MockDetector::rust_project();
    let stack = mock.detect_stack(Path::new("/fake")).unwrap();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.project_name, "mock-project");
}

#[test]
fn mock_detector_returns_custom_stack() {
    let stack = Stack {
        project_name: "custom".to_string(),
        language: Language::Python,
        framework: Framework::Django,
        ..Stack::default()
    };
    let mock = MockDetector::with_stack(stack);
    let result = mock.detect_stack(Path::new("/any")).unwrap();
    assert_eq!(result.language, Language::Python);
    assert_eq!(result.framework, Framework::Django);
}
