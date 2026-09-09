use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::marker::StackMarker;
use super::project_role::ProjectRole;
use super::{Framework, Language, Workspace};

/// A single detected project within a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProject {
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// Detected primary language.
    pub language: Language,
    /// Detected framework, if any.
    pub framework: Option<Framework>,
    /// Inferred role within the repository.
    pub role: ProjectRole,
    /// Marker files found in this project's directory.
    pub markers: Vec<StackMarker>,
    /// Parsed metadata: dependency names, versions, etc.
    pub metadata: ProjectMetadata,
    /// Index of the parent project in the tree, if any.
    pub parent: Option<usize>,
    /// Indices of child projects in the tree.
    pub children: Vec<usize>,
}

/// Parsed metadata extracted from a project's manifest files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Project/package name from the manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Key dependencies (framework-defining or notable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// Version string from the manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// A directed edge between two projects in the tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEdge {
    /// Index of the source project (the one that depends).
    pub from: usize,
    /// Index of the target project (the dependency).
    pub to: usize,
    /// How the relationship was detected.
    pub kind: EdgeKind,
}

/// How an inter-project dependency was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Cargo workspace path dependency.
    CargoPathDep,
    /// npm/pnpm/yarn workspace reference.
    NpmWorkspaceRef,
    /// .csproj `<ProjectReference>` element.
    CsprojRef,
    /// Shared directory (e.g., `packages/common`).
    SharedDir,
}

/// The overall topology of the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    /// Single project with no sub-projects.
    SingleProject,
    /// Workspace with a flat list of member projects.
    FlatMonorepo,
    /// A project nested inside another (e.g., SPA inside API).
    NestedFullstack,
    /// A workspace managed by a monorepo tool.
    Workspace,
}

/// The complete detected project graph for a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTree {
    /// Absolute path to the repository root.
    pub root: PathBuf,
    /// All detected projects, indexed by position.
    pub projects: Vec<DetectedProject>,
    /// Inter-project dependency edges.
    pub edges: Vec<ProjectEdge>,
    /// Detected workspace type at the root level.
    pub workspace: Workspace,
    /// Inferred repository topology.
    pub topology: Topology,
}

impl ProjectTree {
    /// Create an empty tree rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            projects: Vec::new(),
            edges: Vec::new(),
            workspace: Workspace::None,
            topology: Topology::SingleProject,
        }
    }

    /// Number of detected projects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.projects.len()
    }

    /// Whether the tree has no projects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    /// Find a project by its path (relative to root).
    #[must_use]
    pub fn find_by_path(&self, path: &std::path::Path) -> Option<(usize, &DetectedProject)> {
        self.projects
            .iter()
            .enumerate()
            .find(|(_, p)| p.path == path)
    }

    /// Build a lookup table from path to project index.
    ///
    /// A polyglot directory yields several projects at one path; the index
    /// keeps the first (the primary stack), so relationship edges attach to
    /// the primary rather than whichever twin was pushed last.
    #[must_use]
    pub fn path_index(&self) -> HashMap<&std::path::Path, usize> {
        let mut index = HashMap::new();
        for (i, p) in self.projects.iter().enumerate() {
            index.entry(p.path.as_path()).or_insert(i);
        }
        index
    }
}

#[cfg(test)]
#[path = "project_tree.tests.rs"]
mod tests;
