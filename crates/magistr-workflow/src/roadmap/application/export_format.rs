use super::RoadmapError;

/// Supported native roadmap renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
}

impl ExportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Json => "json",
        }
    }
}

impl std::str::FromStr for ExportFormat {
    type Err = RoadmapError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "markdown" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            _ => Err(RoadmapError::InvalidArgs(
                "format must be markdown or json".into(),
            )),
        }
    }
}
