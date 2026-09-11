/// One tool a plugin manifest declares; the last wire-name segment only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginToolDecl {
    pub name: String,
    pub description: String,
}
