use super::*;
use alloc::string::ToString;
use core::num::NonZeroU32;

fn project(key: &str) -> ProjectId {
    ProjectId::new(key).expect("fixture project")
}

fn run(project: ProjectId) -> RunId {
    RunId::new(project, "run-1").expect("fixture run")
}

fn session(run: RunId) -> SessionId {
    SessionId::new(run, "session-1").expect("fixture session")
}

#[test]
fn local_keys_do_not_alias_across_project_run_session_or_command_scopes() {
    let left = run(project("left"));
    let right = run(project("right"));
    assert_ne!(left, right);
    let another_run = RunId::new(project("left"), "run-2").expect("fixture");
    assert_ne!(session(left.clone()), session(another_run));
    let left_session = session(left.clone());
    let right_session = session(right.clone());
    assert_ne!(left_session, right_session);
    let turn = TurnId::new(left_session.clone(), "turn-1").expect("fixture");
    let other_turn = TurnId::new(right_session, "turn-1").expect("fixture");
    assert_ne!(turn, other_turn);
    let command = CommandId::new(left, "command-1").expect("fixture");
    let other_command = CommandId::new(right, "command-1").expect("fixture");
    assert_ne!(command, other_command);
    let first = NonZeroU32::new(1).expect("nonzero");
    let second = NonZeroU32::new(2).expect("nonzero");
    assert_ne!(
        AttemptId::new(command.clone(), first),
        AttemptId::new(command.clone(), second)
    );
    assert_ne!(
        AttemptId::new(command.clone(), first),
        AttemptId::new(other_command, first)
    );
    assert_ne!(
        DecisionId::new(turn.clone(), "decision-1").expect("fixture"),
        DecisionId::new(other_turn, "decision-1").expect("fixture")
    );
    assert_eq!(turn.session(), &left_session);
    assert_eq!(command.run(), left_session.run());
    assert_eq!(turn.local(), "turn-1");
}

#[test]
fn identity_keys_are_opaque_but_reject_empty_or_control_characters() {
    assert_eq!(ProjectId::new(""), Err(IdentityError::Empty));
    assert_eq!(
        ProjectId::new("bad\nkey"),
        Err(IdentityError::ControlCharacter)
    );
    let opaque = project("Case / α:17");
    assert_eq!(opaque.as_str(), "Case / α:17");
    assert_ne!(opaque, project("case / α:17"));
    assert!(RunId::new(project("p"), "").is_err());
    assert!(ProviderSessionId::new("", "remote").is_err());
    assert!(ProviderSessionId::new("codex", "").is_err());
}

#[test]
fn provider_binding_requires_an_allocated_magistr_scope_and_preserves_opaque_id() {
    let local = session(run(project("p")));
    let remote = ProviderSessionId::new("codex", "thread:ABC/42").expect("fixture");
    let binding = remote.clone().bind_to(local.clone());
    assert_eq!(binding.session(), &local);
    assert_eq!(binding.provider_session(), &remote);
    assert_eq!(remote.provider(), "codex");
    assert_eq!(remote.as_str(), "thread:ABC/42");
    assert_ne!(
        remote,
        ProviderSessionId::new("claude", "thread:ABC/42").expect("fixture")
    );
    assert_ne!(binding, remote.bind_to(session(run(project("other")))));
}

#[test]
fn scoped_identity_serialization_round_trips_without_losing_parents() {
    let command = CommandId::new(run(project("p")), "cmd").expect("fixture");
    let attempt = AttemptId::new(command, NonZeroU32::new(1).expect("nonzero"));
    let wire = serde_json::to_string(&attempt).expect("serialize");
    assert_eq!(
        serde_json::from_str::<AttemptId>(&wire).expect("deserialize"),
        attempt
    );
    let corrupted = wire.replace("\"p\"", "\"\"");
    assert!(serde_json::from_str::<AttemptId>(&corrupted).is_err());
    let zero_attempt = wire.replace("\"number\":1", "\"number\":0");
    assert!(serde_json::from_str::<AttemptId>(&zero_attempt).is_err());
    assert_eq!(
        IdentityError::Empty.to_string(),
        "identity must not be empty"
    );
}
