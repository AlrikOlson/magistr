//! Project discovery combines resident owner snapshots with settled session history.
use crate::config::inputs::ProjectInputs;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct RecentProject {
    pub name: String,
    pub directory: String,
}

pub fn recent_projects(inputs: &ProjectInputs, active: &[PathBuf]) -> Vec<RecentProject> {
    let mut projects: Vec<_> = crate::commands::galaxy_data::collect(None, inputs)
        .projects
        .into_iter()
        .filter_map(|project| {
            project.dir.map(|directory| RecentProject {
                name: project.name,
                directory,
            })
        })
        .collect();
    let mut known: BTreeSet<_> = projects.iter().map(|p| p.directory.clone()).collect();
    for workspace in active {
        let directory = magistr_engine::native::ProjectLocation::resolve(workspace)
            .map(|location| location.authority_root)
            .unwrap_or_else(|_| workspace.to_path_buf());
        let directory = directory
            .canonicalize()
            .unwrap_or(directory)
            .display()
            .to_string();
        if known.insert(directory.clone()) {
            projects.push(RecentProject {
                name: crate::commands::galaxy_data::project_name(Path::new(&directory)),
                directory,
            });
        }
    }
    projects
}
#[cfg(test)]
#[path = "project_catalog.tests.rs"]
mod tests;
