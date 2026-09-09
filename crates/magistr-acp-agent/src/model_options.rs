//! The model picker as `session/new` and `session/set_config_option` report it.

use agent_client_protocol::schema::v1::{
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
};

use crate::ModelChoice;

/// The id of the one config option this agent serves.
pub(crate) const MODEL_CONFIG_ID: &str = "model";

/// A `model` select over `models` with `current` chosen, or `None` when the
/// provider offers no models — a picker with nothing on it is not a picker.
pub(crate) fn model_options(
    models: &[ModelChoice],
    current: Option<&str>,
) -> Option<Vec<SessionConfigOption>> {
    let first = models.first()?;
    let current = current.unwrap_or(first.id.as_str());
    let choices = models
        .iter()
        .map(|m| SessionConfigSelectOption::new(m.id.clone(), m.label.clone()))
        .collect::<Vec<_>>();
    Some(vec![
        SessionConfigOption::select(MODEL_CONFIG_ID, "Model", current.to_owned(), choices)
            .category(SessionConfigOptionCategory::Model),
    ])
}

#[cfg(test)]
#[path = "model_options.tests.rs"]
mod tests;
