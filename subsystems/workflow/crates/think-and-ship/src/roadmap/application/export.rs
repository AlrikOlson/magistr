//! File delivery for the native roadmap renderer.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Resolve a workspace once at server startup: declared project root, git root,
/// then the starting directory. Export never resolves the process cwd again.
pub fn resolve_workspace_root(start: &Path) -> io::Result<PathBuf> {
    let start = start.canonicalize()?;
    let root = crate::infra::project_id::find_project_file(&start)
        .filter(|_| crate::infra::project_id::declared_identity_in(&start).is_some())
        .and_then(|file| file.parent()?.parent().map(Path::to_path_buf))
        .or_else(|| crate::infra::repo_sync::discover_repo_root(&start))
        .unwrap_or(start);
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "workspace must be a directory",
        ));
    }
    Ok(root)
}

fn write_projection(root: &Path, format: &str, body: &str) -> io::Result<PathBuf> {
    let filename = match format {
        "markdown" => "ROADMAP.md",
        "json" => "ROADMAP.json",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported export format",
            ));
        }
    };
    let destination = root.join(filename);
    let permissions = match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_file() => Some(metadata.permissions()),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing symlink or non-file export target: {}",
                    destination.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let mut temporary = tempfile::Builder::new()
        .prefix(".roadmap-export-")
        .tempfile_in(root)?;
    temporary.write_all(body.as_bytes())?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    // Same-directory persist is an atomic replacement. The temporary file is
    // cleaned up on failure, leaving the previous export untouched.
    temporary
        .persist(&destination)
        .map_err(|error| error.error)?;
    Ok(destination)
}

impl super::RoadmapUseCases {
    /// Render and optionally deliver an export under the native owner lock,
    /// so an older rendering cannot replace a newer exported snapshot.
    pub fn export(
        &self,
        request: super::ExportRequest,
    ) -> Result<super::ExportOutcome, super::RoadmapError> {
        use super::{ExportOutcome, ExportTarget, RoadmapError};
        if request.target == ExportTarget::File && self.workspace_root.is_none() {
            return Err(RoadmapError::WorkspaceUnavailable);
        }
        let engine = self.lock()?;
        let body = engine.export(request.format.as_str());
        if request.target == ExportTarget::Inline {
            return Ok(ExportOutcome::Inline {
                format: request.format,
                roadmap: body,
            });
        }
        let root = self
            .workspace_root
            .as_ref()
            .expect("file export workspace checked before rendering");
        let path = write_projection(root, request.format.as_str(), &body)
            .map_err(|error| RoadmapError::ExportFailed(error.to_string()))?;
        Ok(ExportOutcome::File {
            format: request.format,
            path,
            bytes: body.len(),
            written: true,
        })
    }
}
