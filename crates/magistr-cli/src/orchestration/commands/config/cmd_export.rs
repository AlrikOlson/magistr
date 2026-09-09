use crate::config;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_export(
    dir: &std::path::Path,
    out_path: Option<&std::path::Path>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let preset = config::shareable::export_preset(&dir).suggest()?;

    let warnings = preset.validate();
    if !warnings.is_empty() && !renderer.is_machine_readable() {
        super::print_validation_warnings(&warnings);
    }

    if renderer.is_machine_readable() {
        let data = serde_json::to_value(&preset)?;
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    let output_path = out_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dir.join(config::shareable::PRESET_FILENAME));

    config::shareable::save_to_file(&output_path, &preset).suggest()?;

    eprintln!(
        "{}",
        fmt::success(&format!("Exported preset to {}", output_path.display()))
    );
    eprintln!(
        "  {}",
        fmt::dim(&format!(
            "Share this file with your team or use `magistr init --from {}` in another project.",
            output_path.display()
        ))
    );

    Ok(())
}
