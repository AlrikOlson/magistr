//! Adaptive wizard interview: question graph with conditional logic.
//!
//! Questions are dynamically filtered based on the detected stack and
//! previous answers. Each answer maps to a preset layer slug, wiring
//! directly into the E2 `PresetPipeline` via `LayerOptions`.

use std::collections::HashMap;

use crate::config::preset_full::LayerOptions;
use crate::config::preset_layers;
use crate::types::{Framework, Language, Stack};

// ── Question identity ──────────────────────────────────────────────────

/// Interview dimensions beyond the base stack detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionId {
    /// Database/ORM selection (e.g., "prisma", "diesel", "sqlalchemy").
    Database,
    /// Architecture pattern (monolith, microservices, serverless).
    Architecture,
    /// CI/CD provider (github-actions, gitlab-ci, circleci).
    CiCd,
    /// Deployment target (docker, kubernetes, vercel, fly-io).
    Deployment,
}

impl QuestionId {
    /// All question IDs in display order.
    pub const ALL: &'static [Self] = &[
        Self::Database,
        Self::Architecture,
        Self::CiCd,
        Self::Deployment,
    ];

    /// Human-readable label for the question.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Database => "Database / ORM",
            Self::Architecture => "Architecture Pattern",
            Self::CiCd => "CI/CD Provider",
            Self::Deployment => "Deployment Target",
        }
    }

    /// Prompt text shown to the user.
    #[must_use]
    pub fn prompt(self) -> &'static str {
        match self {
            Self::Database => "Which database or ORM does this project use?",
            Self::Architecture => "What architecture pattern best describes this project?",
            Self::CiCd => "Which CI/CD provider do you use?",
            Self::Deployment => "Where will this project be deployed?",
        }
    }
}

// ── Question options ───────────────────────────────────────────────────

/// A selectable option for a question.
#[derive(Debug, Clone)]
pub struct QuestionOption {
    /// Display label (e.g., "PostgreSQL + SQLx").
    pub label: String,
    /// Slug that maps to a preset layer (e.g., "postgresql-sqlx").
    pub slug: String,
    /// Short description shown as help text.
    pub description: String,
}

impl QuestionOption {
    fn new(label: &str, slug: &str, description: &str) -> Self {
        Self {
            label: label.to_string(),
            slug: slug.to_string(),
            description: description.to_string(),
        }
    }

    /// The "skip" / "none" option, always available.
    fn none() -> Self {
        Self::new("None / Skip", "none", "No selection for this category")
    }
}

// ── Question node ──────────────────────────────────────────────────────

/// A single question in the adaptive interview graph.
#[derive(Debug, Clone)]
pub struct QuestionNode {
    /// Which interview dimension this question covers.
    pub id: QuestionId,
    /// Available options (filtered for the current stack context).
    pub options: Vec<QuestionOption>,
    /// Index of the pre-selected option (from detection or default).
    pub preselected: usize,
}

// ── Answers ────────────────────────────────────────────────────────────

/// Collected answers from the adaptive interview.
#[derive(Debug, Clone, Default)]
pub struct InterviewAnswers {
    answers: HashMap<QuestionId, String>,
}

impl InterviewAnswers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an answer. A slug of "none" is stored but treated as no selection.
    pub fn set(&mut self, id: QuestionId, slug: String) {
        self.answers.insert(id, slug);
    }

    /// Get the answer for a question, if any.
    #[must_use]
    pub fn get(&self, id: QuestionId) -> Option<&str> {
        self.answers.get(&id).map(String::as_str)
    }

    /// Whether an answer has been recorded for this question.
    #[must_use]
    pub fn has(&self, id: QuestionId) -> bool {
        self.answers.contains_key(&id)
    }

    /// Convert answers into `LayerOptions` for the preset pipeline.
    #[must_use]
    pub fn to_layer_options(&self) -> LayerOptions {
        LayerOptions {
            database: self.effective_slug(QuestionId::Database),
            ci: self.effective_slug(QuestionId::CiCd),
            deployment: self.effective_slug(QuestionId::Deployment),
            ..LayerOptions::default()
        }
    }

    /// Return the slug if it's not "none", otherwise None.
    fn effective_slug(&self, id: QuestionId) -> Option<String> {
        self.answers.get(&id).filter(|s| *s != "none").cloned()
    }

    /// Get the architecture answer (not a preset layer, used for conventions).
    #[must_use]
    pub fn architecture(&self) -> Option<&str> {
        self.answers
            .get(&QuestionId::Architecture)
            .filter(|s| *s != "none")
            .map(String::as_str)
    }
}

