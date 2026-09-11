/// One resolver-confirmed reference edge, admitted through the workspace
/// jail: workspace-relative path, 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEdge {
    pub path: String,
    pub line: u32,
    pub column: u32,
}
