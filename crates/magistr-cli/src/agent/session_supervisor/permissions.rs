//! Permission requests and interactive replies belong to the resident owner.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::agent::permission_request::{permission_kind, permission_request};
use magistr_acp::{
    AcpPermissionOption, AcpPermissionOptionKind, AcpPermissionRequest, AcpPermissionResponse,
};
use magistr_application::sessions::{AgentEvent, PermissionChoice, PolicyDecision, SessionPolicy};
use tokio::sync::mpsc;

use super::{Inner, Policy};
use crate::agent::observation::ObservationSender;
use crate::permissions::NoninteractivePolicy;

pub(super) struct Permissions {
    receiver: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
    policy: Option<Box<dyn SessionPolicy>>,
    handling: Option<NoninteractivePolicy>,
    events: Option<ObservationSender<AgentEvent>>,
    pending: HashMap<u64, AcpPermissionRequest>,
    next_id: u64,
}

impl Permissions {
    pub(super) fn new(
        receiver: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
        policy: Option<Policy>,
    ) -> Self {
        let (policy, handling) = policy.map_or((None, None), |(policy, handling)| {
            (Some(policy), Some(handling))
        });
        Self {
            receiver,
            policy,
            handling,
            events: None,
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    pub(super) fn interactive(
        receiver: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
        policy: Box<dyn SessionPolicy>,
        events: ObservationSender<AgentEvent>,
    ) -> Self {
        Self {
            receiver,
            policy: Some(policy),
            handling: None,
            events: Some(events),
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    pub(super) async fn recv(&mut self) -> Option<AcpPermissionRequest> {
        match self.receiver.as_mut() {
            Some(receiver) => receiver.recv().await,
            None => std::future::pending().await,
        }
    }

    pub(super) fn answer(&mut self, request: Option<AcpPermissionRequest>, state: &Mutex<Inner>) {
        let Some(request) = request else {
            self.receiver = None;
            self.drain();
            return;
        };
        if request.response_tx.is_closed() {
            return;
        }
        // Admission and policy decisions share the cancellation lock: once
        // cancel_turn returns, an old request can never receive a fresh grant.
        let inner = state.lock().expect("supervisor state lock poisoned");
        if !can_answer(&inner) {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            return;
        }
        let Some(policy) = self.policy.as_mut() else {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            return;
        };
        if let Some(handling) = self.handling {
            let response = super::super::noninteractive_response::noninteractive_response(
                policy.as_mut(),
                &request,
                handling,
            );
            let _ = request.response_tx.send(response);
            return;
        }
        let risk = match policy.decide(&permission_request(&request)) {
            PolicyDecision::Allow => {
                let response = select(&request.options, AcpPermissionOptionKind::AllowOnce);
                let _ = request.response_tx.send(response);
                return;
            }
            PolicyDecision::Deny => {
                let response = select(&request.options, AcpPermissionOptionKind::RejectOnce);
                let _ = request.response_tx.send(response);
                return;
            }
            PolicyDecision::Ask(risk) => risk,
        };
        if !inner.snapshot.client_attached {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let event = AgentEvent::PermissionRequest {
            id,
            title: request.tool_title.clone(),
            choices: request
                .options
                .iter()
                .map(|option| {
                    PermissionChoice::new(&option.option_id, permission_kind(option.kind))
                })
                .collect(),
            edit: request.tool_kind == Some(agent_client_protocol::schema::v1::ToolKind::Edit),
            risk,
            plan: None,
        };
        if self
            .events
            .as_ref()
            .is_some_and(|events| events.send(event).is_ok())
        {
            self.pending.insert(id, request);
        } else {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
        }
    }

    pub(super) fn reply(&mut self, id: u64, option_id: String, state: &Mutex<Inner>) {
        let Some(request) = self.pending.remove(&id) else {
            return;
        };
        let inner = state.lock().expect("supervisor state lock poisoned");
        let kind = request
            .options
            .iter()
            .find(|option| option.option_id == option_id)
            .map(|option| permission_kind(option.kind));
        let Some(kind) = kind.filter(|_| {
            can_answer(&inner) && inner.snapshot.client_attached && !request.response_tx.is_closed()
        }) else {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            self.emit(AgentEvent::PermissionCancelled { id });
            return;
        };
        if let Some(policy) = self.policy.as_mut() {
            policy.answered(&permission_request(&request), kind);
        }
        let _ = request
            .response_tx
            .send(AcpPermissionResponse::Selected(option_id));
    }

    pub(super) fn constrain(&mut self, id: u64, condition: &str, state: &Mutex<Inner>) {
        let inner = state.lock().expect("supervisor state lock poisoned");
        if can_answer(&inner)
            && inner.snapshot.client_attached
            && let Some(request) = self.pending.get(&id)
            && !request.response_tx.is_closed()
            && let Some(policy) = self.policy.as_mut()
        {
            policy.constrain(&permission_request(request), condition);
        }
    }

    pub(super) fn delegate(&mut self, key: &str, state: &Mutex<Inner>) {
        let inner = state.lock().expect("supervisor state lock poisoned");
        if !inner.closing
            && let Some(policy) = self.policy.as_mut()
        {
            policy.delegate(key);
        }
    }

    pub(super) fn lapse(&mut self, data: &serde_json::Value) {
        let Some(call) = data.get("tool_call_id").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(id) = self
            .pending
            .iter()
            .find_map(|(id, request)| (request.tool_call_id == call).then_some(*id))
        else {
            return;
        };
        let Some(request) = self.pending.remove(&id) else {
            return;
        };
        let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
        self.emit(AgentEvent::PermissionLapsed {
            id,
            title: request.tool_title,
            after_secs: data
                .get("timeout_secs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        });
    }

    pub(super) fn drain(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (id, request) in pending {
            let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            self.emit(AgentEvent::PermissionCancelled { id });
        }
        if let Some(receiver) = self.receiver.as_mut() {
            while let Ok(request) = receiver.try_recv() {
                let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            }
        }
    }

    pub(super) fn close(&mut self) {
        if let Some(receiver) = self.receiver.as_mut() {
            receiver.close();
        }
        self.drain();
        self.receiver = None;
    }

    fn emit(&self, event: AgentEvent) {
        if let Some(events) = &self.events {
            let _ = events.send(event);
        }
    }
}

impl Drop for Permissions {
    fn drop(&mut self) {
        self.close();
    }
}

fn can_answer(inner: &Inner) -> bool {
    !inner.closing && !inner.snapshot.cancellation_requested && inner.snapshot.active_turn.is_some()
}

fn select(options: &[AcpPermissionOption], kind: AcpPermissionOptionKind) -> AcpPermissionResponse {
    options
        .iter()
        .find(|option| option.kind == kind)
        .map_or(AcpPermissionResponse::Cancelled, |option| {
            AcpPermissionResponse::Selected(option.option_id.clone())
        })
}

#[cfg(test)]
#[path = "permissions.tests.rs"]
mod tests;
