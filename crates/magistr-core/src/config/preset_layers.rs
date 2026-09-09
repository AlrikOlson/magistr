//! Embedded preset layer fragments for database, CI/CD, and deployment.
//!
//! Each fragment follows the `PresetOverride` schema and is loaded
//! by slug name within its layer type.

use crate::error::{MagistrError, Result};
use crate::types::{PresetLayer, PresetLayerType, PresetOverride};

// ── Database/ORM layers ─────────────────────────────────────────

const DB_POSTGRESQL_SQLX: &str = include_str!("../../presets/layers/database/postgresql-sqlx.json");
const DB_MONGODB_MONGOOSE: &str =
    include_str!("../../presets/layers/database/mongodb-mongoose.json");
const DB_PRISMA: &str = include_str!("../../presets/layers/database/prisma.json");
const DB_DIESEL: &str = include_str!("../../presets/layers/database/diesel.json");
const DB_SQLALCHEMY: &str = include_str!("../../presets/layers/database/sqlalchemy.json");
const DB_ACTIVERECORD: &str = include_str!("../../presets/layers/database/activerecord.json");
const DB_ENTITY_FRAMEWORK: &str =
    include_str!("../../presets/layers/database/entity-framework.json");

// ── CI/CD layers ────────────────────────────────────────────────

const CI_GITHUB_ACTIONS: &str = include_str!("../../presets/layers/ci/github-actions.json");
const CI_GITLAB_CI: &str = include_str!("../../presets/layers/ci/gitlab-ci.json");
const CI_CIRCLECI: &str = include_str!("../../presets/layers/ci/circleci.json");

// ── Deployment layers ───────────────────────────────────────────

const DEPLOY_DOCKER: &str = include_str!("../../presets/layers/deployment/docker.json");
const DEPLOY_KUBERNETES: &str = include_str!("../../presets/layers/deployment/kubernetes.json");
const DEPLOY_SERVERLESS: &str = include_str!("../../presets/layers/deployment/serverless.json");
const DEPLOY_VERCEL: &str = include_str!("../../presets/layers/deployment/vercel.json");
const DEPLOY_FLY_IO: &str = include_str!("../../presets/layers/deployment/fly-io.json");

/// Load an embedded database/ORM preset layer by slug.
///
/// Returns `None` if the slug is not recognized (graceful fallback).
pub fn load_database_layer(slug: &str) -> Result<Option<PresetLayer>> {
    let json = match slug {
        "postgresql-sqlx" | "sqlx" => Some(DB_POSTGRESQL_SQLX),
        "mongodb-mongoose" | "mongoose" | "mongodb" => Some(DB_MONGODB_MONGOOSE),
        "prisma" => Some(DB_PRISMA),
        "diesel" => Some(DB_DIESEL),
        "sqlalchemy" | "alembic" => Some(DB_SQLALCHEMY),
        "activerecord" | "active-record" => Some(DB_ACTIVERECORD),
        "entity-framework" | "ef" => Some(DB_ENTITY_FRAMEWORK),
        _ => None,
    };

    parse_optional_layer(json, slug, PresetLayerType::Database)
}

/// Load an embedded CI/CD preset layer by slug.
///
/// Returns `None` if the slug is not recognized.
pub fn load_ci_layer(slug: &str) -> Result<Option<PresetLayer>> {
    let json = match slug {
        "github-actions" | "github" | "gha" => Some(CI_GITHUB_ACTIONS),
        "gitlab-ci" | "gitlab" => Some(CI_GITLAB_CI),
        "circleci" | "circle" => Some(CI_CIRCLECI),
        _ => None,
    };

    parse_optional_layer(json, slug, PresetLayerType::CiCd)
}

/// Load an embedded deployment preset layer by slug.
///
/// Returns `None` if the slug is not recognized.
pub fn load_deployment_layer(slug: &str) -> Result<Option<PresetLayer>> {
    let json = match slug {
        "docker" | "docker-compose" => Some(DEPLOY_DOCKER),
        "kubernetes" | "k8s" => Some(DEPLOY_KUBERNETES),
        "serverless" | "lambda" | "sam" => Some(DEPLOY_SERVERLESS),
        "vercel" => Some(DEPLOY_VERCEL),
        "fly-io" | "fly" | "flyio" => Some(DEPLOY_FLY_IO),
        _ => None,
    };

    parse_optional_layer(json, slug, PresetLayerType::Deployment)
}

/// List all available database layer slugs.
pub fn database_slugs() -> &'static [&'static str] {
    &[
        "postgresql-sqlx",
        "mongodb-mongoose",
        "prisma",
        "diesel",
        "sqlalchemy",
        "activerecord",
        "entity-framework",
    ]
}

/// List all available CI/CD layer slugs.
pub fn ci_slugs() -> &'static [&'static str] {
    &["github-actions", "gitlab-ci", "circleci"]
}

/// List all available deployment layer slugs.
pub fn deployment_slugs() -> &'static [&'static str] {
    &["docker", "kubernetes", "serverless", "vercel", "fly-io"]
}

fn parse_optional_layer(
    json: Option<&str>,
    slug: &str,
    layer_type: PresetLayerType,
) -> Result<Option<PresetLayer>> {
    match json {
        Some(j) => {
            let overrides: PresetOverride = serde_json::from_str(j).map_err(|e| {
                MagistrError::Config(format!("{layer_type} layer '{slug}' parse error: {e}"))
            })?;
            Ok(Some(PresetLayer {
                layer_type,
                name: slug.to_string(),
                overrides,
            }))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "preset_layers.tests.rs"]
mod tests;
