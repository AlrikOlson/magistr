use crate::contracts::{ErrorKind, WorkspaceError};

/// Mint a distinct, nonempty session identity: `<prefix>-<32 hex digits>` from
/// the operating system's random source. The owner uses it for a connection
/// that supplied no identity; the bridge uses it for the durable identity it
/// stamps on every `tools/call` of one client process.
///
/// # Errors
/// The random source is unavailable; reported as `ENGINE_UNAVAILABLE`.
pub fn mint_session_identity(prefix: &str) -> Result<String, WorkspaceError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        WorkspaceError::new(
            ErrorKind::EngineUnavailable,
            format!("cannot mint workspace session identity: {error}"),
        )
    })?;
    let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("{prefix}-{id}"))
}
