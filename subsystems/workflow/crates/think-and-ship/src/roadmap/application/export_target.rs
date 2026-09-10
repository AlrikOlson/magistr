use super::RoadmapError;

/// Return a rendering or write it in the configured workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    File,
    Inline,
}

impl std::str::FromStr for ExportTarget {
    type Err = RoadmapError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "file" => Ok(Self::File),
            "inline" => Ok(Self::Inline),
            _ => Err(RoadmapError::InvalidArgs(
                "output must be file or inline".into(),
            )),
        }
    }
}
