use crate::run::approval::{self, ApprovalOption, Resolution};

use super::{ApprovalOpening, ApprovalRequest, ApprovalResolution, RunError, RunUseCases};

impl RunUseCases {
    pub async fn open_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<ApprovalOpening, RunError> {
        let (client, tenant, step_id) = {
            let engine = self.lock()?;
            (
                engine.cloud_client(),
                engine.project_id.clone(),
                engine.active_step_wire_id(),
            )
        };
        let options = request.options.unwrap_or_else(|| {
            vec![
                ApprovalOption {
                    key: "yes".into(),
                    label: "Yes, go ahead".into(),
                },
                ApprovalOption {
                    key: "no".into(),
                    label: "No, hold".into(),
                },
            ]
        });
        let default_key = request.default_key.filter(|key| !key.trim().is_empty()).ok_or_else(|| RunError::invalid_args(
            "`default` is required — the option key an unanswered approval resolves to at expiry. Pick the SAFE answer.",
        ))?;
        let approval = approval::open(
            uuid::Uuid::new_v4().to_string(),
            &request.question,
            &request.body,
            options,
            &default_key,
            request
                .timeout_secs
                .unwrap_or(approval::DEFAULT_TIMEOUT_SECS),
            chrono::Utc::now(),
        )
        .map_err(RunError::invalid_args)?;
        let Some(client) = client else {
            return Ok(ApprovalOpening::unopened(approval.default_key,
                "no cloud workspace is connected, so nobody could answer — the declared default applies".into()));
        };
        let envelope = crate::cloud::build::from_approval(&tenant, &approval, step_id.as_deref());
        match client.push(&envelope).await {
            Ok(_) => Ok(ApprovalOpening::opened(approval)),
            Err(error) => Ok(ApprovalOpening::unopened(
                approval.default_key,
                format!(
                    "the workspace could not be reached ({error}) — nobody could answer, so the declared default applies"
                ),
            )),
        }
    }

    pub async fn wait_approval(
        &self,
        approval_id: &str,
        wait_secs: Option<i64>,
    ) -> Result<ApprovalResolution, RunError> {
        if approval_id.trim().is_empty() {
            return Err(RunError::invalid_args(
                "`approval_id` is required (returned by approval_open)",
            ));
        }
        let client = self.lock()?.cloud_client().ok_or_else(|| RunError::Refused {
            kind: "no_cloud", message: "no cloud workspace is connected — approvals live in the workspace. Apply your declared default.".into(),
        })?;
        let wait_secs = wait_secs.unwrap_or(25).clamp(0, 55);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs as u64);
        loop {
            let envelope = match client.get("run", "approval", approval_id).await {
                Ok(Some(envelope)) => envelope,
                Ok(None) => {
                    return Err(RunError::Refused {
                        kind: "not_found",
                        message: format!("approval '{approval_id}' was not found in the workspace"),
                    });
                }
                Err(error) => {
                    return Err(RunError::Refused {
                        kind: "cloud_unreachable",
                        message: format!(
                            "the workspace could not be reached ({error}) — apply your declared default once expires_at passes"
                        ),
                    });
                }
            };
            let record = envelope
                .get("record")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            match approval::resolve(&record, chrono::Utc::now()) {
                Ok(Resolution::Answered(answer)) => {
                    return Ok(ApprovalResolution::Answered {
                        choice: answer.choice,
                        decided_by: answer.decided_by,
                        decided_at: answer.decided_at,
                        note: answer.note,
                    });
                }
                Ok(Resolution::Expired { choice }) => {
                    return Ok(ApprovalResolution::Expired {
                        choice,
                        note: "nobody answered before expires_at — the declared default applies",
                    });
                }
                Ok(Resolution::Pending { seconds_left }) => {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        return Ok(ApprovalResolution::Pending {
                            seconds_left,
                            hint: "call approval_wait again to keep waiting; the approval resolves to its default at expires_at",
                        });
                    }
                    tokio::time::sleep(remaining.min(std::time::Duration::from_secs(3))).await;
                }
                Err(message) => {
                    return Err(RunError::Refused {
                        kind: "malformed_approval",
                        message,
                    });
                }
            }
        }
    }
}
