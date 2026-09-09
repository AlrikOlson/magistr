use crate::error::ResultExt;
use crate::scaffold;
use color_eyre::eyre;

/// Preflight existing configuration without removing scaffold or workflow state.
///
/// The force argument remains accepted for CLI compatibility. Both modes use
/// generation's ownership policies; only explicit reset removes project state.
pub fn ensure_no_existing_scaffold(dir: &std::path::Path, _force: bool) -> eyre::Result<()> {
    scaffold::existing_config::ExistingConfig::validate(dir).suggest()
}
