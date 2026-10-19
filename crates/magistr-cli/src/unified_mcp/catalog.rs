//! The operation catalog owns routing and public schemas together. Native owner
//! listings are the source of truth; discovery and direct tools never copy schemas.
use std::sync::Arc;

use rmcp::{
    ErrorData,
    model::{Tool, ToolAnnotations},
};
use serde_json::{Value, json};

#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Discovery {
    /// Match any of these words in names or descriptions. Omit to list all operations.
    query: Option<String>,
    /// Exact name from a discovery listing.
    operation: Option<String>,
    /// Read schemas for up to 16 exact operation names in one call.
    operations: Option<Vec<String>>,
    /// Include full response schemas. Input schemas are always included for exact-name lookups.
    #[serde(default)]
    include_output_schema: bool,
    #[serde(default)]
    offset: usize,
    /// Page size: 1 through 50; use nextOffset to continue.
    #[serde(default = "default_page_size")]
    #[schemars(range(min = 1, max = 50))]
    limit: usize,
}
fn default_page_size() -> usize {
    20
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Owner {
    Workflow,
    Workspace,
}

#[derive(Clone)]
pub(super) struct Operation {
    pub tool: Tool,
    pub native_name: String,
    pub owner: Owner,
    pub direct: bool,
}

#[derive(Clone)]
pub(super) struct Catalog {
    operations: Vec<Operation>,
}

pub(crate) const DIRECT: &[(&str, &str, &str)] = &[
    (
        "magistr_context",
        "context",
        "Read current project, work and verification context.",
    ),
    (
        "search",
        "search",
        "Search live source by text, filename, references or syntax. Results include truncation.",
    ),
    (
        "read",
        "read",
        "Read a file or symbol with a fresh content hash for safe edits.",
    ),
    (
        "edit",
        "edit",
        "Apply a verified, journaled edit. Use expectedHash from read; inspect confirmation or stale-source failures.",
    ),
    (
        "write",
        "write",
        "Create or replace a file through verified, journaled writes. Use expectedHash when replacing.",
    ),
    (
        "ship_plan",
        "plan_work",
        "Add, remove or reorder tasks in the selected objective.",
    ),
    (
        "ship_start",
        "start_work",
        "Start a planned task in the selected objective.",
    ),
    (
        "ship_record",
        "record_progress",
        "Record an action and its result on the active task.",
    ),
    (
        "ship_check",
        "check_work",
        "Run and record a verification command against the active task.",
    ),
    (
        "ship_complete",
        "finish_work",
        "Complete a task after its required checks pass.",
    ),
];

impl Catalog {
    pub fn new(workflow: Vec<Tool>, workspace: Vec<Tool>) -> Result<Self, ErrorData> {
        let mut operations = Vec::new();
        for (owner, tools) in [(Owner::Workflow, workflow), (Owner::Workspace, workspace)] {
            for mut tool in tools {
                let native_name = tool.name.to_string();
                let direct = DIRECT.iter().find(|(name, _, _)| *name == native_name);
                if let Some((_, name, description)) = direct {
                    tool.name = (*name).to_owned().into();
                    tool.description = Some((*description).to_owned().into());
                    if matches!(*name, "context" | "search" | "read") {
                        tool.annotations = Some(
                            ToolAnnotations::new()
                                .read_only(true)
                                .idempotent(true)
                                .open_world(false),
                        );
                    }
                }
                if let Some(description) = &mut tool.description {
                    let mut text = description.to_string();
                    for (native, public, _) in DIRECT {
                        text = text.replace(native, public);
                    }
                    *description = text.into();
                }
                if operations
                    .iter()
                    .any(|op: &Operation| op.tool.name == tool.name)
                {
                    return Err(ErrorData::internal_error(
                        format!("duplicate operation {}", tool.name),
                        None,
                    ));
                }
                operations.push(Operation {
                    tool,
                    native_name,
                    owner,
                    direct: direct.is_some(),
                });
            }
        }
        for (_, name, _) in DIRECT {
            if !operations.iter().any(|op| op.tool.name == *name) {
                return Err(ErrorData::internal_error(
                    format!("missing operation {name}"),
                    None,
                ));
            }
        }
        operations.sort_by(|a, b| a.tool.name.cmp(&b.tool.name));
        Ok(Self { operations })
    }

    pub fn operation(&self, name: &str) -> Result<&Operation, ErrorData> {
        self.operations
            .iter()
            .find(|op| op.tool.name == name)
            .ok_or_else(|| {
                let family = name.split('_').next().unwrap_or(name);
                let suggestions: Vec<_> = self
                    .operations
                    .iter()
                    .filter(|op| op.tool.name.starts_with(family))
                    .take(16)
                    .map(|op| op.tool.name.as_ref())
                    .collect();
                ErrorData::invalid_params(
                    format!(
                        "Unknown operation {name:?}. Use discover with query to find exact names."
                    ),
                    Some(json!({"suggestions":suggestions})),
                )
            })
    }

    pub fn direct_tools(&self) -> Vec<Tool> {
        let mut tools: Vec<_> = self
            .operations
            .iter()
            .filter(|op| op.direct)
            .map(|op| op.tool.clone())
            .collect();
        tools.push(schema_tool("discover", "Find exact operation names with query (any matching word), then fetch input schemas/examples using operation or operations (up to 16 names). limit is 1..50; follow nextOffset. Request includeOutputSchema for full response schemas. Read-only.",
            serde_json::to_value(rmcp::schemars::schema_for!(Discovery)).expect("discovery schema")));
        tools.push(schema_tool("execute", "Compose catalog operations in bounded JavaScript/WASM. Call magistr.call(name, args) synchronously. Effects run sequentially, stop on failure, and return receipts; never retry the whole script after partial failure.",
            serde_json::to_value(rmcp::schemars::schema_for!(super::composition::Execution)).expect("execution schema")));
        tools
    }

    pub fn discover(&self, args: &Value) -> Result<Value, ErrorData> {
        let args: Discovery = serde_json::from_value(args.clone())
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        if !(1..=50).contains(&args.limit) {
            return Err(ErrorData::invalid_params("limit must be 1..50", None));
        }
        if usize::from(args.operation.is_some())
            + usize::from(args.operations.is_some())
            + usize::from(args.query.is_some())
            > 1
        {
            return Err(ErrorData::invalid_params(
                "Choose query, operation, or operations.",
                None,
            ));
        }
        let view = |name: &str| -> Result<Value, ErrorData> {
            let op = self.operation(name)?;
            let mut tool = op.tool.clone();
            if !args.include_output_schema {
                tool.output_schema = None;
            }
            Ok(json!({"operation":tool,"direct":op.direct,"examples":examples(name)}))
        };
        if let Some(name) = args.operation.as_deref() {
            return view(name);
        }
        if let Some(names) = &args.operations {
            if names.is_empty() || names.len() > 16 {
                return Err(ErrorData::invalid_params(
                    "operations must contain 1..16 exact names",
                    None,
                ));
            }
            let entries = names
                .iter()
                .map(|name| view(name))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(json!({"operations":entries,"truncated":false}));
        }
        let query = args.query.unwrap_or_default().to_lowercase();
        let offset = args.offset;
        let limit = args.limit;
        let found: Vec<_> = self
            .operations
            .iter()
            .filter(|op| {
                query.is_empty()
                    || query.split_whitespace().any(|word| {
                        op.tool.name.to_lowercase().contains(word)
                            || op
                                .tool
                                .description
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(word)
                    })
            })
            .collect();
        let entries: Vec<_> = found.iter().skip(offset).take(limit).map(|op| json!({
            "name":op.tool.name,"direct":op.direct,
            "description":op.tool.description.as_deref().unwrap_or("").split('\n').next().unwrap_or("").chars().take(240).collect::<String>()
        })).collect();
        let next = offset.saturating_add(entries.len());
        Ok(
            json!({"operations":entries,"total":found.len(),"nextOffset":(next < found.len()).then_some(next),"truncated":next < found.len()}),
        )
    }
}

fn schema_tool(name: &'static str, description: &'static str, schema: Value) -> Tool {
    let tool = Tool::new(
        name,
        description,
        Arc::new(schema.as_object().cloned().unwrap_or_default()),
    );
    if name == "discover" {
        tool.with_annotations(
            ToolAnnotations::new()
                .read_only(true)
                .idempotent(true)
                .open_world(false),
        )
    } else {
        tool
    }
}

fn examples(name: &str) -> Vec<Value> {
    let args = match name {
        "context" | "ship_status" | "roadmap_status" | "workspace_status" => json!({}),
        "search" => json!({"query":"TODO","mode":"lexical","budget":2000}),
        "read" => json!({"file":"README.md","budget":2000}),
        "map" => json!({"path":".","depth":2,"budget":2000}),
        "plan_work" => {
            json!({"action":"add","task_id":"investigate","title":"Investigate the requested change","task_type":"research"})
        }
        "start_work" | "finish_work" => json!({"task_id":"investigate"}),
        "undo" => json!({"steps":1}),
        "ship_set_objective" => {
            json!({"description":"Investigate the requested change","scope":"investigation","verification":"focused"})
        }
        "signal_capture" => {
            json!({"kind":"bug","body":"Steps to reproduce and observed behavior."})
        }
        _ => return Vec::new(),
    };
    vec![json!({"operation":name,"arguments":args})]
}

#[cfg(test)]
#[path = "catalog.tests.rs"]
mod tests;
