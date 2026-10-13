use crate::application::WorkingCopies;
use crate::working_copy_action::WorkingCopyAction;
use std::io::Write;
use std::path::Path;

/// Dispatch through the application owner and render its records.
pub fn run_working_copy(dir: &Path, action: WorkingCopyAction) -> color_eyre::eyre::Result<()> {
    let service = WorkingCopies::at(dir)?;
    let value = match action {
        WorkingCopyAction::List => serde_json::to_value(service.list()?)?,
        WorkingCopyAction::Create { path, branch, from } => {
            serde_json::to_value(service.create(&path, &branch, &from)?)?
        }
        WorkingCopyAction::Open { path } => serde_json::to_value(service.open(&path)?)?,
        WorkingCopyAction::Archive { path } => serde_json::to_value(service.archive(&path)?)?,
    };
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &value)?;
    writeln!(out)?;
    Ok(())
}
