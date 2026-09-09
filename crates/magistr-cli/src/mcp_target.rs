use clap::ValueEnum;

/// Built-in stdio endpoints with one portable launch definition for every host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum McpTarget {
    Magistr,
    #[value(name = "workbench-mcp")]
    Workbench,
}

impl McpTarget {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Magistr => "magistr",
            Self::Workbench => "workbench-mcp",
        }
    }

    /// Resolve the Magistr executable on the receiving machine's PATH.
    #[must_use]
    pub const fn command(self) -> &'static str {
        "magistr"
    }

    /// Keep the workspace as one literal argument, including spaces and quotes.
    #[must_use]
    pub fn args(self, project_root: &str) -> Vec<String> {
        vec!["mcp-launch".into(), self.name().into(), project_root.into()]
    }

    /// Claude's serializer; other hosts can use the same command and argv.
    #[must_use]
    pub fn mcp_entry(self, project_root: &str) -> serde_json::Value {
        serde_json::json!({
            "command": self.command(),
            "args": self.args(project_root),
            "type": "stdio",
        })
    }
}
