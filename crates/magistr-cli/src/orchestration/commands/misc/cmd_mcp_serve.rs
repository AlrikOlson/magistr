use crate::mcp_tools;
use color_eyre::eyre;

/// Run magistr's built-in MCP server on stdio (rmcp; mounts the
/// think-and-ship families beside the `magistr_*` tools).
///
/// The scaffold is repaired first. This is the seam every magistr project
/// already crosses on every session — the server starts once per project per
/// session, before any tool is called — so a fix that lands in a generated
/// file arrives by *using* magistr rather than by remembering to re-run
/// `magistr init` somewhere. It is best-effort by construction
/// ([`scaffold::repair`](crate::scaffold::repair::repair) writes only files
/// magistr owns outright, only when they exist, and only when they differ)
/// and cannot fail the server: a repair that could stop the thing it runs
/// before would be worse than the staleness it fixes.
pub async fn cmd_mcp_serve(dir: &std::path::Path) -> eyre::Result<()> {
    let project_root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let repaired = crate::scaffold::repair::repair(&project_root);
    if !repaired.is_empty() {
        // At `info`, which the default `WARN` filter hides: a repair is
        // news exactly once and worth having on request (`RUST_LOG=info`),
        // but a line in a client's server log every time a project is one
        // version behind is noise. Silence is what a current project gets
        // either way — the list is empty and nothing is logged at all.
        tracing::info!(
            target: "magistr",
            files = %repaired.join(", "),
            "scaffold repaired to this magistr's version"
        );
    }
    mcp_tools::run_stdio_server(project_root).await
}

#[cfg(test)]
#[path = "cmd_mcp_serve.tests.rs"]
mod tests;
