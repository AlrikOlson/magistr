//! The sortable columns of the roadmap table.

use super::Widget;

/// The active ordering, independent of the selected work item's identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoadmapSort {
    Name,
    #[default]
    Status,
    Priority,
    Workstream,
}

impl RoadmapSort {
    pub const ALL: [Self; 4] = [Self::Name, Self::Status, Self::Priority, Self::Workstream];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Status => "Status",
            Self::Priority => "Pri",
            Self::Workstream => "Stream",
        }
    }

    #[must_use]
    pub const fn widget(self) -> Widget {
        match self {
            Self::Name => Widget::RoadmapSortName,
            Self::Status => Widget::RoadmapSortStatus,
            Self::Priority => Widget::RoadmapSortPriority,
            Self::Workstream => Widget::RoadmapSortWorkstream,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Name => Self::Status,
            Self::Status => Self::Priority,
            Self::Priority => Self::Workstream,
            Self::Workstream => Self::Name,
        }
    }
}
