//! One served call: the root span, `_meta` negotiation and identity, the
//! tool's own admission, the typed use case, the MRTR gate on the
//! confirm-gated verbs, and the one sealing choke point.

use std::future::Future;

use magistr_workspace::{
    contracts::{EnvelopeVersion, ErrorKind, WorkspaceError, WorkspaceResponse},
    diagnostics::{SpanOutcome, SpanStatus},
    extensions::PluginHost,
    response::{ResponseAdapter, SealedResponse},
    service::{WorkspaceService, WorkspaceSession},
};
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, JsonObject},
    service::{ElicitationMode, RequestContext, RoleServer},
};
use serde::Serialize;
use serde_json::Value;

use super::{
    arguments::{self, UndoRequest},
    elicitation,
};

/// The sealed answer and what the root span records about it.
struct Answer {
    sealed: SealedResponse,
    status: SpanStatus,
    outcome: SpanOutcome,
}

pub(crate) async fn call(
    service: &WorkspaceService,
    fallback: &WorkspaceSession,
    request: CallToolRequestParams,
    context: &RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let name = request.name.to_string();
    // The root-span choke point: every core case and the plugin branch pass
    // through this one dispatch. A protocol-level rejection carries no
    // taxonomy code and ends no span — the record stays contract-clean.
    let span = service.trace().start_tool_span(name.clone());
    // rmcp moves the request's `_meta` into the context before dispatch;
    // the params carry none by the time a handler sees them.
    let meta: &JsonObject = &context.meta;
    let args = request.arguments.unwrap_or_default();
    let answer = dispatch(service, fallback, &name, &args, Some(meta), context).await?;
    span.end(&answer.status, answer.outcome);
    // The one door to the wire: the sealed envelope is both representations.
    let mut result =
        CallToolResult::success(vec![ContentBlock::text(answer.sealed.json().to_owned())]);
    result.is_error = Some(answer.sealed.structured()["ok"] == false);
    result.structured_content = Some(answer.sealed.structured().clone());
    Ok(result)
}

async fn dispatch(
    service: &WorkspaceService,
    fallback: &WorkspaceSession,
    name: &str,
    args: &JsonObject,
    meta: Option<&JsonObject>,
    context: &RequestContext<RoleServer>,
) -> Result<Answer, ErrorData> {
    let version = match ResponseAdapter::negotiate_meta(meta) {
        Ok(version) => version,
        Err(error) => return answer(service.refusal::<Value>(error), EnvelopeVersion::LATEST),
    };
    let explicit = ResponseAdapter::session_meta(meta);
    let admitted;
    let session = if explicit.is_empty() {
        fallback
    } else {
        admitted = match service.admit_session(explicit) {
            Ok(session) => session,
            Err(error) => return answer(service.refusal::<Value>(error), version),
        };
        &admitted
    };
    match name {
        "workbench_status" => answer(service.status(), version),
        "map" => answer(service.map(&arguments::map_input(args)?).await, version),
        "read" => answer(service.read(&arguments::read_input(args)?).await, version),
        "search" => answer(
            service
                .search(&arguments::search_input(args)?, context.ct.clone())
                .await,
            version,
        ),
        "edit" => {
            let input = arguments::edit_input(args)?;
            let response = gated(service, context, |confirm| {
                let mut input = input.clone();
                if confirm.is_some() {
                    input.confirm = confirm;
                }
                async move { service.edit(&input, session).await }
            })
            .await;
            answer(response, version)
        }
        "files" => {
            let input = arguments::files_input(args)?;
            let response = gated(service, context, |confirm| {
                let mut input = input.clone();
                if confirm.is_some() {
                    input.confirm = confirm;
                }
                async move { service.files(&input, session).await }
            })
            .await;
            answer(response, version)
        }
        "write" => {
            let input = arguments::write_input(args)?;
            let response = gated(service, context, |confirm| {
                let mut input = input.clone();
                if confirm.is_some() {
                    input.confirm = confirm;
                }
                async move { service.write(&input, session).await }
            })
            .await;
            answer(response, version)
        }
        "undo" => match arguments::undo_request(args)? {
            UndoRequest::Rewind(input) => answer(service.undo(&input, session).await, version),
            UndoRequest::List(input) => {
                answer(service.journal_list(&input, session).await, version)
            }
        },
        "script" => script(service, args, session, context, version).await,
        "run" => answer(
            service
                .run(&arguments::run_input(args)?, session, context.ct.clone())
                .await,
            version,
        ),
        other => plugin(service, other, args, session, version).await,
    }
}

