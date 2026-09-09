use std::path::Path;

use crate::permissions::{CharterStore, CockpitPolicy, PermissionProfile};

/// The cockpit session's permission policy for a project: `profile`
/// over the project's auto-allow rules, its quality-gate commands and
/// its protected paths — gathered exactly the way the legacy ACP
/// permission handler was configured. `workflow` turns on the
/// tool-kind auto-approval `magistr next`/`next-loop` run under.
/// `model` lets the roster's `Model` conditions see what this session
/// actually runs on.
///
/// On top of that sits the org's governance policy from the team roster
/// (`.magistr/team.json`), compiled and handed to the session: this is
/// the single construction site every cockpit session goes through, so
/// wiring it here is what makes the guardrails unavoidable rather than
/// advisory. An absent roster compiles to the empty engine, which costs
/// nothing and constrains nothing.
///
/// Under the roster runs the reader's own charter from
/// `.magistr/charter.json`: the rules their answers wrote, applied to
/// what the profile would ask about and written back as they answer.
/// An unreadable charter runs the session on the profile alone rather
/// than on an empty charter the next answer would save over it.
pub fn cockpit_policy_for_project(
    project_dir: &Path,
    profile: PermissionProfile,
    workflow: bool,
    model: Option<&str>,
) -> CockpitPolicy {
    let config = crate::permissions::load_config(project_dir).unwrap_or_default();
    let identity = crate::team::Identity::resolve(project_dir);
    let store = CharterStore::at(project_dir);
    let charter = store.load().ok();
    CockpitPolicy::new(
        profile,
        config.rules,
        super::collect_gate_commands(),
        super::protected_paths_for_project(project_dir),
        workflow,
    )
    .rooted_at(Some(project_dir.to_path_buf()))
    .governed_by(crate::team::TeamConfig::governance_for(project_dir).compile())
    .on_model(model.map(str::to_string))
    .acting_as(Some(identity.id().to_string()))
    .retain(config.retention)
    .chartered(charter.clone().unwrap_or_default(), charter.map(|_| store))
}
