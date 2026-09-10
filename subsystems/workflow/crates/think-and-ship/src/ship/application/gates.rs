use crate::ship::gate::{self, GateOption, Resolution};

use super::{ApprovalGateRequest, GateOpening, GateResolution, ShipError, ShipUseCases};

impl ShipUseCases {
    pub async fn open_gate(&self, request: ApprovalGateRequest) -> Result<GateOpening, ShipError> {
        let (client, tenant, task_id) = {
            let engine = self.lock()?;
            (
                engine.cloud_client(),
                engine.project_id.clone(),
                engine.active_task_wire_id(),
            )
        };
        let options = request.options.unwrap_or_else(|| {
            vec![
                GateOption {
                    key: "yes".into(),
                    label: "Yes, go ahead".into(),
                },
                GateOption {
                    key: "no".into(),
                    label: "No, hold".into(),
                },
            ]
        });
        let default_key = request.default_key.filter(|key| !key.trim().is_empty()).ok_or_else(|| ShipError::invalid_args(
            "`default` is required — the option key an unanswered gate resolves to at expiry. Pick the SAFE answer.",
        ))?;
        let gate = gate::open(
            uuid::Uuid::new_v4().to_string(),
            &request.question,
            &request.body,
            options,
            &default_key,
            request.timeout_secs.unwrap_or(gate::DEFAULT_TIMEOUT_SECS),
            chrono::Utc::now(),
        )
        .map_err(ShipError::invalid_args)?;
        let Some(client) = client else {
            return Ok(GateOpening::unopened(gate.default_key,
                "no cloud workspace is connected, so nobody could answer — the declared default applies".into()));
        };
        let envelope = crate::cloud::build::from_gate(&tenant, &gate, task_id.as_deref());
        match client.push(&envelope).await {
            Ok(_) => Ok(GateOpening::opened(gate)),
            Err(error) => Ok(GateOpening::unopened(
                gate.default_key,
                format!(
                    "the workspace could not be reached ({error}) — nobody could answer, so the declared default applies"
                ),
            )),
        }
    }

    pub async fn wait_gate(
        &self,
        gate_id: &str,
        wait_secs: Option<i64>,
    ) -> Result<GateResolution, ShipError> {
        if gate_id.trim().is_empty() {
            return Err(ShipError::invalid_args(
                "`gate_id` is required (returned by ship_gate_open)",
            ));
        }
        let client = self.lock()?.cloud_client().ok_or_else(|| ShipError {
            kind: "no_cloud", message: "no cloud workspace is connected — gates live in the workspace. Apply your declared default.".into(),
        })?;
        let wait_secs = wait_secs.unwrap_or(25).clamp(0, 55);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs as u64);
        loop {
            let envelope = match client.get("ship", "gate", gate_id).await {
                Ok(Some(envelope)) => envelope,
                Ok(None) => {
                    return Err(ShipError {
                        kind: "not_found",
                        message: format!("gate '{gate_id}' was not found in the workspace"),
                    });
                }
                Err(error) => {
                    return Err(ShipError {
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
            match gate::resolve(&record, chrono::Utc::now()) {
                Ok(Resolution::Answered(answer)) => {
                    return Ok(GateResolution::Answered {
                        choice: answer.choice,
                        decided_by: answer.decided_by,
                        decided_at: answer.decided_at,
                        note: answer.note,
                    });
                }
                Ok(Resolution::Expired { choice }) => {
                    return Ok(GateResolution::Expired {
                        choice,
                        note: "nobody answered before expires_at — the declared default applies",
                    });
                }
                Ok(Resolution::Pending { seconds_left }) => {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        return Ok(GateResolution::Pending {
                            seconds_left,
                            hint: "call ship_gate_wait again to keep waiting; the gate resolves to its default at expires_at",
                        });
                    }
                    tokio::time::sleep(remaining.min(std::time::Duration::from_secs(3))).await;
                }
                Err(message) => {
                    return Err(ShipError {
                        kind: "malformed_gate",
                        message,
                    });
                }
            }
        }
    }
}
