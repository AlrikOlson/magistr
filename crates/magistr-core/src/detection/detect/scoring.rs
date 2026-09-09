//! Scoring context and trait for detection confidence scoring.
//!
//! Provides `ScoringContext` (shared data for all scorers) and the `Scorable`
//! trait.  Each detection type (Language, Framework, etc.) implements `Scorable`
//! to produce a `Detection<Self>` with a confidence score and evidence.

use std::path::Path;

use super::confidence::Confidence;
use super::config_parser::ManifestData;
use super::detection::Detection;
use crate::types::Language;

/// Shared context for scoring functions.
///
/// Holds the project directory, detected language, and parsed manifest data.
/// Passed to every `Scorable::score()` implementation so they don't need
/// to re-parse or re-detect shared data.
#[allow(dead_code)] // Fields used as scoring expands
pub struct ScoringContext<'a> {
    /// The project root directory.
    pub dir: &'a Path,
    /// The detected primary language.
    pub language: Language,
    /// Parsed package manifest data (package.json, Cargo.toml, etc.).
    pub(crate) manifest: &'a ManifestData,
}

/// Trait for types that can be scored with a detection confidence.
///
/// Each detection target (Language, Framework, PackageManager, etc.)
/// implements this trait to produce a `Detection<Self>` from a shared
/// `ScoringContext`.
pub trait Scorable: Sized {
    /// Score the detected value based on project evidence.
    ///
    /// Returns a `Detection<Self>` with a confidence level and human-readable
    /// reason/evidence for the score.
    fn score(&self, ctx: &ScoringContext<'_>) -> Detection<Self>;
}

// ── Language scoring ─────────────────────────────────────────────────────────

impl Scorable for Language {
    fn score(&self, ctx: &ScoringContext<'_>) -> Detection<Self> {
        match self {
            Language::Unknown => Detection::none(*self),
            Language::Rust => Detection::new(*self, Confidence::high(), "Cargo.toml found"),
            Language::TypeScript => {
                if ctx.dir.join("tsconfig.json").exists() {
                    Detection::new(*self, Confidence::high(), "tsconfig.json found")
                } else {
                    Detection::new(
                        *self,
                        Confidence::medium(),
                        "typescript in package.json dependencies",
                    )
                }
            }
            Language::JavaScript => Detection::new(
                *self,
                Confidence::new(85),
                "package.json found without TypeScript",
            ),
            Language::Python => {
                let mut evidence = Vec::new();
                if ctx.dir.join("pyproject.toml").exists() {
                    evidence.push("pyproject.toml found".to_string());
                }
                if ctx.dir.join("setup.py").exists() {
                    evidence.push("setup.py found".to_string());
                }
                if ctx.dir.join("setup.cfg").exists() {
                    evidence.push("setup.cfg found".to_string());
                }
                let confidence = if evidence.len() >= 2 {
                    Confidence::high()
                } else {
                    Confidence::new(90)
                };
                Detection::with_evidence(*self, confidence, evidence)
            }
            Language::Go => Detection::new(*self, Confidence::high(), "go.mod found"),
            Language::Java => {
                if ctx.dir.join("pom.xml").exists() {
                    Detection::new(*self, Confidence::high(), "pom.xml found")
                } else {
                    Detection::new(*self, Confidence::new(85), "build.gradle found")
                }
            }
            Language::Kotlin => {
                if ctx.dir.join("build.gradle.kts").exists() {
                    Detection::new(*self, Confidence::high(), "build.gradle.kts found")
                } else {
                    Detection::new(
                        *self,
                        Confidence::medium(),
                        "kotlin references in build.gradle",
                    )
                }
            }
            Language::Ruby => Detection::new(*self, Confidence::high(), "Gemfile found"),
            Language::Elixir => Detection::new(*self, Confidence::high(), "mix.exs found"),
            Language::Swift => Detection::new(*self, Confidence::high(), "Package.swift found"),
            Language::Cpp => {
                if ctx.dir.join("CMakeLists.txt").exists() {
                    Detection::new(*self, Confidence::new(90), "CMakeLists.txt found")
                } else {
                    Detection::new(*self, Confidence::low(), "C++ references in Makefile")
                }
            }
            Language::CSharp => {
                Detection::new(*self, Confidence::new(90), ".csproj or .sln file found")
            }
            Language::Dart => Detection::new(*self, Confidence::high(), "pubspec.yaml found"),
            Language::Php => Detection::new(*self, Confidence::new(85), "composer.json found"),
            Language::Scala => Detection::new(*self, Confidence::high(), "build.sbt found"),
        }
    }
}

#[cfg(test)]
#[path = "scoring.tests.rs"]
mod tests;