// ── Question graph ─────────────────────────────────────────────────────

/// Adaptive question graph: produces filtered, ordered questions based
/// on the detected stack and project type.
#[derive(Debug, Clone)]
pub struct QuestionGraph {
    nodes: Vec<QuestionNode>,
}

impl QuestionGraph {
    /// Build a question graph for the given stack and project type.
    ///
    /// Questions are conditionally included based on stack context.
    /// Option lists are filtered by language/framework relevance.
    #[must_use]
    pub fn build(stack: &Stack, project_type: &str) -> Self {
        let mut nodes = Vec::new();

        for &id in QuestionId::ALL {
            if should_show_question(id, stack, project_type) {
                let options = build_options(id, stack);
                let preselected = preselect_index(id, stack, &options);
                nodes.push(QuestionNode {
                    id,
                    options,
                    preselected,
                });
            }
        }

        Self { nodes }
    }

    /// The ordered list of questions to present.
    #[must_use]
    pub fn questions(&self) -> &[QuestionNode] {
        &self.nodes
    }

    /// Number of questions in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph has no questions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

// ── Conditional logic (E3.3) ───────────────────────────────────────────

/// Whether a question should be shown given the current context.
fn should_show_question(id: QuestionId, _stack: &Stack, project_type: &str) -> bool {
    match id {
        // Skip database for CLI tools and libraries — they rarely use ORMs
        QuestionId::Database => !matches!(project_type, "cli" | "library"),

        // Architecture is relevant for web apps, APIs, and monorepos
        QuestionId::Architecture => !matches!(project_type, "cli" | "library"),

        // CI/CD is relevant for all project types
        QuestionId::CiCd => true,

        // Skip deployment for libraries (they're published, not deployed)
        // Show mobile-specific options for Flutter/ReactNative via option filtering
        QuestionId::Deployment => project_type != "library",
    }
}

/// Build the option list for a question, filtered by stack context.
fn build_options(id: QuestionId, stack: &Stack) -> Vec<QuestionOption> {
    let mut options = vec![QuestionOption::none()];

    match id {
        QuestionId::Database => {
            options.extend(database_options(stack.language, stack.framework));
        }
        QuestionId::Architecture => {
            options.extend(architecture_options(stack.framework));
        }
        QuestionId::CiCd => {
            options.extend(ci_options());
        }
        QuestionId::Deployment => {
            options.extend(deployment_options(stack.framework));
        }
    }

    options
}

/// Pre-select an option based on detected stack (e.g., detected CI provider).
fn preselect_index(id: QuestionId, stack: &Stack, options: &[QuestionOption]) -> usize {
    match id {
        QuestionId::CiCd => {
            let ci_str = stack.ci_provider.as_str();
            if ci_str != "none" {
                // Map detected CI provider to slug
                let slug = match ci_str {
                    "github-actions" | "github_actions" => "github-actions",
                    "gitlab-ci" | "gitlab_ci" => "gitlab-ci",
                    "circleci" => "circleci",
                    other => other,
                };
                options.iter().position(|o| o.slug == slug).unwrap_or(0)
            } else {
                0
            }
        }
        // No auto-detection for other dimensions yet
        _ => 0,
    }
}

// ── Option list builders (E3.1) ────────────────────────────────────────

/// Database/ORM options filtered by language relevance.
fn database_options(lang: Language, framework: Framework) -> Vec<QuestionOption> {
    let all_slugs = preset_layers::database_slugs();
    let mut options = Vec::new();

    for &slug in all_slugs {
        if is_database_relevant(slug, lang, framework)
            && let Some(opt) = database_option_meta(slug)
        {
            options.push(opt);
        }
    }

    options
}

/// Whether a database layer is relevant for a language/framework combo.
fn is_database_relevant(slug: &str, lang: Language, _framework: Framework) -> bool {
    match slug {
        // Rust-specific ORMs
        "postgresql-sqlx" | "diesel" => lang == Language::Rust,
        // Python-specific ORM
        "sqlalchemy" => lang == Language::Python,
        // Ruby-specific ORM
        "activerecord" => lang == Language::Ruby,
        // C#-specific ORM
        "entity-framework" => lang == Language::CSharp,
        // JS/TS ORMs — relevant for Node.js frameworks
        "prisma" => matches!(lang, Language::TypeScript | Language::JavaScript),
        "mongodb-mongoose" => matches!(lang, Language::TypeScript | Language::JavaScript),
        // Fallback: show for all languages
        _ => true,
    }
}

/// Metadata for database options.
fn database_option_meta(slug: &str) -> Option<QuestionOption> {
    Some(match slug {
        "postgresql-sqlx" => QuestionOption::new(
            "PostgreSQL + SQLx",
            "postgresql-sqlx",
            "Async, compile-time checked SQL for Rust",
        ),
        "diesel" => QuestionOption::new(
            "Diesel (Rust ORM)",
            "diesel",
            "Safe, extensible ORM and query builder for Rust",
        ),
        "prisma" => QuestionOption::new(
            "Prisma",
            "prisma",
            "Type-safe ORM for Node.js and TypeScript",
        ),
        "mongodb-mongoose" => QuestionOption::new(
            "MongoDB + Mongoose",
            "mongodb-mongoose",
            "Document database with schema validation",
        ),
        "sqlalchemy" => {
            QuestionOption::new("SQLAlchemy", "sqlalchemy", "Python SQL toolkit and ORM")
        }
        "activerecord" => QuestionOption::new(
            "Active Record",
            "activerecord",
            "Ruby on Rails ORM with migrations",
        ),
        "entity-framework" => QuestionOption::new(
            "Entity Framework",
            "entity-framework",
            ".NET ORM with LINQ queries",
        ),
        _ => return None,
    })
}

/// Architecture pattern options (not tied to preset layers — informs conventions).
fn architecture_options(framework: Framework) -> Vec<QuestionOption> {
    let mut options = vec![
        QuestionOption::new(
            "Monolith",
            "monolith",
            "Single deployable unit with all features",
        ),
        QuestionOption::new(
            "Microservices",
            "microservices",
            "Distributed services communicating via APIs",
        ),
        QuestionOption::new(
            "Serverless",
            "serverless",
            "Function-as-a-service (Lambda, Cloud Functions)",
        ),
    ];

    // Add mobile-specific option for mobile frameworks
    if matches!(framework, Framework::Flutter | Framework::ReactNative) {
        options.push(QuestionOption::new(
            "Mobile App",
            "mobile",
            "Native or cross-platform mobile application",
        ));
    }

    options
}

/// CI/CD provider options (all always available).
fn ci_options() -> Vec<QuestionOption> {
    preset_layers::ci_slugs()
        .iter()
        .filter_map(|&slug| ci_option_meta(slug))
        .collect()
}

fn ci_option_meta(slug: &str) -> Option<QuestionOption> {
    Some(match slug {
        "github-actions" => QuestionOption::new(
            "GitHub Actions",
            "github-actions",
            "CI/CD integrated with GitHub repositories",
        ),
        "gitlab-ci" => QuestionOption::new(
            "GitLab CI",
            "gitlab-ci",
            "CI/CD integrated with GitLab repositories",
        ),
        "circleci" => QuestionOption::new(
            "CircleCI",
            "circleci",
            "Cloud-based continuous integration platform",
        ),
        _ => return None,
    })
}

/// Deployment target options filtered by framework relevance.
fn deployment_options(framework: Framework) -> Vec<QuestionOption> {
    let mut options = Vec::new();

    for &slug in preset_layers::deployment_slugs() {
        if is_deployment_relevant(slug, framework)
            && let Some(opt) = deployment_option_meta(slug)
        {
            options.push(opt);
        }
    }

    options
}

/// Whether a deployment target is relevant for a framework.
fn is_deployment_relevant(slug: &str, framework: Framework) -> bool {
    match slug {
        // Vercel is mainly for JS/TS frontend frameworks
        "vercel" => matches!(
            framework,
            Framework::Next
                | Framework::React
                | Framework::Vue
                | Framework::Angular
                | Framework::SvelteKit
                | Framework::Astro
                | Framework::Remix
                | Framework::Nuxt
                | Framework::None // Show for unknown frameworks too
        ),
        // Docker, Kubernetes, Serverless, Fly.io — universal
        _ => true,
    }
}

fn deployment_option_meta(slug: &str) -> Option<QuestionOption> {
    Some(match slug {
        "docker" => QuestionOption::new(
            "Docker",
            "docker",
            "Containerized deployment with Dockerfile",
        ),
        "kubernetes" => QuestionOption::new(
            "Kubernetes",
            "kubernetes",
            "Container orchestration for production workloads",
        ),
        "serverless" => QuestionOption::new(
            "Serverless (Lambda/SAM)",
            "serverless",
            "AWS Lambda or Serverless Framework",
        ),
        "vercel" => QuestionOption::new(
            "Vercel",
            "vercel",
            "Edge-first deployment for frontend frameworks",
        ),
        "fly-io" => QuestionOption::new(
            "Fly.io",
            "fly-io",
            "Global application platform with edge deployment",
        ),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "question_graph.tests.rs"]
mod tests;
