//! The ACP agent: one handler per client→agent method, over any transport.

use std::sync::Arc;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, AuthenticateRequest, CancelNotification, Implementation, InitializeRequest,
    InitializeResponse, LoadSessionRequest, LoadSessionResponse, Meta, NewSessionRequest,
    NewSessionResponse, PromptCapabilities, PromptRequest, SessionConfigOption,
    SessionConfigOptionValue, SessionId, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::{
    Agent, Client, ConnectTo, ConnectionTo, Error, on_receive_notification, on_receive_request,
};

use crate::model_options::{MODEL_CONFIG_ID, model_options};
use crate::queued_turn::QueuedTurn;
use crate::session_registry::SessionRegistry;
use crate::session_worker::run_session_worker;
use crate::{AgentError, Provider, SessionStart};

/// Serve `provider` as an ACP agent over `transport` until the client closes
/// the connection.
///
/// Handlers answer inline when they can (`initialize`, `authenticate`,
/// `session/prompt` — which only queues, `session/cancel`,
/// `session/set_config_option`) and spawn when they must wait: opening a
/// session asks the provider to start, and a turn runs on the session's
/// worker task, so no handler ever holds the dispatch loop on the provider.
pub async fn serve(
    provider: impl Provider,
    transport: impl ConnectTo<Agent> + 'static,
) -> Result<(), Error> {
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let registry = SessionRegistry::new();

    Agent
        .builder()
        .name(format!("magistr-acp-agent/{}", provider.name()))
        .on_receive_request(
            {
                let provider = Arc::clone(&provider);
                async move |_req: InitializeRequest, responder, _cx| {
                    responder.respond(initialize_response(&*provider))
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            async move |req: AuthenticateRequest, responder, _cx| {
                // No auth methods are advertised, so none can be selected: a
                // provider that needs credentials finds them in its own
                // environment (the way `claude` and the API keys work).
                responder.respond_with_result(Err(Error::invalid_params().data(serde_json::json!(
                    format!("unknown auth method: {}", req.method_id)
                ))))
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let (provider, registry) = (Arc::clone(&provider), registry.clone());
                async move |req: NewSessionRequest, responder, cx| {
                    let (provider, registry) = (Arc::clone(&provider), registry.clone());
                    let start = SessionStart {
                        session_id: registry.mint_id(),
                        cwd: req.cwd,
                        mcp_servers: req.mcp_servers,
                        resume: false,
                    };
                    let cx2 = cx.clone();
                    cx.spawn(async move {
                        let opened = open_session(provider, registry, cx2, start).await;
                        responder.respond_with_result(
                            opened
                                .map(|(session_id, options)| {
                                    NewSessionResponse::new(session_id).config_options(options)
                                })
                                .map_err(Error::from),
                        )
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let (provider, registry) = (Arc::clone(&provider), registry.clone());
                async move |req: LoadSessionRequest, responder, cx| {
                    let (provider, registry) = (Arc::clone(&provider), registry.clone());
                    let start = SessionStart {
                        session_id: req.session_id,
                        cwd: req.cwd,
                        mcp_servers: req.mcp_servers,
                        resume: true,
                    };
                    let cx2 = cx.clone();
                    cx.spawn(async move {
                        let opened = open_session(provider, registry, cx2, start).await;
                        responder.respond_with_result(
                            opened
                                .map(|(_, options)| {
                                    LoadSessionResponse::new().config_options(options)
                                })
                                .map_err(Error::from),
                        )
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req: PromptRequest, responder, _cx| {
                    let (queue, cancel) = match registry.admit(&req.session_id) {
                        Ok(admitted) => admitted,
                        Err(error) => return responder.respond_with_result(Err(error.into())),
                    };
                    let session_id = req.session_id.clone();
                    match queue.send(QueuedTurn {
                        request: req,
                        responder,
                        cancel,
                    }) {
                        Ok(()) => Ok(()),
                        Err(failed) => {
                            failed
                                .0
                                .responder
                                .respond_with_result(Err(AgentError::provider(format!(
                                    "session {session_id} has no worker"
                                ))
                                .into()))
                        }
                    }
                }
            },
            on_receive_request!(),
        )
        .on_receive_notification(
            {
                let registry = registry.clone();
                async move |n: CancelNotification, _cx| {
                    if let Err(e) = registry.cancel(&n.session_id) {
                        tracing::debug!(error = %e, "session/cancel for a session not open here");
                    }
                    Ok(())
                }
            },
            on_receive_notification!(),
        )
        .on_receive_request(
            {
                let (provider, registry) = (Arc::clone(&provider), registry.clone());
                async move |req: SetSessionConfigOptionRequest, responder, _cx| {
                    responder.respond_with_result(
                        set_config_option(&*provider, &registry, &req).map_err(Error::from),
                    )
                }
            },
            on_receive_request!(),
        )
        .connect_to(transport)
        .await
}

/// What `initialize` answers: v1, `session/load` and embedded context
/// supported, the agent named, and the provider and its models in
/// `_meta.magistr` for a client that wants them before opening a session.
fn initialize_response(provider: &dyn Provider) -> InitializeResponse {
    let models = provider
        .models()
        .iter()
        .map(|m| serde_json::json!({"id": m.id, "label": m.label}))
        .collect::<Vec<_>>();
    let mut meta = Meta::new();
    meta.insert(
        "magistr".to_owned(),
        serde_json::json!({"provider": provider.name(), "models": models}),
    );
    InitializeResponse::new(ProtocolVersion::V1)
        .agent_capabilities(
            AgentCapabilities::new()
                .load_session(true)
                .prompt_capabilities(PromptCapabilities::new().embedded_context(true)),
        )
        .agent_info(Implementation::new("magistr", env!("CARGO_PKG_VERSION")))
        .meta(meta)
}

/// Start the session on the provider, register it under the id the provider
/// answered with, spawn its worker, and replay whatever history the provider
/// returned. Yields the session's id and the model picker.
async fn open_session(
    provider: Arc<dyn Provider>,
    registry: SessionRegistry,
    cx: ConnectionTo<Client>,
    start: SessionStart,
) -> Result<(SessionId, Option<Vec<SessionConfigOption>>), AgentError> {
    let proposed = start.session_id.clone();
    let resume = start.resume;
    if registry.contains(&proposed) {
        return Err(AgentError::SessionExists(proposed.to_string()));
    }
    let opened = provider.start(start).await?;
    let session_id = opened.session_id;
    if resume && session_id != proposed {
        return Err(AgentError::provider(format!(
            "session/load named {proposed} but the provider restored {session_id}"
        )));
    }
    if session_id != proposed && registry.contains(&session_id) {
        return Err(AgentError::SessionExists(session_id.to_string()));
    }
    let replay = opened.replay;
    let models = provider.models();
    let model = match opened.model {
        Some(model) if models.iter().any(|choice| choice.id == model) => Some(model),
        Some(model) => return Err(AgentError::UnknownModel(model)),
        None => models.first().map(|choice| choice.id.clone()),
    };
    let queue = registry.open(session_id.clone(), model.clone())?;
    cx.spawn(run_session_worker(
        Arc::clone(&provider),
        registry,
        session_id.clone(),
        cx.clone(),
        queue,
    ))?;
    for update in replay {
        emit(&cx, &session_id, update)?;
    }
    Ok((session_id, model_options(&models, model.as_deref())))
}

fn emit(
    cx: &ConnectionTo<Client>,
    session_id: &SessionId,
    update: SessionUpdate,
) -> Result<(), AgentError> {
    cx.send_notification(SessionNotification::new(session_id.clone(), update))
        .map_err(AgentError::from)
}

/// `session/set_config_option`: only the `model` select exists, and only a
/// model the provider offers can be chosen.
fn set_config_option(
    provider: &dyn Provider,
    registry: &SessionRegistry,
    req: &SetSessionConfigOptionRequest,
) -> Result<SetSessionConfigOptionResponse, AgentError> {
    if req.config_id.to_string() != MODEL_CONFIG_ID {
        return Err(AgentError::Protocol(Error::invalid_params().data(
            serde_json::json!(format!("unknown config option: {}", req.config_id)),
        )));
    }
    let SessionConfigOptionValue::ValueId { value } = &req.value else {
        return Err(AgentError::Protocol(
            Error::invalid_params().data(serde_json::json!("the model option takes a value id")),
        ));
    };
    let models = provider.models();
    let chosen = value.to_string();
    if !models.iter().any(|m| m.id == chosen) {
        return Err(AgentError::UnknownModel(chosen));
    }
    registry.set_model(&req.session_id, chosen.clone())?;
    Ok(SetSessionConfigOptionResponse::new(
        model_options(&models, Some(&chosen)).unwrap_or_default(),
    ))
}
