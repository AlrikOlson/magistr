use super::TransactionError;
use crate::{
    file_content::FileContent,
    source_language::language,
    worker::{WorkerDataFormat, WorkerRequest, WorkerResponse, WorkerSupervisor},
};
use std::{path::Path, time::Duration};

pub(super) struct Verification;
impl Verification {
    pub fn supported(path: &str) -> bool {
        data_format(path).is_some() || language(path).is_some()
    }
    pub async fn verify(
        worker: &WorkerSupervisor,
        path: &str,
        content: &FileContent,
    ) -> Result<Option<String>, TransactionError> {
        let FileContent::Text(source) = content else {
            return Ok(None);
        };
        let request = if let Some(format) = data_format(path) {
            WorkerRequest::ValidateData {
                path: path.into(),
                format,
                source: source.clone(),
            }
        } else if let Some(language) = language(path) {
            WorkerRequest::Parse {
                path: path.into(),
                language: language.into(),
                source: source.clone(),
            }
        } else {
            return Ok(None);
        };
        let response = worker
            .call(
                request,
                Duration::from_secs(30),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await?;
        match response {
            WorkerResponse::Parse {
                has_error,
                error_count,
                ..
            } => Ok(has_error.then(|| format!("parser reports {error_count} syntax error(s)"))),
            WorkerResponse::DataValidated { diagnostic } => Ok(diagnostic),
            _ => Err(TransactionError::Io("unexpected parser response".into())),
        }
    }
}
fn data_format(path: &str) -> Option<WorkerDataFormat> {
    match Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Some(WorkerDataFormat::Json),
        "toml" => Some(WorkerDataFormat::Toml),
        "yaml" | "yml" => Some(WorkerDataFormat::Yaml),
        _ => None,
    }
}
