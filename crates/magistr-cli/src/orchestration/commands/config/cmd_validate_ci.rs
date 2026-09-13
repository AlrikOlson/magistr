use crate::error::{self, ResultExt};
use crate::output::{self, annotations};
use color_eyre::eyre;

/// CI mode validation — JSON output, proper exit codes, no TUI.
///
/// When `emit_annotations` is true, also prints GitHub Actions workflow
/// commands (`::error`, `::warning`) so failures appear inline in PR diffs.
pub async fn cmd_validate_ci(dir: &std::path::Path, emit_annotations: bool) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;
    let result = crate::validate::ProjectValidation::run(&dir)
        .await
        .suggest()?;

    // Emit GitHub Actions annotations before JSON output so they appear
    // in the PR diff even if the step fails.
    if emit_annotations {
        let annotation_output = annotations::format_annotations(&result);
        if !annotation_output.is_empty() {
            print!("{annotation_output}");
        }
    }

    let validation = output::ValidationOutput::from(&result);
    let envelope = output::success(&validation);
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    // A run cut short by an interrupt is reported as one, not as a red gate.
    crate::utils::shutdown::Shutdown::exit_if_requested();
    if !result.all_passed() {
        std::process::exit(1);
    }
    Ok(())
}
