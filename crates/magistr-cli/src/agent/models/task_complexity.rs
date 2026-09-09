//! Estimated complexity of a task.

/// Estimated complexity of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskComplexity {
    /// Simple tasks: explain, read, search, fix typos, short prompts.
    Simple,
    /// Moderate tasks: implement features, write tests, medium prompts.
    Moderate,
    /// Complex tasks: refactor, architect, design systems, long prompts.
    Complex,
}

impl TaskComplexity {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Moderate => "moderate",
            Self::Complex => "complex",
        }
    }

    /// Default model tier for this complexity.
    ///
    /// Returns the model ID from built-in defaults. Claude Fable 5 is
    /// intentionally not a tier — it is the priciest model in the lineup and
    /// stays an explicit `--model` choice rather than something routing picks.
    pub fn default_model_id(self) -> &'static str {
        match self {
            Self::Simple => "claude-haiku-4-5",
            Self::Moderate => "claude-sonnet-5",
            Self::Complex => "claude-opus-5",
        }
    }
}

impl std::fmt::Display for TaskComplexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
