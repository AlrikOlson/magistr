//! Estimate task complexity from prompt text and metadata.

use super::complexity_estimate::ComplexityEstimate;
use super::task_complexity::TaskComplexity;

/// Keywords that suggest a simple, low-effort task.
const SIMPLE_KEYWORDS: &[&str] = &[
    "explain",
    "what is",
    "what does",
    "how does",
    "describe",
    "read",
    "search",
    "find",
    "list",
    "show",
    "typo",
    "rename",
    "fix typo",
    "comment",
    "format",
    "lint",
];

/// Keywords that suggest a complex, high-effort task.
const COMPLEX_KEYWORDS: &[&str] = &[
    "refactor",
    "architect",
    "design",
    "redesign",
    "rewrite",
    "migrate",
    "overhaul",
    "from scratch",
    "comprehensive",
    "entire",
    "all files",
    "monorepo",
    "multi-module",
    "cross-cutting",
];

/// Estimate the complexity of a task from the prompt text and metadata.
///
/// Heuristics used (in priority order):
/// 1. **Keyword matching** — simple/complex keywords in the prompt
/// 2. **Prompt length** — short prompts are usually simple, long ones complex
/// 3. **File references** — more files mentioned → higher complexity
///
/// `file_count` is an optional hint for how many files are involved.
pub fn estimate_complexity(prompt: &str, file_count: Option<usize>) -> ComplexityEstimate {
    let prompt_lower = prompt.to_lowercase();
    let word_count = prompt.split_whitespace().count();

    // Score from -1.0 (simple) to +1.0 (complex)
    let mut score: f64 = 0.0;
    let mut reasons: Vec<&str> = Vec::new();

    // 1. Keyword matching
    let simple_matches = SIMPLE_KEYWORDS
        .iter()
        .filter(|kw| prompt_lower.contains(*kw))
        .count();
    let complex_matches = COMPLEX_KEYWORDS
        .iter()
        .filter(|kw| prompt_lower.contains(*kw))
        .count();

    if complex_matches > 0 {
        score += 0.4 * complex_matches as f64;
        reasons.push("complex keywords detected");
    }
    if simple_matches > 0 {
        score -= 0.3 * simple_matches as f64;
        reasons.push("simple keywords detected");
    }

    // 2. Prompt length
    if word_count <= 10 {
        score -= 0.3;
        reasons.push("short prompt");
    } else if word_count >= 50 {
        score += 0.3;
        reasons.push("long prompt");
    } else if word_count >= 100 {
        score += 0.5;
        reasons.push("very long prompt");
    }

    // 3. File count hint
    if let Some(files) = file_count {
        if files <= 2 {
            score -= 0.2;
            reasons.push("few files involved");
        } else if files >= 6 {
            score += 0.4;
            reasons.push("many files involved");
        } else if files >= 3 {
            // 3-5 files: moderate signal
            score += 0.1;
            reasons.push("moderate file count");
        }
    }

    // Map score to complexity
    let complexity = if score <= -0.2 {
        TaskComplexity::Simple
    } else if score >= 0.5 {
        TaskComplexity::Complex
    } else {
        TaskComplexity::Moderate
    };

    // Confidence: higher when score is further from boundaries
    let confidence = match complexity {
        TaskComplexity::Simple => ((-score - 0.2) * 2.0).clamp(0.3, 0.95),
        TaskComplexity::Complex => ((score - 0.5) * 2.0).clamp(0.3, 0.95),
        TaskComplexity::Moderate => {
            // Moderate is the "middle ground" — lower confidence
            let dist_from_boundary = (score + 0.2).min(0.5 - score);
            (dist_from_boundary * 2.0).clamp(0.3, 0.8)
        }
    };

    let reasoning = if reasons.is_empty() {
        "no strong signals".to_string()
    } else {
        reasons.join(", ")
    };

    ComplexityEstimate {
        complexity,
        reasoning,
        confidence,
    }
}
