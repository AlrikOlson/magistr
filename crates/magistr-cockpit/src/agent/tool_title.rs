//! Familiar names for provider tool identifiers shown in the interface.

/// Converts a tool's technical name while retaining its supplied description.
pub struct ToolTitle;

impl ToolTitle {
    /// Human-readable titles pass through; tool identifiers never become interface copy.
    #[must_use]
    pub fn display(title: &str) -> String {
        let (name, detail) = title
            .split_once(": ")
            .map_or((title, None), |(name, detail)| (name, Some(detail)));
        let leaf = name
            .rsplit("__")
            .next()
            .unwrap_or(name)
            .rsplit('.')
            .next()
            .unwrap_or(name);
        let label = match leaf {
            "ship_set_objective" => Some("Set the work objective"),
            "ship_plan" => Some("Plan the work"),
            "ship_start" => Some("Start a work item"),
            "ship_record" => Some("Save a work update"),
            "ship_check" | "magistr_gate" => Some("Check the work"),
            "ship_complete" => Some("Finish a work item"),
            "ship_finalize" => Some("Finish the work"),
            "ship_status" | "ship_get_task" | "ship_get_plan" => Some("Review current work"),
            "magistr_set_phase" => Some("Update work progress"),
            "think_record_step" => Some("Record reasoning"),
            "think_start" | "think_begin" => Some("Start recording reasoning"),
            "think_branch" => Some("Explore a question"),
            "think_merge" => Some("Use the findings"),
            "think_revise" => Some("Revise an earlier conclusion"),
            "think_get_step"
            | "think_get_branch"
            | "think_list_branches"
            | "think_engine_status"
            | "think_load_context" => Some("Review recorded reasoning"),
            "roadmap_add_chunk" => Some("Add a roadmap item"),
            "roadmap_update_chunk" => Some("Update a roadmap item"),
            "roadmap_start_chunk" => Some("Start a roadmap item"),
            "roadmap_complete_chunk" => Some("Finish a roadmap item"),
            "roadmap_obsolete_chunk" => Some("Remove an outdated roadmap item"),
            "roadmap_link" | "roadmap_add_dependency" => Some("Link roadmap items"),
            "roadmap_status"
            | "roadmap_get_chunk"
            | "roadmap_list_chunks"
            | "roadmap_next"
            | "roadmap_ready" => Some("Review the roadmap"),
            "read" | "read_file" => Some("Read files"),
            "search" | "search_files" | "code_search" => Some("Search files"),
            "map" | "list_files" => Some("Explore the project"),
            "edit" | "edit_file" | "apply_patch" | "write_file" => Some("Update files"),
            "files" => Some("Manage files"),
            "run" | "exec_command" | "Bash" | "bash" => Some("Run a command"),
            "workbench_status" => Some("Check the project connection"),
            "undo" => Some("Undo a change"),
            "web_search" => Some("Search the web"),
            "web_fetch" => Some("Read a web page"),
            _ => None,
        };
        let technical = name.contains("__")
            || name.starts_with("mcp_")
            || (name.contains('_')
                && !name.chars().any(char::is_whitespace)
                && !name.contains('/'));
        let label =
            label.filter(|_| technical || (detail.is_some() && matches!(name, "Bash" | "bash")));
        match (label.or(technical.then_some("Use a tool")), detail) {
            (Some(label), Some(detail)) if !detail.trim().is_empty() => {
                format!("{label}: {detail}")
            }
            (Some(label), _) => label.to_owned(),
            (None, _) => title.to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "tool_title.tests.rs"]
mod tests;
