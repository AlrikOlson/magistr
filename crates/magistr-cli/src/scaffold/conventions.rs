use std::collections::HashMap;

/// Resolved conventions: base (static preset) + AI-discovered project-specific content.
///
/// Single source of truth for merging language conventions with AI recommendations.
/// All emitters and scaffold files should use `resolve_conventions()` instead of
/// calling `conventions_content()` directly.
pub(crate) struct ResolvedConventions<'a> {
    /// Static language-specific conventions (from preset templates).
    pub base: &'a str,
    /// AI-discovered project-specific conventions (may be empty).
    pub ai_conventions: &'a str,
    /// AI-discovered tips for agent configuration (may be empty).
    pub ai_tips: &'a str,
}

impl ResolvedConventions<'_> {
    /// Full conventions document: base + AI conventions + AI tips.
    ///
    /// When AI conventions are present, they're appended as a
    /// "Project-Specific Conventions" section after the base content.
    pub fn full_document(&self) -> String {
        let mut doc = self.base.to_string();

        if !self.ai_conventions.is_empty() {
            doc.push_str("\n\n## Project-Specific Conventions\n\n");
            doc.push_str(self.ai_conventions);
        }

        if !self.ai_tips.is_empty() {
            doc.push_str("\n\n## AI Agent Tips\n\n");
            doc.push_str(self.ai_tips);
        }

        doc
    }
}

/// Resolve conventions from template vars.
///
/// Reads `language`, `ai_conventions`, and `ai_tips` from the vars map
/// and returns a `ResolvedConventions` that merges them into a single document.
pub(crate) fn resolve_conventions(vars: &HashMap<String, String>) -> ResolvedConventions<'_> {
    let lang = vars
        .get("language")
        .map(|s| s.as_str())
        .unwrap_or("unknown");
    let ai_conventions = vars.get("ai_conventions").map(|s| s.as_str()).unwrap_or("");
    let ai_tips = vars.get("ai_tips").map(|s| s.as_str()).unwrap_or("");

    ResolvedConventions {
        base: conventions_content(lang),
        ai_conventions,
        ai_tips,
    }
}

/// Select the language-specific conventions content (static preset).
fn conventions_content(language: &str) -> &'static str {
    match language {
        "rust" => include_str!("../../templates/rules/conventions/rust.md"),
        "typescript" | "javascript" => {
            include_str!("../../templates/rules/conventions/typescript.md")
        }
        "python" => include_str!("../../templates/rules/conventions/python.md"),
        "go" => include_str!("../../templates/rules/conventions/go.md"),
        "java" => include_str!("../../templates/rules/conventions/java.md"),
        "ruby" => include_str!("../../templates/rules/conventions/ruby.md"),
        "elixir" => include_str!("../../templates/rules/conventions/elixir.md"),
        "kotlin" => include_str!("../../templates/rules/conventions/kotlin.md"),
        "swift" => include_str!("../../templates/rules/conventions/swift.md"),
        "cpp" | "c++" => include_str!("../../templates/rules/conventions/cpp.md"),
        "csharp" | "c#" => include_str!("../../templates/rules/conventions/csharp.md"),
        "dart" => include_str!("../../templates/rules/conventions/dart.md"),
        "php" => include_str!("../../templates/rules/conventions/php.md"),
        "scala" => include_str!("../../templates/rules/conventions/scala.md"),
        _ => include_str!("../../templates/rules/conventions/default.md"),
    }
}

#[cfg(test)]
#[path = "conventions.tests.rs"]
mod tests;
