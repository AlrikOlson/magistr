use super::WorkerDataFormat;

/// Validate data without expanding YAML aliases or retaining its event stream.
pub(super) fn validate_data(format: WorkerDataFormat, source: &str) -> Option<String> {
    let diagnostic = match format {
        WorkerDataFormat::Json => serde_json::from_str::<serde_json::Value>(source)
            .err()
            .map(|error| error.to_string()),
        WorkerDataFormat::Toml => toml::from_str::<toml::Value>(source)
            .err()
            .map(|error| error.to_string()),
        WorkerDataFormat::Yaml => saphyr_parser::Parser::new_from_str(source)
            .find_map(|event| event.err().map(|error| error.to_string())),
    };
    diagnostic.map(|message| message.chars().take(2048).collect())
}

#[cfg(test)]
#[path = "data_validation.tests.rs"]
mod tests;
