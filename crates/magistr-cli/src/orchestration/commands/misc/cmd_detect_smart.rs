use crate::detect;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use color_eyre::eyre;

pub fn cmd_detect_smart(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let smart = detect::detect_stack_smart(dir).suggest()?;
    let data = serde_json::to_value(&smart)?;
    renderer
        .json(&mut std::io::stdout(), &data)
        .map_err(|e| error::MagistrError::io(dir, e))?;
    Ok(())
}
