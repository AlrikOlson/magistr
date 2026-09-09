use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TestFramework {
    CargoTest,
    Vitest,
    Jest,
    Mocha,
    Pytest,
    GoTest,
    Junit,
    Rspec,
    Minitest,
    Exunit,
    Xctest,
    Ctest,
    Xunit,
    Unknown,
}

impl TestFramework {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CargoTest => "cargo-test",
            Self::Vitest => "vitest",
            Self::Jest => "jest",
            Self::Mocha => "mocha",
            Self::Pytest => "pytest",
            Self::GoTest => "go-test",
            Self::Junit => "junit",
            Self::Rspec => "rspec",
            Self::Minitest => "minitest",
            Self::Exunit => "exunit",
            Self::Xctest => "xctest",
            Self::Ctest => "ctest",
            Self::Xunit => "xunit",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for TestFramework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
