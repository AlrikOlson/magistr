//! Context-aware wizard intelligence for `magistr init`.
//!
//! Analyzes [`SmartStack`] detection results to:
//! - Determine which wizard questions can be skipped (high confidence → auto-accept)
//! - Detect common project archetypes and offer one-click recommended configs
//! - Produce a [`WizardSummary`] showing what's confirmed vs needs review

use super::confidence::Confidence;
use crate::types::*;

// ---------------------------------------------------------------------------
// Common setup / archetype detection
// ---------------------------------------------------------------------------

/// A recognized common project archetype with recommended configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonSetup {
    /// Short name, e.g. "Next.js App".
    pub name: &'static str,
    /// One-line description, e.g. "Standard Next.js application with App Router".
    pub description: &'static str,
    /// Overall confidence that the project matches this archetype.
    pub confidence: Confidence,
}

/// Detect a common project archetype from smart detection results.
///
/// Returns `Some(CommonSetup)` when the detected stack closely matches a
/// well-known archetype. The caller can offer the user a shortcut:
/// "This looks like a standard {name} — use recommended config? [Y/n]"
pub fn detect_common_setup(smart: &SmartStack) -> Option<CommonSetup> {
    // Try each archetype matcher in priority order (most specific first).
    None.or_else(|| match_nextjs_app(smart))
        .or_else(|| match_django_api(smart))
        .or_else(|| match_fastapi_service(smart))
        .or_else(|| match_rust_cli(smart))
        .or_else(|| match_rust_web(smart))
        .or_else(|| match_go_service(smart))
        .or_else(|| match_rails_app(smart))
        .or_else(|| match_spring_boot(smart))
        .or_else(|| match_express_api(smart))
}

fn match_nextjs_app(s: &SmartStack) -> Option<CommonSetup> {
    if s.framework.value != Framework::Next {
        return None;
    }
    let detail = s.framework_detail.as_deref().unwrap_or("App Router");
    let (name, desc) = match detail {
        d if d.contains("Pages") => (
            "Next.js App (Pages Router)",
            "Standard Next.js application with Pages Router, TypeScript, and ESLint",
        ),
        d if d.contains("Hybrid") => (
            "Next.js App (Hybrid)",
            "Next.js application using both App Router and Pages Router",
        ),
        _ => (
            "Next.js App",
            "Standard Next.js application with App Router, TypeScript, and ESLint",
        ),
    };
    Some(CommonSetup {
        name,
        description: desc,
        confidence: s.framework.confidence,
    })
}

fn match_django_api(s: &SmartStack) -> Option<CommonSetup> {
    if s.framework.value != Framework::Django {
        return None;
    }
    let has_rest = s
        .framework_detail
        .as_deref()
        .is_some_and(|d| d.contains("REST"));
    let (name, desc) = if has_rest {
        (
            "Django REST API",
            "Django application with REST Framework, pytest, and ruff/black",
        )
    } else {
        (
            "Django App",
            "Standard Django application with pytest and ruff/black",
        )
    };
    Some(CommonSetup {
        name,
        description: desc,
        confidence: s.framework.confidence,
    })
}

fn match_fastapi_service(s: &SmartStack) -> Option<CommonSetup> {
    if s.framework.value != Framework::FastApi {
        return None;
    }
    Some(CommonSetup {
        name: "FastAPI Service",
        description: "FastAPI application with pytest and ruff",
        confidence: s.framework.confidence,
    })
}

fn match_rust_cli(s: &SmartStack) -> Option<CommonSetup> {
    if s.language.value != Language::Rust || s.framework.value != Framework::None {
        return None;
    }
    Some(CommonSetup {
        name: "Rust CLI",
        description: "Rust command-line application with cargo test, clippy, and rustfmt",
        confidence: s.language.confidence,
    })
}

fn match_rust_web(s: &SmartStack) -> Option<CommonSetup> {
    if s.language.value != Language::Rust {
        return None;
    }
    match s.framework.value {
        Framework::Axum => Some(CommonSetup {
            name: "Rust Web (Axum)",
            description: "Axum web service with cargo test, clippy, and rustfmt",
            confidence: s.framework.confidence,
        }),
        Framework::Actix => Some(CommonSetup {
            name: "Rust Web (Actix)",
            description: "Actix-web service with cargo test, clippy, and rustfmt",
            confidence: s.framework.confidence,
        }),
        Framework::Rocket => Some(CommonSetup {
            name: "Rust Web (Rocket)",
            description: "Rocket web service with cargo test, clippy, and rustfmt",
            confidence: s.framework.confidence,
        }),
        _ => None,
    }
}

fn match_go_service(s: &SmartStack) -> Option<CommonSetup> {
    if s.language.value != Language::Go {
        return None;
    }
    let (name, description) = match s.framework.value {
        Framework::Gin => (
            "Go Web (Gin)",
            "Gin web service with go test and golangci-lint",
        ),
        Framework::Echo => (
            "Go Web (Echo)",
            "Echo web service with go test and golangci-lint",
        ),
        Framework::Fiber => (
            "Go Web (Fiber)",
            "Fiber web service with go test and golangci-lint",
        ),
        Framework::Chi => (
            "Go Web (Chi)",
            "Chi web service with go test and golangci-lint",
        ),
        Framework::None => {
            return Some(CommonSetup {
                name: "Go Service",
                description: "Go application with go test, golangci-lint, and gofmt",
                confidence: s.language.confidence,
            });
        }
        _ => return None,
    };
    Some(CommonSetup {
        name,
        description,
        confidence: s.framework.confidence,
    })
}

