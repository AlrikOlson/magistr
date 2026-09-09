use super::*;

fn default_stack() -> Stack {
    Stack::default()
}

fn rust_cli_stack() -> Stack {
    Stack {
        language: Language::Rust,
        framework: Framework::None,
        ..Stack::default()
    }
}

fn python_fastapi_stack() -> Stack {
    Stack {
        language: Language::Python,
        framework: Framework::FastApi,
        ..Stack::default()
    }
}

fn typescript_next_stack() -> Stack {
    Stack {
        language: Language::TypeScript,
        framework: Framework::Next,
        ..Stack::default()
    }
}

fn flutter_stack() -> Stack {
    Stack {
        language: Language::Dart,
        framework: Framework::Flutter,
        ..Stack::default()
    }
}

fn rust_axum_stack() -> Stack {
    Stack {
        language: Language::Rust,
        framework: Framework::Axum,
        ..Stack::default()
    }
}

// ── E3.0: QuestionGraph structure ──────────────────────────────────────

#[test]
fn question_graph_builds_for_web_app() {
    let stack = rust_axum_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    // Web apps get all 4 questions
    assert_eq!(graph.len(), 4);
    let ids: Vec<QuestionId> = graph.questions().iter().map(|q| q.id).collect();
    assert!(ids.contains(&QuestionId::Database));
    assert!(ids.contains(&QuestionId::Architecture));
    assert!(ids.contains(&QuestionId::CiCd));
    assert!(ids.contains(&QuestionId::Deployment));
}

#[test]
fn question_graph_is_not_empty_for_api() {
    let stack = python_fastapi_stack();
    let graph = QuestionGraph::build(&stack, "api");
    assert!(!graph.is_empty());
    assert_eq!(graph.len(), 4);
}

// ── E3.1: Encyclopedia-driven option lists ─────────────────────────────

#[test]
fn database_options_filtered_by_language_rust() {
    let stack = rust_axum_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    let db_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    let slugs: Vec<&str> = db_node.options.iter().map(|o| o.slug.as_str()).collect();

    // Rust should see SQLx and Diesel
    assert!(slugs.contains(&"postgresql-sqlx"));
    assert!(slugs.contains(&"diesel"));
    // But not Python/Ruby/C#/JS ORMs
    assert!(!slugs.contains(&"sqlalchemy"));
    assert!(!slugs.contains(&"activerecord"));
    assert!(!slugs.contains(&"entity-framework"));
    assert!(!slugs.contains(&"prisma"));
}

#[test]
fn database_options_filtered_by_language_python() {
    let stack = python_fastapi_stack();
    let graph = QuestionGraph::build(&stack, "api");

    let db_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    let slugs: Vec<&str> = db_node.options.iter().map(|o| o.slug.as_str()).collect();

    assert!(slugs.contains(&"sqlalchemy"));
    assert!(!slugs.contains(&"diesel"));
    assert!(!slugs.contains(&"prisma"));
}

#[test]
fn database_options_filtered_by_language_typescript() {
    let stack = typescript_next_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    let db_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    let slugs: Vec<&str> = db_node.options.iter().map(|o| o.slug.as_str()).collect();

    assert!(slugs.contains(&"prisma"));
    assert!(slugs.contains(&"mongodb-mongoose"));
    assert!(!slugs.contains(&"diesel"));
    assert!(!slugs.contains(&"sqlalchemy"));
}

#[test]
fn all_options_include_none_first() {
    let stack = rust_axum_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    for node in graph.questions() {
        assert_eq!(
            node.options[0].slug, "none",
            "Question {:?} missing 'none' first option",
            node.id
        );
    }
}

// ── E3.2: New interview dimensions ─────────────────────────────────────

#[test]
fn architecture_options_include_mobile_for_flutter() {
    let stack = flutter_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    let arch_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Architecture)
        .unwrap();
    let slugs: Vec<&str> = arch_node.options.iter().map(|o| o.slug.as_str()).collect();

    assert!(
        slugs.contains(&"mobile"),
        "Flutter should see mobile architecture option"
    );
    assert!(slugs.contains(&"monolith"));
}

#[test]
fn architecture_options_no_mobile_for_web() {
    let stack = rust_axum_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    let arch_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Architecture)
        .unwrap();
    let slugs: Vec<&str> = arch_node.options.iter().map(|o| o.slug.as_str()).collect();

    assert!(!slugs.contains(&"mobile"));
}

// ── E3.3: Conditional question logic ───────────────────────────────────

#[test]
fn cli_projects_skip_database_and_architecture() {
    let stack = rust_cli_stack();
    let graph = QuestionGraph::build(&stack, "cli");

    let ids: Vec<QuestionId> = graph.questions().iter().map(|q| q.id).collect();

    assert!(
        !ids.contains(&QuestionId::Database),
        "CLI projects should skip database"
    );
    assert!(
        !ids.contains(&QuestionId::Architecture),
        "CLI projects should skip architecture"
    );
    assert!(
        ids.contains(&QuestionId::CiCd),
        "CLI projects should still get CI/CD"
    );
    assert!(
        ids.contains(&QuestionId::Deployment),
        "CLI projects should still get deployment"
    );
}

