//! Apply explicit choices against negotiated options before exposing readiness.
use crate::{AcpError, Result};
use agent_client_protocol::schema::v1::{
    Meta, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigOptionValue, SessionConfigSelectOptions, SessionId, SetSessionConfigOptionRequest,
};
use agent_client_protocol::{Agent, ConnectionTo};

pub(super) async fn configure(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    mut options: Option<Vec<SessionConfigOption>>,
    requested: Option<&Meta>,
    advertised: Option<&Meta>,
) -> Result<Option<Vec<SessionConfigOption>>> {
    let Some(session) = requested
        .and_then(|meta| meta.get("magistr"))
        .and_then(|value| value.get("session"))
    else {
        return Ok(options);
    };
    for (key, category) in [
        ("model", SessionConfigOptionCategory::Model),
        ("effort", SessionConfigOptionCategory::ThoughtLevel),
    ] {
        let Some(value) = session.get(key) else {
            continue;
        };
        let chosen = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AcpError::connection(format!("session {key} must be a nonempty string"))
            })?;
        // A provider advertising this extension validates and applies the effort
        // on session/new/load itself. Others must expose a standard selector.
        if key == "effort"
            && advertised
                .and_then(|meta| meta.get("magistr"))
                .and_then(|value| value.get("session_options"))
                .and_then(|value| value.get(key))
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            continue;
        }
        let option = options
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|option| {
                option.category.as_ref() == Some(&category)
                    || (key == "model" && option.id.to_string() == "model")
            })
            .ok_or_else(|| {
                AcpError::connection(format!("agent does not support selecting {key} '{chosen}'"))
            })?;
        let SessionConfigKind::Select(select) = &option.kind else {
            return Err(AcpError::connection(format!(
                "agent does not expose a {key} selector"
            )));
        };
        let supported = match &select.options {
            SessionConfigSelectOptions::Ungrouped(values) => {
                values.iter().any(|v| v.value.to_string() == chosen)
            }
            SessionConfigSelectOptions::Grouped(groups) => groups
                .iter()
                .flat_map(|g| &g.options)
                .any(|v| v.value.to_string() == chosen),
            _ => false,
        };
        if !supported {
            return Err(AcpError::connection(format!(
                "unsupported {key} '{chosen}'"
            )));
        }
        if select.current_value.to_string() == chosen {
            continue;
        }
        let id = option.id.clone();
        let updated = cx
            .send_request(SetSessionConfigOptionRequest::new(
                session_id.clone(),
                id.clone(),
                SessionConfigOptionValue::ValueId {
                    value: chosen.to_owned().into(),
                },
            ))
            .block_task()
            .await
            .map_err(|error| {
                AcpError::connection(format!("ACP session/configure {key} failed: {error}"))
            })?;
        let acknowledged = updated.config_options.iter().find(|option| option.id == id)
            .is_some_and(|option| matches!(&option.kind, SessionConfigKind::Select(select) if select.current_value.to_string() == chosen));
        if !acknowledged {
            return Err(AcpError::connection(format!(
                "agent did not acknowledge {key} '{chosen}'"
            )));
        }
        options = Some(updated.config_options);
    }
    Ok(options)
}
