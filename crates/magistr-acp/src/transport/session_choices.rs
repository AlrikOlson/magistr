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
        let option = find_option(options.as_deref(), key, &category).ok_or_else(|| {
            AcpError::connection(format!("agent does not support selecting {key} '{chosen}'"))
        })?;
        let SessionConfigKind::Select(select) = &option.kind else {
            return Err(AcpError::connection(format!(
                "agent does not expose a {key} selector"
            )));
        };
        if !select_values(&select.options).any(|value| value == chosen) {
            return Err(AcpError::connection(format!(
                "unsupported {key} '{chosen}'"
            )));
        }
        if select.current_value.to_string() == chosen {
            continue;
        }
        let id = option.id.clone();
        options = Some(apply(cx, session_id, id, key, chosen).await?);
    }
    // A preferred model is a soft default (a user's configured favourite):
    // the closest offered model is chosen and a stale id degrades to the
    // agent's own default with a warning instead of refusing the session.
    // An explicit `model` choice above always wins over it.
    if session.get("model").is_none()
        && let Some(preferred) = session
            .get("preferred_model")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
    {
        let Some(option) = find_option(
            options.as_deref(),
            "model",
            &SessionConfigOptionCategory::Model,
        ) else {
            tracing::warn!(
                preferred,
                "agent exposes no model selector; keeping its default model"
            );
            return Ok(options);
        };
        let SessionConfigKind::Select(select) = &option.kind else {
            return Ok(options);
        };
        let offered: Vec<String> = select_values(&select.options).collect();
        match nearest_model(&offered, preferred) {
            Some(chosen) => {
                if chosen != preferred {
                    tracing::warn!(
                        preferred,
                        chosen,
                        "configured default model is not offered by this agent; selected the nearest offered model"
                    );
                }
                if select.current_value.to_string() != chosen {
                    let id = option.id.clone();
                    options = Some(apply(cx, session_id, id, "model", &chosen).await?);
                }
            }
            None => tracing::warn!(
                preferred,
                current = %select.current_value,
                "configured default model is not offered by this agent; keeping the agent's default model"
            ),
        }
    }
    Ok(options)
}

fn find_option<'o>(
    options: Option<&'o [SessionConfigOption]>,
    key: &str,
    category: &SessionConfigOptionCategory,
) -> Option<&'o SessionConfigOption> {
    options.unwrap_or_default().iter().find(|option| {
        option.category.as_ref() == Some(category)
            || (key == "model" && option.id.to_string() == "model")
    })
}

fn select_values(options: &SessionConfigSelectOptions) -> Box<dyn Iterator<Item = String> + '_> {
    match options {
        SessionConfigSelectOptions::Ungrouped(values) => {
            Box::new(values.iter().map(|value| value.value.to_string()))
        }
        SessionConfigSelectOptions::Grouped(groups) => Box::new(
            groups
                .iter()
                .flat_map(|group| &group.options)
                .map(|value| value.value.to_string()),
        ),
        _ => Box::new(std::iter::empty()),
    }
}

/// Strip a trailing context-window marker such as `[1m]`.
fn base_id(id: &str) -> (&str, &str) {
    match id.rfind('[') {
        Some(at) if id.ends_with(']') => (&id[..at], &id[at..]),
        _ => (id, ""),
    }
}

/// The offered model that best answers `preferred`: the exact id, else the
/// same model family (`claude-fable-5` → `claude-fable-5-1`), preferring the
/// same context marker, else none. Offered order is the agent's preference.
pub(crate) fn nearest_model(offered: &[String], preferred: &str) -> Option<String> {
    if offered.iter().any(|value| value == preferred) {
        return Some(preferred.to_owned());
    }
    let (base, marker) = base_id(preferred);
    if base.is_empty() {
        return None;
    }
    let family: Vec<&String> = offered
        .iter()
        .filter(|value| {
            let (candidate, _) = base_id(value);
            candidate == base || candidate.starts_with(&format!("{base}-"))
        })
        .collect();
    family
        .iter()
        .find(|value| base_id(value).1 == marker)
        .or_else(|| family.first())
        .map(|value| (*value).clone())
}

async fn apply(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    id: agent_client_protocol::schema::v1::SessionConfigId,
    key: &str,
    chosen: &str,
) -> Result<Vec<SessionConfigOption>> {
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
    Ok(updated.config_options)
}

#[cfg(test)]
#[path = "session_choices.tests.rs"]
mod tests;
