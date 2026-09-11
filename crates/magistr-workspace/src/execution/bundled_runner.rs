use super::{
    RunOutput, RunReport, runner_bun, runner_cargo_test, runner_go, runner_jest, runner_nextest,
    runner_pytest, runner_vitest,
};

/// The bundled first-party runner adapters on the retained `can-parse` /
/// `parse-output` vocabulary, consulted in [`BundledRunner::TRUST_ORDER`]; the
/// first claimant parses. `None` from a claimant means it could not extract
/// structure and the host falls back to the honest head+tail cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledRunner {
    Bun,
    Vitest,
    Jest,
    Pytest,
    Nextest,
    Cargo,
    Go,
}

impl BundledRunner {
    pub const TRUST_ORDER: [Self; 7] = [
        Self::Bun,
        Self::Vitest,
        Self::Jest,
        Self::Pytest,
        Self::Nextest,
        Self::Cargo,
        Self::Go,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bun => "bun",
            Self::Vitest => "vitest",
            Self::Jest => "jest",
            Self::Pytest => "pytest",
            Self::Nextest => "nextest",
            Self::Cargo => "cargo",
            Self::Go => "go",
        }
    }

    #[must_use]
    pub fn can_parse(self, command_line: &str) -> bool {
        match self {
            Self::Bun => runner_bun::can_parse(command_line),
            Self::Vitest => runner_vitest::can_parse(command_line),
            Self::Jest => runner_jest::can_parse(command_line),
            Self::Pytest => runner_pytest::can_parse(command_line),
            Self::Nextest => runner_nextest::can_parse(command_line),
            Self::Cargo => runner_cargo_test::can_parse(command_line),
            Self::Go => runner_go::can_parse(command_line),
        }
    }

    #[must_use]
    pub fn parse_output(self, output: &RunOutput) -> Option<RunReport> {
        match self {
            Self::Bun => runner_bun::parse(output),
            Self::Vitest => runner_vitest::parse(output),
            Self::Jest => runner_jest::parse(output),
            Self::Pytest => runner_pytest::parse(output),
            Self::Nextest => runner_nextest::parse(output),
            Self::Cargo => runner_cargo_test::parse(output),
            Self::Go => runner_go::parse(output),
        }
    }
}

#[cfg(test)]
#[path = "bundled_runner.tests.rs"]
mod tests;
