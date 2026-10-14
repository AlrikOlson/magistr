use super::*;
use crate::config::inputs::ProjectInputs;

#[test]
fn a_new_choice_replaces_launch_choices_and_allocates_its_own_history() {
    let original = AgentConfig::new("initial prompt", "/project")
        .provider("claude")
        .model("launch model")
        .resume("provider ID")
        .continue_conversation(true);
    let chosen = SessionOpening::New {
        provider: "copilot".into(),
        model: None,
    }
    .apply(original)
    .unwrap();
    assert_eq!(chosen.provider.as_deref(), Some("copilot"));
    assert_eq!(chosen.model, None);
    assert_eq!(chosen.resume, None);
    assert!(!chosen.continue_conversation);
    assert!(chosen.prompt.is_empty());
}
#[test]
fn refused_saved_open_never_changes_the_launch_config_or_creates_history() {
    let folder = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(folder.path(), |_| None)
        .with_data_dir(folder.path().join("history"));
    let chosen = AgentConfig::new("", folder.path())
        .with_inputs(&inputs)
        .provider("copilot");
    assert!(
        SessionOpening::Resume {
            id: "missing".into()
        }
        .apply(chosen)
        .is_err()
    );
    assert!(!folder.path().join("history/sessions.db").exists());
}
#[test]
fn an_initial_advertised_model_reaches_normal_resolution() {
    let chosen = SessionOpening::New {
        provider: "copilot".into(),
        model: Some("advertised".into()),
    }
    .apply(AgentConfig::new("", "/project"))
    .unwrap();
    assert_eq!(chosen.model.as_deref(), Some("advertised"));
}
