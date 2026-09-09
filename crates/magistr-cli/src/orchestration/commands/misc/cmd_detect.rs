use crate::detect;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use color_eyre::eyre;

pub fn cmd_detect(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let stack = detect::detect_stack(dir).suggest()?;
    let data = serde_json::to_value(&stack)?;
    renderer
        .json(&mut std::io::stdout(), &data)
        .map_err(|e| error::MagistrError::io(dir, e))?;
    Ok(())
}