#[test]
fn library_projects_skip_database_architecture_deployment() {
    let stack = default_stack();
    let graph = QuestionGraph::build(&stack, "library");

    let ids: Vec<QuestionId> = graph.questions().iter().map(|q| q.id).collect();

    assert!(!ids.contains(&QuestionId::Database));
    assert!(!ids.contains(&QuestionId::Architecture));
    assert!(!ids.contains(&QuestionId::Deployment));
    // Only CI/CD remains
    assert_eq!(graph.len(), 1);
    assert_eq!(graph.questions()[0].id, QuestionId::CiCd);
}

#[test]
fn ci_preselects_detected_provider() {
    use crate::types::CiProvider;

    let stack = Stack {
        ci_provider: CiProvider::GitHubActions,
        ..Stack::default()
    };
    let graph = QuestionGraph::build(&stack, "web-app");

    let ci_node = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::CiCd)
        .unwrap();
    let preselected_slug = &ci_node.options[ci_node.preselected].slug;

    assert_eq!(preselected_slug, "github-actions");
}

// ── E3.4: Wire answers into preset layer system ────────────────────────

#[test]
fn answers_convert_to_layer_options() {
    let mut answers = InterviewAnswers::new();
    answers.set(QuestionId::Database, "prisma".to_string());
    answers.set(QuestionId::CiCd, "github-actions".to_string());
    answers.set(QuestionId::Deployment, "docker".to_string());
    answers.set(QuestionId::Architecture, "microservices".to_string());

    let opts = answers.to_layer_options();

    assert_eq!(opts.database.as_deref(), Some("prisma"));
    assert_eq!(opts.ci.as_deref(), Some("github-actions"));
    assert_eq!(opts.deployment.as_deref(), Some("docker"));
    // Architecture is not a preset layer
    assert!(opts.project_root.is_none());
}

#[test]
fn none_answers_produce_empty_layer_options() {
    let mut answers = InterviewAnswers::new();
    answers.set(QuestionId::Database, "none".to_string());
    answers.set(QuestionId::CiCd, "none".to_string());

    let opts = answers.to_layer_options();

    assert!(opts.database.is_none());
    assert!(opts.ci.is_none());
    assert!(opts.deployment.is_none());
}

#[test]
fn architecture_answer_accessible_separately() {
    let mut answers = InterviewAnswers::new();
    answers.set(QuestionId::Architecture, "serverless".to_string());

    assert_eq!(answers.architecture(), Some("serverless"));
}

#[test]
fn architecture_none_returns_none() {
    let mut answers = InterviewAnswers::new();
    answers.set(QuestionId::Architecture, "none".to_string());

    assert_eq!(answers.architecture(), None);
}

// ── E3.5: Representative stack traversals ──────────────────────────────

#[test]
fn traversal_rust_axum_web_app() {
    let stack = rust_axum_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    // Should get 4 questions
    assert_eq!(graph.len(), 4);

    // Database should offer Rust-specific ORMs
    let db = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    assert!(db.options.iter().any(|o| o.slug == "postgresql-sqlx"));

    // Simulate answering all questions
    let mut answers = InterviewAnswers::new();
    for node in graph.questions() {
        // Pick the first non-none option if available
        let slug = node
            .options
            .get(1)
            .map(|o| o.slug.as_str())
            .unwrap_or("none");
        answers.set(node.id, slug.to_string());
    }

    let opts = answers.to_layer_options();
    assert!(opts.database.is_some());
}

#[test]
fn traversal_typescript_next_web_app() {
    let stack = typescript_next_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    assert_eq!(graph.len(), 4);

    let db = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    assert!(db.options.iter().any(|o| o.slug == "prisma"));

    let deploy = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Deployment)
        .unwrap();
    assert!(deploy.options.iter().any(|o| o.slug == "vercel"));
}

#[test]
fn traversal_python_fastapi_api() {
    let stack = python_fastapi_stack();
    let graph = QuestionGraph::build(&stack, "api");

    assert_eq!(graph.len(), 4);

    let db = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Database)
        .unwrap();
    assert!(db.options.iter().any(|o| o.slug == "sqlalchemy"));
    assert!(!db.options.iter().any(|o| o.slug == "diesel"));
}

#[test]
fn traversal_flutter_mobile() {
    let stack = flutter_stack();
    let graph = QuestionGraph::build(&stack, "web-app");

    let arch = graph
        .questions()
        .iter()
        .find(|q| q.id == QuestionId::Architecture)
        .unwrap();
    assert!(arch.options.iter().any(|o| o.slug == "mobile"));
}

#[test]
fn traversal_rust_cli() {
    let stack = rust_cli_stack();
    let graph = QuestionGraph::build(&stack, "cli");

    // CLI: only CI/CD and Deployment
    assert_eq!(graph.len(), 2);

    let ids: Vec<QuestionId> = graph.questions().iter().map(|q| q.id).collect();
    assert!(ids.contains(&QuestionId::CiCd));
    assert!(ids.contains(&QuestionId::Deployment));
}
