use serde::{Deserialize, Serialize};

/// A marker file that signals a project boundary and its technology stack.
///
/// Each variant maps to a specific manifest or config file. The associated
/// metadata (filename pattern, priority weight, parse strategy) drives the
/// scanner and per-project detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackMarker {
    // Rust
    CargoToml,
    // JavaScript / TypeScript
    PackageJson,
    // Python
    PyprojectToml,
    RequirementsTxt,
    SetupPy,
    // Go
    GoMod,
    // Ruby
    Gemfile,
    // Elixir
    MixExs,
    // Dart / Flutter
    PubspecYaml,
    // PHP
    ComposerJson,
    // Java / Kotlin
    PomXml,
    BuildGradle,
    BuildGradleKts,
    BuildSbt,
    // Swift
    PackageSwift,
    // C# / .NET
    Csproj,
    Sln,
    // C / C++
    CMakeLists,
    // Monorepo / workspace tooling
    NxJson,
    TurboJson,
    PnpmWorkspaceYaml,
    LernaJson,
    MoonYml,
    // Infrastructure
    DockerCompose,
}

impl StackMarker {
    /// All known marker variants.
    pub const ALL: &[StackMarker] = &[
        Self::CargoToml,
        Self::PackageJson,
        Self::PyprojectToml,
        Self::RequirementsTxt,
        Self::SetupPy,
        Self::GoMod,
        Self::Gemfile,
        Self::MixExs,
        Self::PubspecYaml,
        Self::ComposerJson,
        Self::PomXml,
        Self::BuildGradle,
        Self::BuildGradleKts,
        Self::BuildSbt,
        Self::PackageSwift,
        Self::Csproj,
        Self::Sln,
        Self::CMakeLists,
        Self::NxJson,
        Self::TurboJson,
        Self::PnpmWorkspaceYaml,
        Self::LernaJson,
        Self::MoonYml,
        Self::DockerCompose,
    ];

    /// The filename (or pattern) that identifies this marker.
    #[must_use]
    pub fn filename(&self) -> &'static str {
        match self {
            Self::CargoToml => "Cargo.toml",
            Self::PackageJson => "package.json",
            Self::PyprojectToml => "pyproject.toml",
            Self::RequirementsTxt => "requirements.txt",
            Self::SetupPy => "setup.py",
            Self::GoMod => "go.mod",
            Self::Gemfile => "Gemfile",
            Self::MixExs => "mix.exs",
            Self::PubspecYaml => "pubspec.yaml",
            Self::ComposerJson => "composer.json",
            Self::PomXml => "pom.xml",
            Self::BuildGradle => "build.gradle",
            Self::BuildGradleKts => "build.gradle.kts",
            Self::BuildSbt => "build.sbt",
            Self::PackageSwift => "Package.swift",
            Self::Csproj => "*.csproj",
            Self::Sln => "*.sln",
            Self::CMakeLists => "CMakeLists.txt",
            Self::NxJson => "nx.json",
            Self::TurboJson => "turbo.json",
            Self::PnpmWorkspaceYaml => "pnpm-workspace.yaml",
            Self::LernaJson => "lerna.json",
            Self::MoonYml => "moon.yml",
            Self::DockerCompose => "docker-compose.yml",
        }
    }

    /// Priority weight for specificity ordering (lower = more specific).
    ///
    /// Language-specific manifests rank higher than generic tooling markers.
    #[must_use]
    pub fn priority(&self) -> u16 {
        match self {
            // Language manifests — highest priority
            Self::CargoToml => 10,
            Self::PackageJson => 10,
            Self::PyprojectToml => 10,
            Self::GoMod => 10,
            Self::PomXml => 10,
            Self::BuildGradleKts => 10,
            Self::BuildGradle => 12,
            Self::BuildSbt => 10,
            Self::Gemfile => 10,
            Self::MixExs => 10,
            Self::PubspecYaml => 10,
            Self::ComposerJson => 10,
            Self::PackageSwift => 10,
            Self::CMakeLists => 15,
            // .NET project files
            Self::Csproj => 10,
            Self::Sln => 20,
            // Secondary markers
            Self::RequirementsTxt => 30,
            Self::SetupPy => 25,
            // Workspace/monorepo markers
            Self::NxJson => 50,
            Self::TurboJson => 50,
            Self::PnpmWorkspaceYaml => 50,
            Self::LernaJson => 50,
            Self::MoonYml => 50,
            // Infrastructure
            Self::DockerCompose => 60,
        }
    }

    /// Whether this marker uses a glob pattern (e.g., `*.csproj`) rather than
    /// an exact filename.
    #[must_use]
    pub fn is_glob(&self) -> bool {
        matches!(self, Self::Csproj | Self::Sln)
    }

    /// Whether this marker indicates a workspace/monorepo tool rather than a
    /// project boundary.
    #[must_use]
    pub fn is_workspace_tool(&self) -> bool {
        matches!(
            self,
            Self::NxJson
                | Self::TurboJson
                | Self::PnpmWorkspaceYaml
                | Self::LernaJson
                | Self::MoonYml
        )
    }

    /// Whether this marker identifies a project boundary (a directory containing
    /// this file is considered a project root).
    #[must_use]
    pub fn is_project_boundary(&self) -> bool {
        !self.is_workspace_tool() && !matches!(self, Self::DockerCompose)
    }

    /// Try to match a filename to a `StackMarker`.
    #[must_use]
    pub fn from_filename(name: &str) -> Option<Self> {
        match name {
            "Cargo.toml" => Some(Self::CargoToml),
            "package.json" => Some(Self::PackageJson),
            "pyproject.toml" => Some(Self::PyprojectToml),
            "requirements.txt" => Some(Self::RequirementsTxt),
            "setup.py" => Some(Self::SetupPy),
            "go.mod" => Some(Self::GoMod),
            "Gemfile" => Some(Self::Gemfile),
            "mix.exs" => Some(Self::MixExs),
            "pubspec.yaml" => Some(Self::PubspecYaml),
            "composer.json" => Some(Self::ComposerJson),
            "pom.xml" => Some(Self::PomXml),
            "build.gradle" => Some(Self::BuildGradle),
            "build.gradle.kts" => Some(Self::BuildGradleKts),
            "build.sbt" => Some(Self::BuildSbt),
            "Package.swift" => Some(Self::PackageSwift),
            "CMakeLists.txt" => Some(Self::CMakeLists),
            "nx.json" => Some(Self::NxJson),
            "turbo.json" => Some(Self::TurboJson),
            "pnpm-workspace.yaml" => Some(Self::PnpmWorkspaceYaml),
            "lerna.json" => Some(Self::LernaJson),
            "moon.yml" => Some(Self::MoonYml),
            "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
                Some(Self::DockerCompose)
            }
            _ => {
                if name.ends_with(".csproj") {
                    Some(Self::Csproj)
                } else if name.ends_with(".sln") {
                    Some(Self::Sln)
                } else {
                    None
                }
            }
        }
    }
}

impl std::fmt::Display for StackMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.filename())
    }
}

#[cfg(test)]
#[path = "marker.tests.rs"]
mod tests;