fn match_rails_app(s: &SmartStack) -> Option<CommonSetup> {
    if s.framework.value != Framework::Rails {
        return None;
    }
    Some(CommonSetup {
        name: "Rails App",
        description: "Ruby on Rails application with RSpec and RuboCop",
        confidence: s.framework.confidence,
    })
}

fn match_spring_boot(s: &SmartStack) -> Option<CommonSetup> {
    if s.framework.value != Framework::Spring {
        return None;
    }
    let lang_label = match s.language.value {
        Language::Kotlin => "Kotlin",
        _ => "Java",
    };
    Some(CommonSetup {
        name: if lang_label == "Kotlin" {
            "Spring Boot (Kotlin)"
        } else {
            "Spring Boot"
        },
        description: if lang_label == "Kotlin" {
            "Spring Boot application with Kotlin, JUnit, and ktlint"
        } else {
            "Spring Boot application with Java and JUnit"
        },
        confidence: s.framework.confidence,
    })
}

fn match_express_api(s: &SmartStack) -> Option<CommonSetup> {
    match s.framework.value {
        Framework::Express => Some(CommonSetup {
            name: "Express API",
            description: "Express.js API with TypeScript, Jest/Vitest, and ESLint",
            confidence: s.framework.confidence,
        }),
        Framework::Fastify => Some(CommonSetup {
            name: "Fastify API",
            description: "Fastify API with TypeScript, Jest/Vitest, and ESLint",
            confidence: s.framework.confidence,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Skippable categories
// ---------------------------------------------------------------------------

/// Return the set of detection categories where confidence is high enough
/// to skip the wizard question (confidence ≥ 90).
///
/// Categories with lower confidence should still be presented to the user
/// for confirmation or manual override.
pub fn skippable_categories(smart: &SmartStack) -> Vec<&'static str> {
    let mut result = Vec::new();
    if smart.language.confidence.is_high() {
        result.push("language");
    }
    if smart.package_manager.confidence.is_high() {
        result.push("package_manager");
    }
    if smart.framework.confidence.is_high() {
        result.push("framework");
    }
    if smart.test_framework.confidence.is_high() {
        result.push("test_framework");
    }
    if smart.linter.confidence.is_high() {
        result.push("linter");
    }
    if smart.formatter.confidence.is_high() {
        result.push("formatter");
    }
    if smart.workspace.confidence.is_high() {
        result.push("workspace");
    }
    if smart.ci_provider.confidence.is_high() {
        result.push("ci_provider");
    }
    result
}

// ---------------------------------------------------------------------------
// Wizard summary
// ---------------------------------------------------------------------------

/// Summary of detection results for wizard display.
#[derive(Debug, Clone)]
pub struct WizardSummary {
    /// Categories auto-accepted (high confidence, no question needed).
    pub confirmed: Vec<ConfirmedCategory>,
    /// Categories needing user review (low/medium confidence).
    pub needs_review: Vec<ReviewCategory>,
    /// Detected common setup archetype, if any.
    pub common_setup: Option<CommonSetup>,
}

/// A category that was auto-accepted by the wizard.
#[derive(Debug, Clone)]
pub struct ConfirmedCategory {
    pub category: &'static str,
    pub value: String,
    pub confidence: Confidence,
}

/// A category that needs user review / confirmation.
#[derive(Debug, Clone)]
pub struct ReviewCategory {
    pub category: &'static str,
    pub value: String,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

/// Build a wizard summary from smart detection results.
///
/// Splits categories into "confirmed" (high confidence → skip) and
/// "needs review" (lower confidence → ask the user). Also includes
/// any detected common setup archetype.
pub fn wizard_summary(smart: &SmartStack) -> WizardSummary {
    let mut confirmed = Vec::new();
    let mut needs_review = Vec::new();

    let categories: Vec<(&str, String, &Confidence, &[String])> = vec![
        (
            "language",
            smart.language.value.to_string(),
            &smart.language.confidence,
            &smart.language.evidence,
        ),
        (
            "package_manager",
            smart.package_manager.value.to_string(),
            &smart.package_manager.confidence,
            &smart.package_manager.evidence,
        ),
        (
            "framework",
            format_framework_value(smart),
            &smart.framework.confidence,
            &smart.framework.evidence,
        ),
        (
            "test_framework",
            smart.test_framework.value.to_string(),
            &smart.test_framework.confidence,
            &smart.test_framework.evidence,
        ),
        (
            "linter",
            smart.linter.value.to_string(),
            &smart.linter.confidence,
            &smart.linter.evidence,
        ),
        (
            "formatter",
            smart.formatter.value.to_string(),
            &smart.formatter.confidence,
            &smart.formatter.evidence,
        ),
        (
            "workspace",
            smart.workspace.value.to_string(),
            &smart.workspace.confidence,
            &smart.workspace.evidence,
        ),
        (
            "ci_provider",
            smart.ci_provider.value.to_string(),
            &smart.ci_provider.confidence,
            &smart.ci_provider.evidence,
        ),
    ];

    for (cat, value, confidence, evidence) in categories {
        if confidence.is_high() {
            confirmed.push(ConfirmedCategory {
                category: cat,
                value,
                confidence: *confidence,
            });
        } else {
            needs_review.push(ReviewCategory {
                category: cat,
                value,
                confidence: *confidence,
                evidence: evidence.to_vec(),
            });
        }
    }

    let common_setup = detect_common_setup(smart);

    WizardSummary {
        confirmed,
        needs_review,
        common_setup,
    }
}

/// Format the framework value, appending detail if present.
fn format_framework_value(smart: &SmartStack) -> String {
    match &smart.framework_detail {
        Some(detail) => format!("{} ({})", smart.framework.value, detail),
        None => smart.framework.value.to_string(),
    }
}

#[cfg(test)]
#[path = "wizard.tests.rs"]
mod tests;
