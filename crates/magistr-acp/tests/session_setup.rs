//! Explicit session choices must be applied or rejected before inference.
mod fake_agent;

use agent_client_protocol::schema::v1::{Meta, SessionConfigKind};
use fake_agent::{FABLE, Script};
use magistr_acp::{AcpEvent, AcpTransport};
use std::path::PathBuf;

fn requested(model: Option<&str>, effort: Option<&str>) -> Meta {
    let mut session = serde_json::Map::new();
    if let Some(value) = model {
        session.insert("model".into(), value.into());
    }
    if let Some(value) = effort {
        session.insert("effort".into(), value.into());
    }
    let mut meta = Meta::new();
    meta.insert("magistr".into(), serde_json::json!({"session": session}));
    meta
}

#[tokio::test]
async fn explicit_model_is_selected_before_the_transport_is_ready() {
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_meta(Some(requested(Some(FABLE), None)));
    transport
        .connect_over(client)
        .await
        .expect("configured handshake");
    assert_eq!(
        observed.config_sets.lock().expect("lock").as_slice(),
        &[("model".to_owned(), FABLE.to_owned())]
    );
    let mut events = transport.read_events().expect("events");
    let AcpEvent::Initialized { config_options, .. } =
        events.recv().await.expect("event").expect("ok")
    else {
        panic!("initialized");
    };
    let options = config_options.expect("config");
    let SessionConfigKind::Select(select) = &options[0].kind else {
        panic!("model select");
    };
    assert_eq!(select.current_value.to_string(), FABLE);
    assert!(transport.is_ready());
}

#[tokio::test]
async fn an_unknown_model_fails_before_the_first_prompt() {
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_meta(Some(requested(Some("not-a-model"), None)));
    let error = transport
        .connect_over(client)
        .await
        .expect_err("unknown model");
    assert!(error.to_string().contains("not-a-model"), "{error}");
    assert!(!transport.is_ready());
    assert!(transport.prompt("must not infer").is_err());
    assert!(observed.config_sets.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn unsupported_effort_is_not_silently_discarded() {
    let (client, _observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_meta(Some(requested(None, Some("high"))));
    let error = transport
        .connect_over(client)
        .await
        .expect_err("no effort capability");
    assert!(error.to_string().contains("effort"), "{error}");
    assert!(!transport.is_ready());
}

#[tokio::test]
async fn ignored_model_selection_is_refused_before_readiness() {
    let (client, _observed, _agent) = fake_agent::spawn(Script {
        ignore_config_changes: true,
        ..Script::default()
    });
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_model(Some(FABLE.to_owned()));
    let error = transport
        .connect_over(client)
        .await
        .expect_err("selection ignored");
    assert!(error.to_string().contains("did not acknowledge"), "{error}");
    assert!(!transport.is_ready());
}

#[tokio::test]
async fn a_preferred_model_that_is_offered_is_selected_like_an_explicit_one() {
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_model_preferred(Some(FABLE.to_owned()));
    transport.connect_over(client).await.expect("handshake");
    assert_eq!(
        observed.config_sets.lock().expect("lock").as_slice(),
        &[("model".to_owned(), FABLE.to_owned())]
    );
    assert!(transport.is_ready());
}

#[tokio::test]
async fn a_stale_preferred_model_resolves_to_its_family_or_keeps_the_agent_default() {
    // The fake agent offers claude-fable-5[1m]; an older configured release
    // id of the same family lands on it.
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_model_preferred(Some("claude-fable[1m]".to_owned()));
    transport.connect_over(client).await.expect("handshake");
    assert_eq!(
        observed.config_sets.lock().expect("lock").as_slice(),
        &[("model".to_owned(), FABLE.to_owned())]
    );
    assert!(transport.is_ready());

    // An id outside every offered family keeps the agent's default and the
    // session still becomes ready: a stale user default never blocks work.
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_model_preferred(Some("claude-fable-4[1m]".to_owned()));
    transport.connect_over(client).await.expect("handshake");
    assert!(observed.config_sets.lock().expect("lock").is_empty());
    let mut events = transport.read_events().expect("events");
    let AcpEvent::Initialized { config_options, .. } =
        events.recv().await.expect("event").expect("ok")
    else {
        panic!("initialized");
    };
    let options = config_options.expect("config");
    let SessionConfigKind::Select(select) = &options[0].kind else {
        panic!("model select");
    };
    assert_eq!(select.current_value.to_string(), "sonnet");
    assert!(transport.is_ready());
}

#[tokio::test]
async fn an_explicit_model_wins_over_a_preferred_one() {
    let (client, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake"), vec![], None)
        .expect("transport")
        .session_model(Some(FABLE.to_owned()))
        .session_model_preferred(Some("sonnet".to_owned()));
    transport.connect_over(client).await.expect("handshake");
    assert_eq!(
        observed.config_sets.lock().expect("lock").as_slice(),
        &[("model".to_owned(), FABLE.to_owned())]
    );
}
