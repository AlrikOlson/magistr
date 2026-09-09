use super::*;

#[test]
fn a_resume_ask_becomes_a_session_load_config_with_no_opening_prompt() {
    let root = std::path::Path::new("/tmp/magistr-resume");
    let config = resume_config(root, "sess-42".to_owned(), agent::Backend::Acp);

    assert_eq!(
        config.resume.as_deref(),
        Some("sess-42"),
        "the row's id rides the transport's session/load"
    );
    assert!(
        config.prompt.trim().is_empty(),
        "a resumed cockpit opens waiting for the next turn, not mid-prompt"
    );
    assert_eq!(config.cwd, root);
    assert!(
        !config.continue_conversation,
        "the id is explicit — never the agent's own last conversation"
    );
}
