//! File export over the mounted think-and-ship roadmap service.
//! Rendering stays in that live service; only its inline result stays in-process.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use magistr_workflow::mcp::UnifiedService;
use rmcp::ErrorData;
use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult, Tool, ToolAnnotations};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct RoadmapExport {
    root: Option<PathBuf>,
    // One snapshot-to-replacement lane across cloned services: an older
    // export must never finish after and overwrite a newer export.
    serialization: Arc<tokio::sync::Mutex<()>>,
}

impl RoadmapExport {
    pub(super) fn new(root: &Path) -> Self {
        Self {
            root: root.canonicalize().ok(),
            serialization: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub(super) fn tool() -> Tool {
        let input = json!({
            "type": "object", "additionalProperties": false,
            "properties": {
                "format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"},
                "output": {"type": "string", "enum": ["file", "inline"], "default": "file",
                    "description": "file writes the generated view and returns a receipt; inline returns its text without writing"}
            }
        });
        let output = json!({
            "type": "object",
            "oneOf": [
                {"required": ["format", "path", "bytes", "written"], "properties": {
                    "format": {"type": "string"}, "path": {"type": "string"},
                    "bytes": {"type": "integer", "minimum": 0}, "written": {"const": true}
                }},
                {"required": ["format", "roadmap"], "properties": {
                    "format": {"type": "string"}, "roadmap": {"type": "string"}
                }},
                {"required": ["ok", "error_kind", "message"], "properties": {
                    "ok": {"const": false}, "error_kind": {"type": "string"}, "message": {"type": "string"}
                }}
            ]
        });
        let mut tool = Tool::new(
            "roadmap_export",
            "Write the current roadmap directly to ROADMAP.md (or ROADMAP.json) in the project root. \
             Returns only {format,path,bytes,written}, never the document in file mode. \
             Do not copy or edit the generated file manually. Use output:'inline' only to read the \
             full document without writing. Replaces regular files atomically; refuses symlinks and directories.",
            Arc::new(input.as_object().expect("object schema").clone()),
        );
        tool.output_schema = Some(Arc::new(output.as_object().expect("object schema").clone()));
        tool.annotations = Some(
            ToolAnnotations::new()
                .read_only(false)
                .destructive(true)
                .idempotent(true)
                .open_world(false),
        );
        tool
    }

    pub(super) async fn call(
        &self,
        unified: &UnifiedService,
        mut request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request.arguments.get_or_insert_with(Default::default);
        let format = match args.get("format") {
            None => "markdown",
            Some(Value::String(s)) if s == "markdown" => "markdown",
            Some(Value::String(s)) if s == "json" => "json",
            _ => return Ok(failure("invalid_args", "format must be markdown or json").into()),
        };
        let inline = match args.get("output") {
            None => false,
            Some(Value::String(s)) if s == "file" => false,
            Some(Value::String(s)) if s == "inline" => true,
            _ => return Ok(failure("invalid_args", "output must be file or inline").into()),
        };
        if args.keys().any(|key| key != "format" && key != "output") {
            return Ok(failure("invalid_args", "only format and output are supported").into());
        }
        let _guard = if inline {
            None
        } else {
            Some(self.serialization.lock().await)
        };
        if !inline {
            let Some(root) = &self.root else {
                return Ok(failure(
                    "workspace_unavailable",
                    "project root could not be resolved at startup",
                )
                .into());
            };
            if let Err(error) = magistr_core::roadmap::ensure_file_export_allowed(root) {
                return Ok(failure("export_disabled", error.to_string()).into());
            }
        }
        // Ask the mounted service only to render. Magistr owns the file
        // replacement, so the upstream service must not write a second file.
        args.insert("output".into(), json!("inline"));
        let result = unified.call_tool(request, context).await?;
        if inline {
            return Ok(result);
        }
        let CallToolResponse::Complete(result) = result else {
            return Ok(result);
        };
        let body =
            match magistr_workflow::infra::checked_tool_result::CheckedToolResult::body(&result) {
                Ok(body) => body,
                Err(error) => return Ok((*error).into()),
            };
        let Some(document) = body["roadmap"].as_str() else {
            return Ok(failure(
                "export_failed",
                format!("roadmap renderer returned no document: {body}"),
            )
            .into());
        };
        let Some(root) = &self.root else {
            return Ok(failure(
                "workspace_unavailable",
                "project root could not be resolved at startup",
            )
            .into());
        };
        let path = root.join(if format == "json" {
            "ROADMAP.json"
        } else {
            "ROADMAP.md"
        });
        // Rendering awaited the native service: policy may have changed while
        // this export was in flight, even though it passed admission above.
        if let Err(error) = magistr_core::roadmap::ensure_file_export_allowed(root) {
            return Ok(failure("export_disabled", error.to_string()).into());
        }
        match write_export(&path, document) {
            Ok(()) => Ok(CallToolResult::structured(json!({
                "format": format, "path": path, "bytes": document.len(), "written": true
            }))
            .into()),
            Err(error) => {
                Ok(failure("export_failed", format!("{}: {error}", path.display())).into())
            }
        }
    }
}

fn failure(kind: &str, message: impl Into<String>) -> CallToolResult {
    magistr_workflow::infra::tool_failure::ToolFailure::uncertain(kind, message)
}

fn write_export(path: &Path, document: &str) -> std::io::Result<()> {
    let existing = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Some(metadata.permissions()),
        Ok(_) => {
            return Err(std::io::Error::other(
                "export target must be a regular file, not a symlink or directory",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let root = path
        .parent()
        .ok_or_else(|| std::io::Error::other("export has no parent"))?;
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    temp.write_all(document.as_bytes())?;
    if let Some(permissions) = existing {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}
