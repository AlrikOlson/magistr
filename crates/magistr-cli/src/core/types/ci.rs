use serde::{Deserialize, Serialize};

/// CI/CD provider detected in the project.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CiProvider {
    GitHubActions,
    GitLabCi,
    CircleCi,
    TravisCi,
    None,
}

impl CiProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GitHubActions => "github-actions",
            Self::GitLabCi => "gitlab-ci",
            Self::CircleCi => "circleci",
            Self::TravisCi => "travis-ci",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for CiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