async fn script(
    service: &WorkspaceService,
    args: &JsonObject,
    session: &WorkspaceSession,
    context: &RequestContext<RoleServer>,
    version: EnvelopeVersion,
) -> Result<Answer, ErrorData> {
    if !service.offers_script() {
        return Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "script is not offered: this server was composed without the sandbox lane (§6.2)",
            None,
        ));
    }
    let input = arguments::script_input(args)?;
    let response = gated(service, context, |confirm| {
        let mut input = input.clone();
        if confirm.is_some() {
            input.confirm = confirm;
        }
        async move {
            match service.script(&input, session).await {
                Some(response) => response,
                None => service.refusal(WorkspaceError::new(
                    ErrorKind::EngineUnavailable,
                    "script: no interpreter is composed",
                )),
            }
        }
    })
    .await;
    answer(response, version)
}

/// Plugin tools dispatch only AFTER every core case and only for namespaced
/// names: a plugin cannot shadow, wrap or rename a core tool because a core
/// name never reaches this branch and a derived wire name always contains
/// "/".
async fn plugin(
    service: &WorkspaceService,
    name: &str,
    args: &JsonObject,
    session: &WorkspaceSession,
    version: EnvelopeVersion,
) -> Result<Answer, ErrorData> {
    if PluginHost::is_plugin_tool_name(name) {
        let budget = args.get("budget").and_then(Value::as_u64).unwrap_or(4000);
        if let Some(response) = service
            .run_plugin_tool(name, &Value::Object(args.clone()), budget, session)
            .await
        {
            return answer(response, version);
        }
    }
    Err(ErrorData::new(
        ErrorCode::METHOD_NOT_FOUND,
        format!("unknown tool {name:?}"),
        None,
    ))
}

/// The MRTR gate: a client that can answer a form elicitation gets the
/// dry-run→confirm contract inside this one call — an accept re-runs the
/// identical call with the minted token. Every other client, and every
/// declined, cancelled or failed elicitation, keeps the equal-path token
/// fallback: CONFIRM_REQUIRED carries confirmToken; a second call with
/// confirm applies. `edit.confirm_mode = "token"` never elicits.
async fn gated<R, F, Fut>(
    service: &WorkspaceService,
    context: &RequestContext<RoleServer>,
    run: F,
) -> WorkspaceResponse<R>
where
    R: Serialize,
    F: Fn(Option<String>) -> Fut,
    Fut: Future<Output = WorkspaceResponse<R>>,
{
    let first = run(None).await;
    let Some(token) = pending_confirm_token(&first) else {
        return first;
    };
    if !service.elicitation_allowed()
        || !context
            .peer
            .supported_elicitation_modes()
            .contains(&ElicitationMode::Form)
    {
        return first;
    }
    let Ok(sealed) = ResponseAdapter::seal(&first) else {
        return first;
    };
    if elicitation::confirm(&context.peer, sealed.structured()).await {
        return run(Some(token)).await;
    }
    first
}

fn pending_confirm_token<R>(response: &WorkspaceResponse<R>) -> Option<String> {
    match response {
        WorkspaceResponse::Failure {
            error:
                WorkspaceError {
                    kind: ErrorKind::ConfirmRequired { confirm_token },
                    ..
                },
            ..
        } => Some(confirm_token.clone()),
        _ => None,
    }
}

/// Stamp the negotiated version, derive the span outcome from the typed
/// answer, then seal once.
fn answer<R: Serialize>(
    mut response: WorkspaceResponse<R>,
    version: EnvelopeVersion,
) -> Result<Answer, ErrorData> {
    let (status, outcome) = match &mut response {
        WorkspaceResponse::Success {
            version: stamped,
            results,
            notes,
            ..
        } => {
            *stamped = version;
            (
                SpanStatus::Ok,
                SpanOutcome {
                    envelope_v: Some(version.get()),
                    notes_count: Some(notes.as_ref().map_or(0, Vec::len)),
                    truncated: Some(results.truncation().is_some()),
                },
            )
        }
        WorkspaceResponse::Failure {
            version: stamped,
            error,
            notes,
        } => {
            *stamped = version;
            (
                SpanStatus::Failed(error.kind.clone()),
                SpanOutcome {
                    envelope_v: Some(version.get()),
                    notes_count: Some(notes.as_ref().map_or(0, Vec::len)),
                    truncated: None,
                },
            )
        }
    };
    let sealed = ResponseAdapter::seal(&response)
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    Ok(Answer {
        sealed,
        status,
        outcome,
    })
}
