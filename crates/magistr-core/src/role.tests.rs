use super::*;

#[test]
fn built_in_roles_count() {
    let roles = SpecializedRole::built_in_roles();
    assert_eq!(roles.len(), 5);
}

#[test]
fn researcher_role_properties() {
    let role = SpecializedRole::researcher();
    assert_eq!(role.role_id, "researcher");
    assert_eq!(role.display_name, "Researcher");
    assert!(role.constraints.read_only);
    assert!(!role.denied_tools.is_empty());
    assert!(role.denied_tools.iter().any(|t| t == "Edit"));
    assert!(role.denied_tools.iter().any(|t| t == "Write"));
    assert!(role.system_prompt_template.contains("{task}"));
    assert_eq!(role.icon, '\u{1f50d}');
}

#[test]
fn coder_role_properties() {
    let role = SpecializedRole::coder();
    assert_eq!(role.role_id, "coder");
    assert_eq!(role.display_name, "Coder");
    assert!(!role.constraints.read_only);
    assert!(role.denied_tools.is_empty());
    assert!(role.system_prompt_template.contains("{task}"));
    assert!(role.system_prompt_template.contains("TDD"));
}

#[test]
fn reviewer_role_properties() {
    let role = SpecializedRole::reviewer();
    assert_eq!(role.role_id, "reviewer");
    assert!(role.constraints.read_only);
    assert!(role.denied_tools.iter().any(|t| t == "Edit"));
}

#[test]
fn tester_role_properties() {
    let role = SpecializedRole::tester();
    assert_eq!(role.role_id, "tester");
    assert!(!role.constraints.read_only);
    assert!(role.system_prompt_template.contains("test"));
}

#[test]
fn documenter_role_properties() {
    let role = SpecializedRole::documenter();
    assert_eq!(role.role_id, "documenter");
    assert!(role.system_prompt_template.contains("document"));
}

#[test]
fn role_constraints_default() {
    let constraints = RoleConstraints::default();
    assert!(!constraints.read_only);
    assert_eq!(constraints.max_iterations, 0);
}

#[test]
fn specialized_role_render_prompt() {
    let role = SpecializedRole::researcher();
    let prompt = role.render_prompt("Find all usages of AgentPool");
    assert!(prompt.contains("Find all usages of AgentPool"));
    assert!(!prompt.contains("{task}"));
}

#[test]
fn built_in_by_id_found() {
    let role = SpecializedRole::built_in_by_id("coder");
    assert!(role.is_some());
    assert_eq!(role.unwrap().role_id, "coder");
}

#[test]
fn built_in_by_id_not_found() {
    let role = SpecializedRole::built_in_by_id("nonexistent");
    assert!(role.is_none());
}

#[test]
fn role_display() {
    let role = SpecializedRole::researcher();
    assert_eq!(format!("{role}"), "Researcher");
}

#[test]
fn model_preference_and_fallback() {
    let role = SpecializedRole::researcher();
    assert!(role.model_preference.is_some());
    assert!(!role.model_fallback.is_empty());
}

#[test]
fn effective_model_returns_preference_first() {
    let role = SpecializedRole::researcher();
    let model = role.effective_model();
    assert_eq!(model, role.model_preference.as_deref());
}

#[test]
fn effective_model_none_when_no_preference() {
    let mut role = SpecializedRole::researcher();
    role.model_preference = None;
    role.model_fallback.clear();
    assert!(role.effective_model().is_none());
}

#[test]
fn description_for_known_roles() {
    assert!(SpecializedRole::description_for("researcher").contains("Investigates"));
    assert!(SpecializedRole::description_for("coder").contains("Implements"));
    assert!(SpecializedRole::description_for("reviewer").contains("Reviews"));
    assert!(SpecializedRole::description_for("tester").contains("tests"));
    assert!(SpecializedRole::description_for("documenter").contains("documentation"));
}

#[test]
fn description_for_unknown_role() {
    let desc = SpecializedRole::description_for("custom_role");
    assert!(desc.contains("Custom role"));
}
