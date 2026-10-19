//! Miscellaneous commands: detect, reset, completions, man-pages,
//! learn, memory, plugins, roadmap, mcp-proxy, mcp, acp-agent.

use crate::detect;

// ─── Detection ──────────────────────────────────────────────────────────────

pub(crate) fn flag_if_low<T>(d: &detect::detection::Detection<T>) -> &'static str {
    if d.needs_override() {
        "⚠ suggest override"
    } else {
        ""
    }
}

// ─── Post-scaffold validation ───────────────────────────────────────────────

// ─── Scaffold management ────────────────────────────────────────────────────

// ─── Completions & man pages ────────────────────────────────────────────────

// ─── Keys ───────────────────────────────────────────────────────────────────

// ─── Learn & Memory ─────────────────────────────────────────────────────────

// ─── MCP ────────────────────────────────────────────────────────────────────

mod audit_roster_edit;
mod build_roadmap_generation_prompt;
mod build_template_vars;
mod cmd_acp_agent;
mod cmd_codex_remove;
mod cmd_completions;
mod cmd_completions_install;
mod cmd_detect;
mod cmd_detect_smart;
mod cmd_detect_tree;
mod cmd_learn;
mod cmd_login;
mod cmd_logout;
mod cmd_man_pages;
mod cmd_mcp_proxy;
mod cmd_memory;
mod cmd_plans;
mod cmd_plugins;
mod cmd_policy;
mod cmd_reset;
mod cmd_roadmap;
mod cmd_team;
mod cmd_workspace;
mod completions_install_path;
mod edit_roster;
mod ensure_no_existing_scaffold;
mod login_provider;
mod print_post_scaffold_validation;
mod print_smart_stack;
mod print_wizard_summary;
mod roadmap_native_render;

pub use audit_roster_edit::audit_roster_edit;
pub use build_roadmap_generation_prompt::build_roadmap_generation_prompt;
pub use build_template_vars::build_template_vars;
pub use cmd_acp_agent::cmd_acp_agent;
pub use cmd_codex_remove::cmd_codex_remove;
pub use cmd_completions::cmd_completions;
pub use cmd_completions_install::cmd_completions_install;
pub use cmd_detect::cmd_detect;
pub use cmd_detect_smart::cmd_detect_smart;
pub use cmd_detect_tree::cmd_detect_tree;
pub use cmd_learn::cmd_learn;
pub use cmd_login::cmd_login;
pub use cmd_logout::cmd_logout;
pub use cmd_man_pages::cmd_man_pages;
pub use cmd_mcp_proxy::cmd_mcp_proxy;
pub use cmd_memory::cmd_memory;
pub use cmd_plans::cmd_plans;
pub use cmd_plugins::cmd_plugins;
pub use cmd_policy::cmd_policy;
pub use cmd_reset::cmd_reset;
pub use cmd_roadmap::cmd_roadmap;
pub use cmd_team::cmd_team;
pub use cmd_workspace::cmd_workspace;
pub use completions_install_path::completions_install_path;
pub use edit_roster::edit_roster;
pub use ensure_no_existing_scaffold::ensure_no_existing_scaffold;
pub use print_post_scaffold_validation::print_post_scaffold_validation;
pub use print_smart_stack::print_smart_stack;
pub use print_wizard_summary::print_wizard_summary;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
