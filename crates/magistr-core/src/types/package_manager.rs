use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Cargo,
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Pip,
    Uv,
    Poetry,
    Pipenv,
    Go,
    Maven,
    Gradle,
    Bundler,
    Mix,
    Spm,
    Cmake,
    Meson,
    Make,
    Dotnet,
    Pub,
    Composer,
    Sbt,
    Unknown,
}

impl PackageManager {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
            Self::Pip => "pip",
            Self::Uv => "uv",
            Self::Poetry => "poetry",
            Self::Pipenv => "pipenv",
            Self::Go => "go",
            Self::Maven => "maven",
            Self::Gradle => "gradle",
            Self::Bundler => "bundler",
            Self::Mix => "mix",
            Self::Spm => "spm",
            Self::Cmake => "cmake",
            Self::Meson => "meson",
            Self::Make => "make",
            Self::Dotnet => "dotnet",
            Self::Pub => "pub",
            Self::Composer => "composer",
            Self::Sbt => "sbt",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
