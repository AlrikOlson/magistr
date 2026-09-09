use super::*;
use crate::config::shareable::PresetSource;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_init_from_preset(
    dir: &std::path::Path,
    source: &str,
    force: bool,
    agents: &[String],
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    crate::commands::misc::ensure_no_existing_scaffold(&dir, force)?;

    let preset_source = PresetSource::parse(source);
    let preset = preset_source.load().suggest()?;

    let warnings = preset.validate();
    print_validation_warnings(&warnings);
    if preset.has_validation_errors() {
        return Err(eyre::eyre!(
            "preset has validation errors — fix the issues above or use a different preset"
        ));
    }

    let lang = parse_language_or_unknown(&preset.stack.language);
    let project_name = detect::detect_project_name(&dir, lang);
    let mut vars = preset.to_template_vars(&project_name);
    if !agents.is_empty() {
        vars.insert("selected_agents".to_owned(), agents.join(","));
    }

    let files = scaffold::generate(&dir, &vars, &std::collections::HashMap::new()).suggest()?;

    let source_label = match &preset_source {
        PresetSource::Local(_) => "file",
        PresetSource::Url(_) => "URL",
    };
    eprintln!(
        "{} Initialized from {source_label} preset {} — {} files scaffolded",
        fmt::green("✓"),
        fmt::bold(&preset.name),
        files.len()
    );
    for f in &files {
        eprintln!("  {}", fmt::dim(f));
    }

    Ok(())
}
